use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ::crossterm::event::KeyCode;
use crossterm::event::{KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Tabs, Widget};
use ratatui_image::picker::Picker;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use self::feed::Feed;
use self::home::Home;
use self::manga::MangaPage;
use self::reader::MangaReader;
use self::search::{InputMode, SearchPage};
use super::widgets::Component;
use crate::backend::database::Database;
use crate::backend::error_log::write_to_error_log;
use crate::backend::manga_provider::{
    ChapterToRead, FeedPageProvider, LatestChapter, ListOfChapters, Manga, MangaProvider, MangaProviders, MangaStatus, Pagination,
};
use crate::backend::tracker::MangaTracker;
use crate::backend::tui::{Action, Events, NewChapterNotificationGroup};
use crate::config::MangaTuiConfig;
use crate::global::INSTRUCTIONS_STYLE;
use crate::view::pages::*;
use crate::view::tasks::manga::download_new_chapters;
use crate::view::widgets::{ErrorModal, NewChapterItem, NewChaptersModal, NewChaptersModalState};

const NEW_CHAPTER_CHECK_INTERVAL: Duration = Duration::from_secs(10 * 60);

fn find_new_chapters(latest_chapters: &[LatestChapter], previous_ids: &[String], downloaded_ids: &[String]) -> Vec<LatestChapter> {
    let mut known_ids: HashSet<&str> = previous_ids.iter().map(String::as_str).collect();
    known_ids.extend(downloaded_ids.iter().map(String::as_str));

    latest_chapters
        .iter()
        .filter(|chapter| !known_ids.contains(chapter.id.as_str()))
        .cloned()
        .collect()
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum AppState {
    Runnning,
    Done,
}

#[derive(PartialEq, Eq, Debug, Clone, Default)]
pub struct MangaToRead {
    pub title: String,
    pub manga_id: String,
    pub list: ListOfChapters,
}

/// The application which owns every `page` or section in manga-tui
/// its job mainly consists of displaying the selected page, passing the `manga_provider` to each
/// page handling when to quit and switching between pages based on user inputs
pub struct App<T, S>
where
    T: MangaProvider,
    S: MangaTracker,
{
    pub global_action_tx: UnboundedSender<Action>,
    pub global_action_rx: UnboundedReceiver<Action>,
    pub global_event_tx: UnboundedSender<Events>,
    pub global_event_rx: UnboundedReceiver<Events>,
    pub state: AppState,
    pub current_tab: SelectedPage,
    pub manga_page: Option<MangaPage<T, S>>,
    pub manga_reader_page: Option<MangaReader<T, S>>,
    pub search_page: SearchPage<T, S>,
    pub home_page: Home<T>,
    pub feed_page: Feed<T>,
    manga_provider: Arc<T>,
    manga_tracker: Option<S>,
    error_message: Option<String>,
    new_chapters_state: Option<NewChaptersModalState>,
    favorite_status_toast: Option<(String, Instant)>,
    last_new_chapter_check: Instant,
    // The picker is what decides how big a image needs to be rendered depending on the user's
    // terminal font size and the graphics it supports
    // if the terminal doesn't support any graphics protocol the picker is `None`
    picker: Option<Picker>,
    top_tabs_area: Rect,
}

impl<T, S> Component for App<T, S>
where
    T: MangaProvider,
    S: MangaTracker,
{
    type Actions = Action;

    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) {
        if let Some(manga_reader) = &mut self.manga_reader_page
            && self.current_tab == SelectedPage::ReaderTab
        {
            manga_reader.render(area, frame);
        } else {
            let main_layout = Layout::vertical([Constraint::Percentage(6), Constraint::Percentage(94)]);

            let [top_tabs_area, page_area] = main_layout.areas(area);

            self.top_tabs_area = top_tabs_area;
            self.render_top_tabs(top_tabs_area, frame.buffer_mut());

            self.render_pages(page_area, frame);
        }

        if let Some(message) = self.error_message.as_ref().cloned() {
            self.render_modal_error_message(area, frame.buffer_mut(), &message);
        } else if let Some(state) = self.new_chapters_state.as_ref() {
            NewChaptersModal::new(state).render(area, frame.buffer_mut());
        }
    }

    fn handle_events(&mut self, events: Events) {
        match events {
            Events::Key(key_event) => self.handle_key_events(key_event),
            Events::Mouse(mouse_event) => self.handle_mouse_events(mouse_event),
            Events::GoToMangaPage(manga) => self.go_to_manga_page(manga),
            Events::ReadChapter(chapter_response, manga_to_read) => {
                self.go_to_read_chapter(chapter_response, manga_to_read, self.manga_tracker.clone())
            },
            Events::GoSearchPage => {
                self.go_search_page();
            },
            Events::GoToHome => self.go_to_home(),
            Events::GoFeedPage => self.go_feed_page(),

            Events::Error(message) => self.display_error_message(message),
            Events::CheckForNewDownloadedChapters => {
                self.last_new_chapter_check = Instant::now();
                self.check_for_new_downloaded_chapters();
            },
            Events::NewChaptersNotification(groups) => {
                let items = groups
                    .into_iter()
                    .map(|g| NewChapterItem {
                        manga_id: g.manga_id,
                        manga_title: g.manga_title,
                        chapter_numbers: g.chapter_numbers,
                        chapters: g.chapters,
                        selected: true,
                    })
                    .collect();
                self.new_chapters_state = Some(NewChaptersModalState::new(items));
            },
            Events::NewChaptersDownloadFinished(result) => match result {
                Ok(count) => {
                    if let Some(state) = &mut self.new_chapters_state {
                        state.status_message = Some(format!("Downloaded {count} new chapter(s). Press <q> to close."));
                        state.items.clear();
                    } else {
                        self.new_chapters_state = Some(NewChaptersModalState {
                            items: Vec::new(),
                            cursor: 0,
                            status_message: Some(format!("Downloaded {count} new chapter(s). Press <q> to close.")),
                        });
                    }
                },
                Err(error) => self.display_error_message(error),
            },
            Events::AutoDownloadedFavorites(count) => {
                self.favorite_status_toast = Some((format!("★ Auto-downloaded {count} new favorite chapter(s)"), Instant::now()));
            },

            Events::GoBackMangaPage => {
                if self.current_tab == SelectedPage::ReaderTab
                    && let Some(manga_reader) = &mut self.manga_reader_page
                {
                    manga_reader.clean_up();
                    self.current_tab = SelectedPage::MangaTab;
                }
            },
            _ => {},
        }
    }

    fn update(&mut self, action: Action) {
        match action {
            Action::Quit => {
                self.state = AppState::Done;
            },
        }
    }

    fn clean_up(&mut self) {}
}

