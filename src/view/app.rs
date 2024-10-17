use ::crossterm::event::KeyCode;
use crossterm::event::{KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::{Block, Borders, Tabs, Widget};
use ratatui::Frame;
use ratatui_image::picker::Picker;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use self::feed::Feed;
use self::home::Home;
use self::manga::MangaPage;
use self::reader::{ChapterToRead, ListOfChapters, MangaReader, SearchChapter, SearchMangaPanel};
use self::search::{InputMode, SearchPage};
use super::widgets::search::MangaItem;
use super::widgets::Component;
use crate::backend::fetch::ApiClient;
use crate::backend::tui::{Action, Events};
use crate::global::INSTRUCTIONS_STYLE;
use crate::view::pages::*;

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

pub struct App<T: ApiClient + SearchChapter + SearchMangaPanel> {
    pub global_action_tx: UnboundedSender<Action>,
    pub global_action_rx: UnboundedReceiver<Action>,
    pub global_event_tx: UnboundedSender<Events>,
    pub global_event_rx: UnboundedReceiver<Events>,
    pub state: AppState,
    pub current_tab: SelectedPage,
    pub manga_page: Option<MangaPage>,
    pub manga_reader_page: Option<MangaReader<T>>,
    pub search_page: SearchPage,
    pub home_page: Home,
    pub feed_page: Feed<T>,
    api_client: T,
    // The picker is what decides how big a image needs to be rendered depending on the user's
    // terminal font size and the graphics it supports
    // if the terminal doesn't support any graphics protocol the picker is `None`
    picker: Option<Picker>,
}

impl<T: ApiClient + SearchChapter + SearchMangaPanel> Component for App<T> {
    type Actions = Action;

    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) {
        if self.manga_reader_page.is_some() && self.current_tab == SelectedPage::ReaderTab {
            self.manga_reader_page.as_mut().unwrap().render(area, frame);
        } else {
            let main_layout = Layout::vertical([Constraint::Percentage(6), Constraint::Percentage(94)]);

            let [top_tabs_area, page_area] = main_layout.areas(area);

            self.render_top_tabs(top_tabs_area, frame.buffer_mut());

            self.render_pages(page_area, frame);
        }
    }

    fn handle_events(&mut self, events: Events) {
        match events {
            Events::Key(key_event) => self.handle_key_events(key_event),
            Events::GoToMangaPage(manga) => self.go_to_manga_page(manga),
            Events::ReadChapter(chapter_response, manga_to_read) => self.go_to_read_chapter(chapter_response, manga_to_read),
            Events::GoSearchPage => {
                self.go_search_page();
            },
            Events::GoToHome => self.go_to_home(),
            Events::GoFeedPage => self.go_feed_page(),

            Events::GoSearchMangasAuthor(author) => {
                self.go_search_page();
                self.search_page.search_mangas_of_author(author);
            },
            Events::GoSearchMangasArtist(artist) => {
                self.go_search_page();
                self.search_page.search_mangas_of_artist(artist);
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

impl<T: ApiClient + SearchChapter + SearchMangaPanel> App<T> {
    pub fn new(api_client: T, picker: Option<Picker>) -> Self {
        let (global_action_tx, global_action_rx) = unbounded_channel::<Action>();
        let (global_event_tx, global_event_rx) = unbounded_channel::<Events>();

        global_event_tx.send(Events::GoToHome).ok();

        App {
            picker,
            current_tab: SelectedPage::default(),
            search_page: SearchPage::new(picker).with_global_sender(global_event_tx.clone()),
            feed_page: Feed::new()
                .with_global_sender(global_event_tx.clone())
                .with_api_client(api_client.clone()),
            home_page: Home::new(picker).with_global_sender(global_event_tx.clone()),
            manga_page: None,
            manga_reader_page: None,
            global_action_tx,
            global_action_rx,
            global_event_tx,
            global_event_rx,
            state: AppState::Runnning,
            api_client,
        }
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

    fn handle_key_events(&mut self, key_event: KeyEvent) {
        if self.manga_page.as_ref().is_some_and(|page| page.is_downloading_all_chapters()) {
            return;
        }

        if self.search_page.input_mode != InputMode::Typing && !self.search_page.is_typing_filter() && !self.feed_page.is_typing() {
            match key_event.code {
                KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => {
                    self.global_action_tx.send(Action::Quit).ok();
                },
                KeyCode::Char('u') | KeyCode::F(1) => {
                    if self.current_tab != SelectedPage::ReaderTab {
                        self.global_event_tx.send(Events::GoToHome).ok();
                    }
                },
                KeyCode::Char('i') | KeyCode::F(2) => {
                    if self.current_tab != SelectedPage::ReaderTab {
                        self.global_event_tx.send(Events::GoSearchPage).ok();
                    }
                },
                KeyCode::Char('o') | KeyCode::F(3) => {
                    if self.current_tab != SelectedPage::ReaderTab {
                        self.global_event_tx.send(Events::GoFeedPage).ok();
                    }
                },
                KeyCode::Backspace => {
                    if self.current_tab == SelectedPage::ReaderTab && self.manga_reader_page.is_some() {
                        self.manga_reader_page.as_mut().unwrap().clean_up();
                        self.current_tab = SelectedPage::MangaTab;
                    }
                },

                _ => {},
            }
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

    fn go_to_manga_page(&mut self, manga: MangaItem) {
        if self.manga_reader_page.is_some() {
            self.manga_reader_page.as_mut().unwrap().clean_up();
            self.manga_reader_page = None;
        }

        self.feed_page.clean_up();

        self.current_tab = SelectedPage::MangaTab;
        self.manga_page = Some(MangaPage::new(manga.manga, self.picker).with_global_sender(self.global_event_tx.clone()));
    }

    fn go_to_read_chapter(&mut self, chapter_to_read: ChapterToRead, manga_to_read: MangaToRead) {
        self.home_page.clean_up();
        self.feed_page.clean_up();
        self.current_tab = SelectedPage::ReaderTab;

        let mut manga_reader = MangaReader::new(
            chapter_to_read,
            manga_to_read.manga_id,
            self.picker.as_ref().cloned().unwrap(),
            self.api_client.clone(),
        )
        .with_global_sender(self.global_event_tx.clone())
        .with_list_of_chapters(manga_to_read.list)
        .with_manga_title(manga_to_read.title);

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
                if let Some(manga_page) = self.manga_page.as_mut() {
                    if let Ok(action) = manga_page.local_action_rx.try_recv() {
                        manga_page.update(action);
                    }
                }
            },
            SelectedPage::ReaderTab => {
                if let Some(reader_page) = self.manga_reader_page.as_mut() {
                    if let Ok(reader_action) = reader_page.local_action_rx.try_recv() {
                        reader_page.update(reader_action);
                    }
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
        self.manga_page = Some(MangaPage::new(crate::common::Manga::default(), self.picker.as_ref().cloned()));

        self
    }
}

#[cfg(test)]
mod tests {

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::backend::fetch::fake_api_client::MockMangadexClient;
    use crate::view::widgets::press_key;

    fn tick<T: ApiClient + SearchChapter + SearchMangaPanel>(app: &mut App<T>) {
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
    fn goes_to_home_page() {
        let mut app = App::new(MockMangadexClient::new(), None);

        let first_event = app.global_event_rx.blocking_recv().expect("no event was sent");

        assert_eq!(Events::GoToHome, first_event);
        assert_eq!(app.current_tab, SelectedPage::Home);
    }

    #[test]
    fn can_go_to_search_page_by_pressing_i() {
        let mut app = App::new(MockMangadexClient::new(), None);

        press_key(&mut app, KeyCode::Char('i'));

        tick(&mut app);

        assert_eq!(app.current_tab, SelectedPage::Search);
    }

    #[test]
    fn can_go_to_home_by_pressing_u() {
        let mut app = App::new(MockMangadexClient::new(), None);

        app.go_search_page();

        press_key(&mut app, KeyCode::Char('u'));

        tick(&mut app);

        assert_eq!(app.current_tab, SelectedPage::Home);
    }

    #[test]
    fn can_go_to_feed_by_pressing_o() {
        let mut app = App::new(MockMangadexClient::new(), None);

        press_key(&mut app, KeyCode::Char('o'));

        tick(&mut app);

        assert_eq!(app.current_tab, SelectedPage::Feed);
    }

    #[test]
    fn doesnt_listen_to_key_events_if_it_is_downloading_all_chapters() {
        let mut app = App::new(MockMangadexClient::new(), None).with_manga_page();

        app.manga_page.as_mut().unwrap().start_downloading_all_chapters();

        press_key(&mut app, KeyCode::Char('o'));
        press_key(&mut app, KeyCode::Char('i'));
        press_key(&mut app, KeyCode::F(2));

        tick(&mut app);

        assert_eq!(app.current_tab, SelectedPage::Home)
    }
}
