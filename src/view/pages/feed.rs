use crate::backend::database::{get_history, MangaHistory, MangaHistoryType};
use crate::backend::error_log::{write_to_error_log, ErrorType};
use crate::backend::fetch::MangadexClient;
use crate::backend::tui::Events;
use crate::backend::ChapterResponse;
use crate::global::INSTRUCTIONS_STYLE;
use crate::utils::{from_manga_response, render_search_bar};
use crate::view::widgets::feed::{FeedTabs, HistoryWidget, MangasRead};
use crate::view::widgets::search::MangaItem;
use crate::view::widgets::Component;
use crate::PICKER;
use crossterm::event::{KeyCode, KeyEvent};
use image::io::Reader;
use ratatui::{prelude::*, widgets::*};
use std::io::Cursor;
use throbber_widgets_tui::{Throbber, ThrobberState};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

//todo! make search bar

#[derive(Eq, PartialEq)]
pub enum FeedState {
    SearchingMangaData,
    MangaDataNotFound,
    Normal,
}

pub enum FeedActions {
    ScrollHistoryUp,
    ScrollHistoryDown,
    ToggleSearchBar,
    NextPage,
    PreviousPage,
    ChangeTab,
    GoToMangaPage,
}

pub enum FeedEvents {
    SearchingFinalized,
    SearchHistory,
    SearchRecentChapters,
    LoadRecentChapters(String, Option<ChapterResponse>),
    ErrorSearchingMangaData,
    /// page , (history_data, total_results)
    LoadHistory(u32, Option<(Vec<MangaHistory>, u32)>),
}

pub struct Feed {
    pub tabs: FeedTabs,
    state: FeedState,
    pub history: Option<HistoryWidget>,
    pub loading_state: Option<ThrobberState>,
    pub global_event_tx: UnboundedSender<Events>,
    pub local_action_tx: UnboundedSender<FeedActions>,
    pub local_action_rx: UnboundedReceiver<FeedActions>,
    pub local_event_tx: UnboundedSender<FeedEvents>,
    pub local_event_rx: UnboundedReceiver<FeedEvents>,
    search_bar: Input,
    is_typing: bool,
    tasks: JoinSet<()>,
}

impl Feed {
    pub fn new(global_event_tx: UnboundedSender<Events>) -> Self {
        let (local_action_tx, local_action_rx) = mpsc::unbounded_channel::<FeedActions>();
        let (local_event_tx, local_event_rx) = mpsc::unbounded_channel::<FeedEvents>();
        Self {
            tabs: FeedTabs::History,
            loading_state: None,
            history: None,
            state: FeedState::Normal,
            global_event_tx,
            local_action_tx,
            local_action_rx,
            local_event_tx,
            local_event_rx,
            tasks: JoinSet::new(),
            search_bar: Input::default(),
            is_typing: false,
        }
    }

    fn render_history(&mut self, area: Rect, buf: &mut Buffer) {
        match self.history.as_mut() {
            Some(history) => StatefulWidget::render(history.clone(), area, buf, &mut history.state),
            None => {
                Paragraph::new("You have not read any mangas yet").render(area, buf);
            }
        }
    }