impl<T, S> App<T, S>
where
    T: MangaProvider,
    S: MangaTracker,
{
    pub fn new(
        api_client: T,
        manga_tracker: Option<S>,
        picker: Option<Picker>,
        filters_state: T::FiltersHandler,
        filter_widget: T::Widget,
    ) -> Self {
        let (global_action_tx, global_action_rx) = unbounded_channel::<Action>();
        let (global_event_tx, global_event_rx) = unbounded_channel::<Events>();

        global_event_tx.send(Events::GoToHome).ok();

        let which_provider = api_client.name();

        let provider = Arc::new(api_client);

        App {
            picker,
            current_tab: SelectedPage::default(),
            search_page: SearchPage::new(
                picker,
                Arc::clone(&provider),
                manga_tracker.clone(),
                filters_state,
                filter_widget,
                Pagination::from_first_page(match which_provider {
                    MangaProviders::Mangadex => 10,
                    MangaProviders::Weebcentral => 24,
                    MangaProviders::Mangapill => 50,
                    MangaProviders::Inmanga => 10,
                    MangaProviders::Mangaoni => 20,
                    MangaProviders::Leercapitulo => 20,
                    MangaProviders::Comizy => 24,
                }),
            )
            .with_global_sender(global_event_tx.clone()),
            feed_page: Feed::new()
                .with_global_sender(global_event_tx.clone())
                .with_api_client(Arc::clone(&provider)),
            home_page: Home::new(picker, Arc::clone(&provider)).with_global_sender(global_event_tx.clone()),
            manga_page: None,
            manga_reader_page: None,
            global_action_tx,
            global_action_rx,
            global_event_tx,
            global_event_rx,
            manga_tracker,
            state: AppState::Runnning,
            error_message: None,
            new_chapters_state: None,
            favorite_status_toast: None,
            last_new_chapter_check: Instant::now(),
            manga_provider: Arc::clone(&provider),
            top_tabs_area: Rect::default(),
        }
    }

    pub fn check_for_new_downloaded_chapters(&self) {
        let provider = Arc::clone(&self.manga_provider);
        let provider_name = provider.name();
        let event_tx = self.global_event_tx.clone();

        tokio::spawn(async move {
            let downloaded_mangas = match Database::get_connection().and_then(|connection| {
                let database = Database::new(&connection);
                database.get_downloaded_mangas(provider_name)
            }) {
                Ok(mangas) => mangas,
                Err(error) => {
                    event_tx.send(Events::Error(error.to_string())).ok();
                    return;
                },
            };

            let mut groups = Vec::new();
            let mut auto_download_chapters = Vec::new();

            let favorite_ids = Database::get_connection()
                .and_then(|conn| Database::new(&conn).get_favorite_manga_ids(provider_name))
                .unwrap_or_default();

            for manga in downloaded_mangas {
                let manga_details = match provider.get_manga_by_id(&manga.id).await {
                    Ok(details) => details,
                    Err(error) => {
                        let message = format!("Could not check the status of {}: {error}", manga.title);
                        write_to_error_log(error.into());
                        event_tx.send(Events::Error(message)).ok();
                        return;
                    },
                };
                if manga_details.status != MangaStatus::Ongoing {
                    continue;
                }

                let latest_chapters = match provider.get_latest_chapters(&manga.id).await {
                    Ok(chapters) if !chapters.is_empty() => chapters,
                    Ok(_) => continue,
                    Err(error) => {
                        let message = format!("Could not check for new chapters of {}: {error}", manga.title);
                        write_to_error_log(error.into());
                        event_tx.send(Events::Error(message)).ok();
                        return;
                    },
                };

                let state_result = Database::get_connection().and_then(|connection| {
                    let database = Database::new(&connection);
                    let dismissed_ids = database.get_dismissed_chapter_ids(&manga.id, provider_name)?;
                    Ok(find_new_chapters(&latest_chapters, &dismissed_ids, &manga.downloaded_chapter_ids))
                });

                let newly_available = match state_result {
                    Ok(chapters) => chapters,
                    Err(error) => {
                        event_tx.send(Events::Error(error.to_string())).ok();
                        return;
                    },
                };

                if !newly_available.is_empty() {
                    if favorite_ids.contains(&manga.id) {
                        auto_download_chapters.extend(newly_available);
                    } else {
                        let chapter_numbers = newly_available
                            .iter()
                            .map(|chapter| chapter.chapter_number.as_str())
                            .collect::<Vec<_>>()
                            .join(", ");
                        groups.push(NewChapterNotificationGroup {
                            manga_id: manga.id,
                            manga_title: manga.title,
                            chapter_numbers,
                            chapters: newly_available,
                        });
                    }
                }
            }

            if !auto_download_chapters.is_empty() {
                let client = Arc::clone(&provider);
                let config = MangaTuiConfig::get().clone();
                let tx = event_tx.clone();
                tokio::spawn(async move {
                    let result = download_new_chapters(client, auto_download_chapters, config).await;
                    match result {
                        Ok(count) => {
                            tx.send(Events::AutoDownloadedFavorites(count)).ok();
                        },
                        Err(e) => {
                            tx.send(Events::Error(format!("Auto-download of favorite chapters failed: {e}"))).ok();
                        },
                    }
                });
            }

            if !groups.is_empty() {
                event_tx.send(Events::NewChaptersNotification(groups)).ok();
            }
        });
    }

    pub fn render_top_tabs(&self, area: Rect, buf: &mut Buffer) {
        let mut titles: Vec<&str> = vec!["Home <F1>/<u>", "Search <F2>/<i>", "Feed <F3>/<o>"];

        let tabs_block = Block::default().borders(Borders::BOTTOM);

        let index_current_tab = match self.current_tab {
            SelectedPage::Home => 0,
            SelectedPage::Search => 1,
            SelectedPage::Feed => 2,
            SelectedPage::MangaTab => {
                titles.push(" 📖 Manga page");
                3
            },
            _ => 0,
        };

        Tabs::new(titles)
            .block(tabs_block)
            .highlight_style(*INSTRUCTIONS_STYLE)
            .select(index_current_tab)
            .padding("", "")
            .divider(" | ")
            .render(area, buf);

        if let Some((msg, time)) = &self.favorite_status_toast {
            if time.elapsed().as_secs() < 12 {
                let toast = Line::from(vec![Span::styled(format!(" {msg} "), Style::default().fg(Color::Yellow).bold())]);
                Paragraph::new(toast).alignment(Alignment::Right).render(area, buf);
            }
        }
    }

    fn render_modal_error_message(&self, area: Rect, buf: &mut Buffer, message: &str) {
        ErrorModal::new(message).render(area, buf);
    }

    fn display_error_message(&mut self, error_message: String) {
        self.error_message = Some(error_message);
    }

    fn close_modal(&mut self) {
        self.error_message = None;
        self.new_chapters_state = None;
    }

    fn dismiss_new_chapters_notification(&mut self) -> Result<(), String> {
        if let Some(state) = &self.new_chapters_state {
            let mut chapter_ids_by_manga = std::collections::BTreeMap::<String, Vec<String>>::new();
            for item in &state.items {
                for chapter in &item.chapters {
                    chapter_ids_by_manga.entry(item.manga_id.clone()).or_default().push(chapter.id.clone());
                }
            }

            let connection = Database::get_connection().map_err(|error| error.to_string())?;
            let database = Database::new(&connection);
            for (manga_id, chapter_ids) in chapter_ids_by_manga {
                database
                    .save_dismissed_chapter_ids(&manga_id, self.manga_provider.name(), &chapter_ids)
                    .map_err(|error| error.to_string())?;
            }
        }

        Ok(())
    }

    fn is_displaying_modal(&self) -> bool {
        self.error_message.is_some() || self.new_chapters_state.is_some()
    }

    pub fn render_pages(&mut self, area: Rect, frame: &mut Frame<'_>) {
        match self.current_tab {
            SelectedPage::Search => self.render_search_page(area, frame),
            SelectedPage::MangaTab => self.render_manga_page(area, frame),
            SelectedPage::Home => self.render_home_page(area, frame),
            SelectedPage::Feed => self.render_feed_page(area, frame),
            // Reader tab should be on full screen
            SelectedPage::ReaderTab => {},
        }
    }

    fn render_feed_page(&mut self, area: Rect, frame: &mut Frame<'_>) {
        self.feed_page.render(area, frame);
    }

    pub fn render_search_page(&mut self, area: Rect, frame: &mut Frame<'_>) {
        self.search_page.render(area, frame);
    }

    pub fn render_manga_page(&mut self, area: Rect, frame: &mut Frame<'_>) {
        if let Some(page) = self.manga_page.as_mut() {
            page.render(area, frame);
        }
    }

    pub fn render_home_page(&mut self, area: Rect, frame: &mut Frame<'_>) {
        self.home_page.render(area, frame);
    }

    /// This method ensures a chapter is bookmarked on quit as well
    /// only if auto_bookmark = true
    fn auto_bookmark_on_quit(&mut self) {
        if let Some(reader_page) = self.manga_reader_page.as_mut()
            && reader_page.auto_bookmark
        {
            reader_page.bookmark_current_chapter();
        }
    }

    fn quit(&mut self) {
        self.auto_bookmark_on_quit();
        self.global_action_tx.send(Action::Quit).ok();
    }

    fn key_events_modal(&mut self, key_event: KeyEvent) {
        if let Some(state) = &mut self.new_chapters_state {
            if state.status_message.is_none() {
                match key_event.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        state.cursor = state.cursor.saturating_sub(1);
                    },
                    KeyCode::Down | KeyCode::Char('j') => {
                        if state.cursor + 1 < state.items.len() {
                            state.cursor += 1;
                        }
                    },
                    KeyCode::Char(' ') => {
                        if let Some(item) = state.items.get_mut(state.cursor) {
                            item.selected = !item.selected;
                        }
                    },
                    KeyCode::Char('a') | KeyCode::Char('v') => {
                        let all_selected = state.items.iter().all(|i| i.selected);
                        for item in &mut state.items {
                            item.selected = !all_selected;
                        }
                    },
                    KeyCode::Char('y') | KeyCode::Char('d') | KeyCode::Enter => {
                        self.download_new_chapters();
                    },
                    KeyCode::Char('q') | KeyCode::Esc => {
                        let dismissal_result = self.dismiss_new_chapters_notification();
                        self.close_modal();
                        if let Err(error) = dismissal_result {
                            self.display_error_message(error);
                        }
                    },
                    KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => self.quit(),
                    _ => {},
                }
                return;
            } else {
                match key_event.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        self.close_modal();
                    },
                    KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => self.quit(),
                    _ => {},
                }
                return;
            }
        }

        match key_event.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.close_modal();
            },
            KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => self.quit(),
            _ => {},
        }
    }

    fn download_new_chapters(&mut self) {
        if let Some(state) = &mut self.new_chapters_state {
            let mut chapters_to_download = Vec::new();
            let mut unselected_by_manga = std::collections::BTreeMap::<String, Vec<String>>::new();

            for item in &state.items {
                if item.selected {
                    chapters_to_download.extend(item.chapters.clone());
                } else {
                    for chapter in &item.chapters {
                        unselected_by_manga.entry(item.manga_id.clone()).or_default().push(chapter.id.clone());
                    }
                }
            }

            if chapters_to_download.is_empty() {
                return;
            }

            if !unselected_by_manga.is_empty() {
                if let Ok(connection) = Database::get_connection() {
                    let database = Database::new(&connection);
                    for (manga_id, chapter_ids) in unselected_by_manga {
                        let _ = database.save_dismissed_chapter_ids(&manga_id, self.manga_provider.name(), &chapter_ids);
                    }
                }
            }

            state.status_message =
                Some(format!("Downloading {} new chapter(s)... Press <q> to close.", chapters_to_download.len()));

            let client = Arc::clone(&self.manga_provider);
            let config = MangaTuiConfig::get().clone();
            let event_tx = self.global_event_tx.clone();

            tokio::spawn(async move {
                let result = download_new_chapters(client, chapters_to_download, config).await;
                event_tx.send(Events::NewChaptersDownloadFinished(result)).ok();
            });
        }
    }

    fn handle_key_events(&mut self, key_event: KeyEvent) {
        if self.is_displaying_modal() {
            return;
        }
        if self.manga_page.as_ref().is_some_and(|page| page.is_downloading_all_chapters()) {
            return;
        }

        if self.search_page.input_mode != InputMode::Typing && !self.search_page.is_typing_filter() && !self.feed_page.is_typing() {
            match key_event.code {
                KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => self.quit(),
                KeyCode::Char('u') | KeyCode::F(1) if self.current_tab != SelectedPage::ReaderTab => {
                    self.global_event_tx.send(Events::GoToHome).ok();
                },
                KeyCode::Char('i') | KeyCode::F(2) if self.current_tab != SelectedPage::ReaderTab => {
                    self.global_event_tx.send(Events::GoSearchPage).ok();
                },
                KeyCode::Char('o') | KeyCode::F(3) if self.current_tab != SelectedPage::ReaderTab => {
                    self.global_event_tx.send(Events::GoFeedPage).ok();
                },

                _ => {},
            }
        }
    }

    fn handle_mouse_events(&mut self, mouse_event: MouseEvent) {
        if self.current_tab == SelectedPage::ReaderTab {
            return;
        }
        if let MouseEventKind::Down(MouseButton::Left) = mouse_event.kind {
            let pos = Position::new(mouse_event.column, mouse_event.row);
            if !self.top_tabs_area.contains(pos) {
                return;
            }
            let num_tabs: u16 = if self.current_tab == SelectedPage::MangaTab { 4 } else { 3 };
            let tab_width = self.top_tabs_area.width / num_tabs;
            if tab_width == 0 {
                return;
            }
            let tab_index = ((mouse_event.column.saturating_sub(self.top_tabs_area.x)) / tab_width).min(num_tabs - 1);
            match tab_index {
                0 => self.global_event_tx.send(Events::GoToHome).ok(),
                1 => self.global_event_tx.send(Events::GoSearchPage).ok(),
                2 => self.global_event_tx.send(Events::GoFeedPage).ok(),
                _ => None,
            };
        }
    }

    fn go_search_page(&mut self) {
        if self.manga_page.is_some() {
            self.manga_page.as_mut().unwrap().clean_up();
            self.manga_page = None;
        }
        self.feed_page.clean_up();
        self.current_tab = SelectedPage::Search;
    }

    fn go_to_manga_page(&mut self, manga: Manga) {
        if self.manga_reader_page.is_some() {
            self.manga_reader_page.as_mut().unwrap().clean_up();
            self.manga_reader_page = None;
        }

        self.feed_page.clean_up();

        self.current_tab = SelectedPage::MangaTab;

        let config = MangaTuiConfig::get();

        let manga_page = MangaPage::new(manga, self.picker, Arc::clone(&self.manga_provider))
            .with_global_sender(self.global_event_tx.clone())
            .auto_bookmark(config.auto_bookmark)
            .with_manga_tracker(self.manga_tracker.clone());

        self.manga_page = Some(manga_page);
    }

    fn go_to_read_chapter(&mut self, chapter_to_read: ChapterToRead, manga_to_read: MangaToRead, manga_tracker: Option<S>) {
        self.home_page.clean_up();
        self.feed_page.clean_up();
        self.current_tab = SelectedPage::ReaderTab;

        let mut manga_reader = MangaReader::new(
            chapter_to_read,
            manga_to_read.manga_id,
            self.picker.as_ref().cloned().unwrap(),
            Arc::clone(&self.manga_provider),
        )
        .with_global_sender(self.global_event_tx.clone())
        .with_list_of_chapters(manga_to_read.list)
        .with_manga_title(manga_to_read.title)
        .with_manga_tracker(manga_tracker);

        let config = MangaTuiConfig::get();

        if config.auto_bookmark {
            manga_reader.set_auto_bookmark();
        }

        manga_reader.init_fetching_pages();

        self.manga_reader_page = Some(manga_reader);
    }

    fn go_to_home(&mut self) {
        if self.manga_page.is_some() {
            self.manga_page.as_mut().unwrap().clean_up();
            self.manga_page = None;
        }

        self.feed_page.clean_up();

        if self.home_page.require_search() {
            self.home_page.init_search();
        }

        self.current_tab = SelectedPage::Home;
    }

    fn go_feed_page(&mut self) {
        if self.manga_page.is_some() {
            self.manga_page.as_mut().unwrap().clean_up();
            self.manga_page = None;
        }
        self.feed_page.init_search();
        self.current_tab = SelectedPage::Feed;
    }

    pub async fn listen_to_event(&mut self) {
        if let Some(event) = self.global_event_rx.recv().await {
            self.handle_events(event.clone());

            if event == Events::Tick && self.last_new_chapter_check.elapsed() >= NEW_CHAPTER_CHECK_INTERVAL {
                self.global_event_tx.send(Events::CheckForNewDownloadedChapters).ok();
            }

            // If the app is displaying an error message then the user can only close the app or
            // the error message popup
            if self.is_displaying_modal()
                && let Events::Key(key_event) = event
            {
                self.key_events_modal(key_event);
                return;
            }

            match self.current_tab {
                SelectedPage::Search => {
                    self.search_page.handle_events(event);
                },
                SelectedPage::MangaTab => {
                    self.manga_page.as_mut().unwrap().handle_events(event);
                },
                SelectedPage::ReaderTab => {
                    self.manga_reader_page.as_mut().unwrap().handle_events(event);
                },
                SelectedPage::Home => {
                    self.home_page.handle_events(event);
                },
                SelectedPage::Feed => {
                    self.feed_page.handle_events(event);
                },
            };
        }
    }

    pub fn update_based_on_action(&mut self) {
        if let Ok(app_action) = self.global_action_rx.try_recv() {
            self.update(app_action);
        }

        match self.current_tab {
            SelectedPage::Search => {
                if let Ok(search_page_action) = self.search_page.local_action_rx.try_recv() {
                    self.search_page.update(search_page_action);
                }
            },
            SelectedPage::MangaTab => {
                if let Some(manga_page) = self.manga_page.as_mut()
                    && let Ok(action) = manga_page.local_action_rx.try_recv()
                {
                    manga_page.update(action);
                }
            },
            SelectedPage::ReaderTab => {
                if let Some(reader_page) = self.manga_reader_page.as_mut()
                    && let Ok(reader_action) = reader_page.local_action_rx.try_recv()
                {
                    reader_page.update(reader_action);
                }
            },
            SelectedPage::Home => {
                if let Ok(home_action) = self.home_page.local_action_rx.try_recv() {
                    self.home_page.update(home_action);
                }
            },
            SelectedPage::Feed => {
                if let Ok(feed_event) = self.feed_page.local_action_rx.try_recv() {
                    self.feed_page.update(feed_event);
                }
            },
        };
    }

    #[cfg(test)]
    fn with_manga_page(mut self) -> Self {
        self.manga_page =
            Some(MangaPage::new(crate::backend::manga_provider::Manga::default(), None, Arc::clone(&self.manga_provider)));

        self
    }
}

#[cfg(test)]
mod tests {

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::backend::manga_provider::mock::{MockFilterState, MockFiltersHandler, MockMangaPageProvider, MockWidgetFilter};
    use crate::backend::manga_provider::{Languages, SortedVolumes, Volumes};
    use crate::backend::tracker::MangaTracker;
    use crate::global::test_utils::TrackerTest;
    use crate::view::widgets::press_key;

    fn tick<T: MangaProvider, S: MangaTracker>(app: &mut App<T, S>) {
        let max_amoun_ticks = 10;
        let mut count = 0;

        loop {
            if let Ok(event) = app.global_event_rx.try_recv() {
                app.handle_events(event);
            }

            if count > max_amoun_ticks {
                break;
            }
            count += 1;
        }
    }

    #[test]
    fn new_chapters_remain_pending_until_dismissed() {
        let latest = vec![
            LatestChapter {
                id: "chapter-2".into(),
                chapter_number: "2".into(),
                ..LatestChapter::default()
            },
            LatestChapter {
                id: "chapter-1".into(),
                chapter_number: "1".into(),
                ..LatestChapter::default()
            },
        ];

        let first_check = find_new_chapters(&latest, &[], &["chapter-1".into()]);
        assert_eq!(first_check.len(), 1);
        assert_eq!(first_check[0].id, "chapter-2");

        let repeated_check = find_new_chapters(&latest, &[], &["chapter-1".into()]);
        assert_eq!(repeated_check.len(), 1);
        assert_eq!(repeated_check[0].id, "chapter-2");

        let dismissed = find_new_chapters(&latest, &["chapter-2".into()], &["chapter-1".into()]);
        assert!(dismissed.is_empty());
        assert!(find_new_chapters(&[latest[0].clone()], &[], &["chapter-2".into()]).is_empty());

        let latest_after_release = vec![
            LatestChapter {
                id: "chapter-3".into(),
                chapter_number: "3".into(),
                ..LatestChapter::default()
            },
            latest[0].clone(),
        ];
        let new_release = find_new_chapters(&latest_after_release, &["chapter-2".into()], &[]);
        assert_eq!(new_release.len(), 1);
        assert_eq!(new_release[0].id, "chapter-3");
    }