    fn render_tabs_area(&mut self, area: Rect, frame: &mut Frame) {
        let buf = frame.buffer_mut();
        let [tabs_area, loading_state_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(area);

        let selected_tab = match self.tabs {
            FeedTabs::History => 0,
            FeedTabs::PlantToRead => 1,
        };

        let tabs_instructions = Line::from(vec![
            "Switch tab: ".into(),
            Span::raw("<tab>").style(*INSTRUCTIONS_STYLE),
        ]);

        Tabs::new(vec!["Reading history", "Plan to Read"])
            .select(selected_tab)
            .block(Block::bordered().title(tabs_instructions))
            .highlight_style(Style::default().fg(Color::Yellow))
            .render(tabs_area, buf);

        match self.loading_state.as_mut() {
            Some(state) => {
                let loader = Throbber::default()
                    .label("Searching manga data, please wait ")
                    .style(Style::default().fg(Color::Yellow))
                    .throbber_set(throbber_widgets_tui::BRAILLE_SIX)
                    .use_type(throbber_widgets_tui::WhichUse::Spin);

                StatefulWidget::render(
                    loader,
                    loading_state_area.inner(Margin {
                        horizontal: 1,
                        vertical: 1,
                    }),
                    buf,
                    state,
                );
            }
            None => {
                if self.state == FeedState::MangaDataNotFound {
                    Paragraph::new("Error, could not get manga data, please try another time")
                        .render(loading_state_area, buf);
                }

                let input_help = if self.is_typing {
                    "Press <Enter> to serch"
                } else {
                    "Press <s> to search"
                };

                render_search_bar(
                    self.is_typing,
                    input_help.into(),
                    &self.search_bar,
                    frame,
                    loading_state_area,
                );
            }
        }
    }
    pub fn init_search(&mut self) {
        self.local_event_tx.send(FeedEvents::SearchHistory).ok();
    }

    fn handle_key_events(&mut self, key_event: KeyEvent) {
        if self.is_typing {
            match key_event.code {
                KeyCode::Enter => {
                    self.local_event_tx.send(FeedEvents::SearchHistory).ok();
                }
                KeyCode::Esc => {
                    self.local_action_tx.send(FeedActions::ToggleSearchBar).ok();
                }
                _ => {
                    self.search_bar
                        .handle_event(&crossterm::event::Event::Key(key_event));
                }
            };
        } else {
            match key_event.code {
                KeyCode::Tab => {
                    self.local_action_tx.send(FeedActions::ChangeTab).ok();
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.local_action_tx
                        .send(FeedActions::ScrollHistoryDown)
                        .ok();
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.local_action_tx.send(FeedActions::ScrollHistoryUp).ok();
                }
                KeyCode::Char('w') => {
                    self.local_action_tx.send(FeedActions::NextPage).ok();
                }

                KeyCode::Char('b') => {
                    self.local_action_tx.send(FeedActions::PreviousPage).ok();
                }
                KeyCode::Char('r') => {
                    self.local_action_tx.send(FeedActions::GoToMangaPage).ok();
                }
                KeyCode::Char('s') => {
                    self.local_action_tx.send(FeedActions::ToggleSearchBar).ok();
                }
                _ => {}
            }
        }
    }

    pub fn tick(&mut self) {
        if let Some(loader_state) = self.loading_state.as_mut() {
            loader_state.calc_next();
        }
        if let Ok(local_event) = self.local_event_rx.try_recv() {
            match local_event {
                FeedEvents::SearchingFinalized => self.state = FeedState::Normal,
                FeedEvents::ErrorSearchingMangaData => self.display_error_searching_manga(),
                FeedEvents::SearchHistory => self.search_history(),
                FeedEvents::LoadHistory(page, maybe_history) => {
                    self.load_history(page, maybe_history)
                }
                FeedEvents::SearchRecentChapters => self.search_latest_chapters(),
                FeedEvents::LoadRecentChapters(manga_id, maybe_chapters) => {
                    self.load_recent_chapters(manga_id, maybe_chapters);
                }
            }
        }
    }

    fn load_recent_chapters(&mut self, manga_id: String, maybe_history: Option<ChapterResponse>) {
        if let Some(chapters_response) = maybe_history {
            // todo! handle this unwrap
            if let Some(history) = self.history.as_mut() {
                history.set_chapter(manga_id, chapters_response);
            }
        }
    }

    fn search_latest_chapters(&mut self) {
        let history = self.history.as_ref().unwrap();

        for manga in history.mangas.clone() {
            let manga_id = manga.id;
            let tx = self.local_event_tx.clone();
            self.tasks.spawn(async move {
                let latest_chapter_response = MangadexClient::global()
                    .get_latest_chapters(&manga_id)
                    .await;
                match latest_chapter_response {
                    Ok(chapters) => {
                        tx.send(FeedEvents::LoadRecentChapters(manga_id, Some(chapters)))
                            .ok();
                    }
                    Err(e) => {
                        write_to_error_log(ErrorType::FromError(Box::new(e)));

                        tx.send(FeedEvents::LoadRecentChapters(manga_id, None)).ok();
                    }
                }
            });
        }
    }

    // Todo! display that manga data could not be found
    fn display_error_searching_manga(&mut self) {
        self.loading_state = None;
        self.state = FeedState::MangaDataNotFound;
    }

    fn search_history(&mut self) {
        let tx = self.local_event_tx.clone();
        self.tasks.abort_all();
        let search_term = self.search_bar.value().trim().to_lowercase();

        let page = match &self.history {
            Some(history) => history.page,
            None => 1,
        };

        let history_type = match self.tabs {
            FeedTabs::History => MangaHistoryType::ReadingHistory,
            FeedTabs::PlantToRead => MangaHistoryType::PlanToRead,
        };

        self.tasks.spawn(async move {
            let maybe_reading_history = get_history(history_type, page, &search_term);
            tx.send(FeedEvents::LoadHistory(
                page,
                Some(maybe_reading_history.unwrap()),
            ))
            .ok();
        });
    }

    fn search_next_page(&mut self) {
        if let Some(history) = self.history.as_mut() {
            self.tasks.abort_all();
            history.next_page();
            self.search_history();
        }
    }

    fn search_previous_page(&mut self) {
        if let Some(history) = self.history.as_mut() {
            self.tasks.abort_all();
            history.previous_page();
            self.search_history();
        }
    }

    fn load_history(&mut self, page: u32, maybe_history: Option<(Vec<MangaHistory>, u32)>) {
        if let Some(history) = maybe_history {
            self.history = Some(HistoryWidget {
                page,
                total_results: history.1,
                mangas: history
                    .0
                    .iter()
                    .map(|history| MangasRead {
                        id: history.id.clone(),
                        title: history.title.clone(),
                        recent_chapters: vec![],
                        style: Style::default(),
                    })
                    .collect(),
                state: tui_widget_list::ListState::default(),
            });
            self.local_event_tx
                .send(FeedEvents::SearchRecentChapters)
                .ok();
        }
    }

    fn select_next_manga(&mut self) {
        if let Some(mangas) = self.history.as_mut() {
            mangas.select_next();
        }
    }

    fn select_previous_manga(&mut self) {
        if let Some(mangas) = self.history.as_mut() {
            mangas.select_previous();
        }
    }

    fn change_tab(&mut self) {
        match self.tabs {
            FeedTabs::History => self.tabs = FeedTabs::PlantToRead,
            FeedTabs::PlantToRead => self.tabs = FeedTabs::History,
        }
    }

    fn go_to_manga_page(&mut self) {
        if let Some(history) = self.history.as_mut() {
            if let Some(currently_selected_manga) = history.get_current_manga_selected() {
                self.state = FeedState::SearchingMangaData;
                let tx = self.global_event_tx.clone();
                let loca_tx = self.local_event_tx.clone();
                let manga_id = currently_selected_manga.id.clone();

                self.loading_state = Some(ThrobberState::default());
                self.tasks.spawn(async move {
                    let response = MangadexClient::global().get_one_manga(&manga_id).await;
                    match response {
                        Ok(manga) => {
                            let manga_found = from_manga_response(manga.data);

                            if PICKER.is_some() {
                                let cover = MangadexClient::global()
                                    .get_cover_for_manga(
                                        &manga_id,
                                        manga_found.img_url.clone().unwrap_or_default().as_str(),
                                    )
                                    .await;

                                loca_tx.send(FeedEvents::SearchingFinalized).ok();

                                match cover {
                                    Ok(bytes) => {
                                        let dyn_img = Reader::new(Cursor::new(bytes))
                                            .with_guessed_format()
                                            .unwrap();

                                        let maybe_decoded = dyn_img.decode();
                                        tx.send(Events::GoToMangaPage(MangaItem::new(
                                            manga_found,
                                            maybe_decoded.ok().map(|decoded| {
                                                PICKER.unwrap().new_resize_protocol(decoded)
                                            }),
                                        )))
                                        .ok();
                                    }
                                    Err(_) => {
                                        tx.send(Events::GoToMangaPage(MangaItem::new(
                                            manga_found,
                                            None,
                                        )))
                                        .ok();
                                    }
                                }
                            } else {
                                tx.send(Events::GoToMangaPage(MangaItem::new(manga_found, None)))
                                    .ok();
                            }
                        }
                        Err(e) => {
                            write_to_error_log(ErrorType::FromError(Box::new(e)));
                            loca_tx.send(FeedEvents::ErrorSearchingMangaData).ok();
                        }
                    }
                });
            }
        }
    }

    fn toggle_focus_search_bar(&mut self) {
        self.is_typing = !self.is_typing;
    }
}

impl Component for Feed {
    type Actions = FeedActions;
    fn render(&mut self, area: ratatui::prelude::Rect, frame: &mut Frame<'_>) {
        let layout = Layout::vertical([Constraint::Percentage(10), Constraint::Percentage(90)]);

        let [tabs_area, history_area] = layout.areas(area);

        self.render_tabs_area(tabs_area, frame);

        self.render_history(history_area, frame.buffer_mut());
    }

    fn update(&mut self, action: Self::Actions) {
        if self.state != FeedState::SearchingMangaData {
            match action {
                FeedActions::ToggleSearchBar => self.toggle_focus_search_bar(),
                FeedActions::NextPage => self.search_next_page(),
                FeedActions::PreviousPage => self.search_previous_page(),
                FeedActions::GoToMangaPage => self.go_to_manga_page(),
                FeedActions::ScrollHistoryUp => self.select_previous_manga(),
                FeedActions::ScrollHistoryDown => self.select_next_manga(),
                FeedActions::ChangeTab => {
                    if let Some(history) = self.history.as_mut() {
                        history.page = 1;
                    }
                    self.change_tab();
                    self.search_history();
                }
            }
        }
    }

    fn clean_up(&mut self) {
        self.history = None;
        self.loading_state = None;
    }

    fn handle_events(&mut self, events: crate::backend::tui::Events) {
        match events {
            Events::Key(key_event) => {
                self.handle_key_events(key_event);
            }
            Events::Tick => self.tick(),
            _ => {}
        }
    }
}