    #[test]
    fn test_new_chapters_modal_selection_toggle() {
        let items = vec![
            NewChapterItem {
                manga_id: "one-piece".into(),
                manga_title: "One Piece".into(),
                chapter_numbers: "1196, 1195".into(),
                chapters: vec![LatestChapter {
                    id: "ch-1196".into(),
                    manga_id: "one-piece".into(),
                    chapter_number: "1196".into(),
                    ..Default::default()
                }],
                selected: true,
            },
            NewChapterItem {
                manga_id: "dandadan".into(),
                manga_title: "Dandadan".into(),
                chapter_numbers: "249".into(),
                chapters: vec![LatestChapter {
                    id: "ch-249".into(),
                    manga_id: "dandadan".into(),
                    chapter_number: "249".into(),
                    ..Default::default()
                }],
                selected: true,
            },
        ];

        let mut state = NewChaptersModalState::new(items);
        assert_eq!(state.cursor, 0);
        assert!(state.items[0].selected);
        assert!(state.items[1].selected);

        // Toggle first item (e.g. One Piece)
        state.items[0].selected = !state.items[0].selected;
        assert!(!state.items[0].selected);
        assert!(state.items[1].selected);

        // Toggle all
        let all_selected = state.items.iter().all(|i| i.selected);
        for item in &mut state.items {
            item.selected = !all_selected;
        }
        assert!(state.items[0].selected);
        assert!(state.items[1].selected);
    }

    #[test]
    fn goes_to_home_page() {
        let mut app: App<MockMangaPageProvider, TrackerTest> =
            App::new(MockMangaPageProvider::new(), None, None, MockFiltersHandler::new(MockFilterState {}), MockWidgetFilter {});

        let first_event = app.global_event_rx.blocking_recv().expect("no event was sent");

        assert_eq!(Events::GoToHome, first_event);
        assert_eq!(app.current_tab, SelectedPage::Home);
    }

    #[test]
    fn can_go_to_search_page_by_pressing_i() {
        let mut app: App<MockMangaPageProvider, TrackerTest> =
            App::new(MockMangaPageProvider::new(), None, None, MockFiltersHandler::new(MockFilterState {}), MockWidgetFilter {});

        press_key(&mut app, KeyCode::Char('i'));

        tick(&mut app);

        assert_eq!(app.current_tab, SelectedPage::Search);
    }

    #[test]
    fn can_go_to_home_by_pressing_u() {
        let mut app: App<MockMangaPageProvider, TrackerTest> =
            App::new(MockMangaPageProvider::new(), None, None, MockFiltersHandler::new(MockFilterState {}), MockWidgetFilter {});

        app.go_search_page();

        press_key(&mut app, KeyCode::Char('u'));

        tick(&mut app);

        assert_eq!(app.current_tab, SelectedPage::Home);
    }

    #[test]
    fn can_go_to_feed_by_pressing_o() {
        let mut app: App<MockMangaPageProvider, TrackerTest> =
            App::new(MockMangaPageProvider::new(), None, None, MockFiltersHandler::new(MockFilterState {}), MockWidgetFilter {});

        press_key(&mut app, KeyCode::Char('o'));

        tick(&mut app);

        assert_eq!(app.current_tab, SelectedPage::Feed);
    }

    #[test]
    fn doesnt_listen_to_key_events_if_it_is_downloading_all_chapters() {
        let mut app: App<MockMangaPageProvider, TrackerTest> =
            App::new(MockMangaPageProvider::new(), None, None, MockFiltersHandler::new(MockFilterState {}), MockWidgetFilter {})
                .with_manga_page();

        app.manga_page.as_mut().unwrap().start_downloading_all_chapters();

        press_key(&mut app, KeyCode::Char('o'));
        press_key(&mut app, KeyCode::Char('i'));
        press_key(&mut app, KeyCode::F(2));

        tick(&mut app);

        assert_eq!(app.current_tab, SelectedPage::Home)
    }

    #[test]
    fn reader_page_is_initialized_corectly() {
        let mut app: App<MockMangaPageProvider, TrackerTest> = App::new(
            MockMangaPageProvider::new(),
            None,
            Some(Picker::new((8, 8))),
            MockFiltersHandler::new(MockFilterState {}),
            MockWidgetFilter {},
        )
        .with_manga_page();

        let chapter_to_read = ChapterToRead {
            id: "some_id".to_string(),
            title: "some_title".to_string(),
            number: 1.0,
            volume_number: Some("1".to_string()),
            num_page_bookmarked: None,
            language: Languages::default(),
            pages_url: vec!["http://localhost:3000".parse().unwrap()],
        };

        let list_of_chapter: ListOfChapters = ListOfChapters {
            volumes: SortedVolumes::new(vec![Volumes {
                volume: "1".to_string(),
                ..Default::default()
            }]),
        };

        let manga_tracker = TrackerTest::new();

        app.go_to_read_chapter(
            chapter_to_read,
            MangaToRead {
                title: "some_title".to_string(),
                manga_id: "some_manga_id".to_string(),
                list: list_of_chapter.clone(),
            },
            Some(manga_tracker),
        );

        let reader_page = app.manga_reader_page.unwrap();

        assert!(reader_page.global_event_tx.is_some());
        assert_eq!(reader_page.list_of_chapters, list_of_chapter);
        assert_eq!(SelectedPage::ReaderTab, app.current_tab);
        assert!(reader_page.manga_tracker.is_some());
    }
}
