use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use manga_tui::SearchTerm;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, ToSpan};
use ratatui::widgets::{Block, Paragraph, StatefulWidget, Tabs, Widget};
use ratatui::Frame;
use throbber_widgets_tui::{Throbber, ThrobberState};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

use crate::backend::api_responses::feed::OneMangaResponse;
use crate::backend::api_responses::ChapterResponse;
use crate::backend::database::{get_history, GetHistoryArgs, MangaHistoryResponse, MangaHistoryType, DBCONN};
use crate::backend::error_log::{write_to_error_log, ErrorType};
use crate::backend::fetch::{ApiClient, MangadexClient};
use crate::backend::tui::Events;
use crate::global::{ERROR_STYLE, INSTRUCTIONS_STYLE};
use crate::utils::{from_manga_response, render_search_bar};
use crate::view::widgets::feed::{FeedTabs, HistoryWidget};
use crate::view::widgets::search::MangaItem;
use crate::view::widgets::Component;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FeedState {
    SearchingHistory,
    ErrorSearchingHistory,
    HistoryNotFound,
    SearchingMangaPage,
    MangaPageNotFound,
    DisplayingHistory,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FeedActions {
    ScrollHistoryUp,
    ScrollHistoryDown,
    ToggleSearchBar,
    NextPage,
    PreviousPage,
    SwitchTab,
    GoToMangaPage,
}

#[derive(Debug, PartialEq)]
pub enum FeedEvents {
    SearchingFinalized,
    SearchHistory,
    SearchRecentChapters,
    LoadRecentChapters(String, Option<ChapterResponse>),
    ErrorSearchingMangaData,
    /// page , (history_data, total_results)
    LoadHistory(Option<MangaHistoryResponse>),
}

pub struct Feed {
    pub tabs: FeedTabs,
    state: FeedState,
    pub history: Option<HistoryWidget>,
    pub loading_state: Option<ThrobberState>,
    pub global_event_tx: Option<UnboundedSender<Events>>,
    pub local_action_tx: UnboundedSender<FeedActions>,
    pub local_action_rx: UnboundedReceiver<FeedActions>,
    pub local_event_tx: UnboundedSender<FeedEvents>,
    pub local_event_rx: UnboundedReceiver<FeedEvents>,
    search_bar: Input,
    is_typing: bool,
    tasks: JoinSet<()>,
}

impl Feed {
    pub fn new() -> Self {
        let (local_action_tx, local_action_rx) = mpsc::unbounded_channel::<FeedActions>();
        let (local_event_tx, local_event_rx) = mpsc::unbounded_channel::<FeedEvents>();
        Self {
            tabs: FeedTabs::History,
            loading_state: None,
            history: None,
            state: FeedState::DisplayingHistory,
            global_event_tx: None,
            local_action_tx,
            local_action_rx,
            local_event_tx,
            local_event_rx,
            tasks: JoinSet::new(),
            search_bar: Input::default(),
            is_typing: false,
        }
    }

    pub fn is_typing(&self) -> bool {
        self.is_typing
    }

    pub fn with_global_sender(mut self, sender: UnboundedSender<Events>) -> Self {
        self.global_event_tx = Some(sender);
        self
    }

    fn render_history(&mut self, area: Rect, buf: &mut Buffer) {
        if self.state == FeedState::ErrorSearchingHistory {
            Paragraph::new(
                "Cannot get your reading history due to some issues, please check error logs"
                    .to_span()
                    .style(*ERROR_STYLE),
            )
            .render(area, buf);
            return;
        }
        match self.history.as_mut() {
            Some(history) => {
                if self.state == FeedState::HistoryNotFound {
                    Paragraph::new("It seems you have no mangas stored here, try reading some").render(area, buf);
                } else {
                    StatefulWidget::render(history.clone(), area, buf, &mut history.state);
                }
            },
            None => {
                Paragraph::new("It seems you have no mangas stored here, try reading some").render(area, buf);
            },
        }
    }

    fn render_tabs_and_search_bar(&mut self, area: Rect, frame: &mut Frame) {
        let [tabs_area, search_bar_area] = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(area);

        let selected_tab = match self.tabs {
            FeedTabs::History => 0,
            FeedTabs::PlantToRead => 1,
        };

        let tabs_instructions = Line::from(vec!["Switch tab: ".into(), Span::raw("<tab>").style(*INSTRUCTIONS_STYLE)]);

        Tabs::new(vec!["Reading history", "Plan to Read"])
            .select(selected_tab)
            .block(Block::bordered().title(tabs_instructions))
            .highlight_style(Style::default().fg(Color::Yellow))
            .render(tabs_area, frame.buffer_mut());

        let input_help: Vec<Span<'_>> = if self.is_typing {
            vec!["Press ".into(), Span::raw("<Enter>").style(*INSTRUCTIONS_STYLE), " to search".into()]
        } else {
            vec!["Press ".into(), Span::raw("<s>").style(*INSTRUCTIONS_STYLE), " to filter mangas".into()]
        };

        render_search_bar(self.is_typing, input_help.into(), &self.search_bar, frame, search_bar_area);
    }

    fn render_searching_status(&mut self, area: Rect, buf: &mut Buffer) {
        if let Some(state) = self.loading_state.as_mut() {
            let loader = Throbber::default()
                .label("Searching manga data, please wait ")
                .style(Style::default().fg(Color::Yellow))
                .throbber_set(throbber_widgets_tui::BRAILLE_SIX)
                .use_type(throbber_widgets_tui::WhichUse::Spin);

            StatefulWidget::render(
                loader,
                area.inner(Margin {
                    horizontal: 1,
                    vertical: 1,
                }),
                buf,
                state,
            );
        }
        if self.state == FeedState::MangaPageNotFound {
            Paragraph::new(
                "Error, could not get manga data, please try again another time"
                    .to_span()
                    .style(*ERROR_STYLE),
            )
            .render(area, buf);
        }
    }

    fn render_top_area(&mut self, area: Rect, frame: &mut Frame) {
        let [tabs_and_search_bar_area, searching_area] = Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)]).areas(area);

        self.render_tabs_and_search_bar(tabs_and_search_bar_area, frame);

        self.render_searching_status(searching_area, frame.buffer_mut());
    }

    pub fn init_search(&mut self) {
        self.local_event_tx.send(FeedEvents::SearchHistory).ok();
    }

    fn handle_key_events(&mut self, key_event: KeyEvent) {
        if self.is_typing && self.state != FeedState::SearchingMangaPage {
            match key_event.code {
                KeyCode::Enter => {
                    self.local_event_tx.send(FeedEvents::SearchHistory).ok();
                },
                KeyCode::Esc => {
                    self.local_action_tx.send(FeedActions::ToggleSearchBar).ok();
                },
                _ => {
                    self.search_bar.handle_event(&crossterm::event::Event::Key(key_event));
                },
            };
        } else {
            match key_event.code {
                KeyCode::Tab => {
                    self.local_action_tx.send(FeedActions::SwitchTab).ok();
                },
                KeyCode::Char('j') | KeyCode::Down => {
                    self.local_action_tx.send(FeedActions::ScrollHistoryDown).ok();
                },
                KeyCode::Char('k') | KeyCode::Up => {
                    self.local_action_tx.send(FeedActions::ScrollHistoryUp).ok();
                },
                KeyCode::Char('w') => {
                    self.local_action_tx.send(FeedActions::NextPage).ok();
                },

                KeyCode::Char('b') => {
                    self.local_action_tx.send(FeedActions::PreviousPage).ok();
                },
                KeyCode::Char('r') => {
                    self.local_action_tx.send(FeedActions::GoToMangaPage).ok();
                },
                KeyCode::Char('s') => {
                    self.local_action_tx.send(FeedActions::ToggleSearchBar).ok();
                },
                _ => {},
            }
        }
    }

    pub fn tick(&mut self) {
        if let Some(loader_state) = self.loading_state.as_mut() {
            loader_state.calc_next();
        }
        if let Ok(local_event) = self.local_event_rx.try_recv() {
            match local_event {
                FeedEvents::SearchingFinalized => self.state = FeedState::DisplayingHistory,
                FeedEvents::ErrorSearchingMangaData => self.display_error_searching_manga(),
                FeedEvents::SearchHistory => self.search_history(),
                FeedEvents::LoadHistory(maybe_history) => self.load_history(maybe_history),
                FeedEvents::SearchRecentChapters => self.search_latest_chapters(),
                FeedEvents::LoadRecentChapters(manga_id, maybe_chapters) => {
                    self.load_recent_chapters(manga_id, maybe_chapters);
                },
            }
        }
    }

    fn load_recent_chapters(&mut self, manga_id: String, maybe_history: Option<ChapterResponse>) {
        if let Some(chapters_response) = maybe_history {
            if let Some(history) = self.history.as_mut() {
                history.set_chapter(manga_id, chapters_response);
            }
        }
    }

    fn search_latest_chapters(&mut self) {
        if let Some(history) = self.history.as_mut() {
            for manga in history.mangas.clone() {
                let manga_id = manga.id;
                let tx = self.local_event_tx.clone();
                self.tasks.spawn(async move {
                    let latest_chapter_response = MangadexClient::global().get_latest_chapters(&manga_id).await;
                    match latest_chapter_response {
                        Ok(res) => {
                            if let Ok(chapter_data) = res.json().await {
                                tx.send(FeedEvents::LoadRecentChapters(manga_id, Some(chapter_data))).ok();
                            }
                        },
                        Err(e) => {
                            write_to_error_log(ErrorType::FromError(Box::new(e)));
                            tx.send(FeedEvents::LoadRecentChapters(manga_id, None)).ok();
                        },
                    }
                });
            }
        }
    }

    fn display_error_searching_manga(&mut self) {
        self.loading_state = None;
        self.state = FeedState::MangaPageNotFound;
    }

    fn search_history(&mut self) {
        self.state = FeedState::SearchingHistory;
        let tx = self.local_event_tx.clone();
        self.tasks.abort_all();
        let search_term = self.search_bar.value().to_string();

        let page = match &self.history {
            Some(history) => history.page,
            None => 1,
        };

        let history_type: MangaHistoryType = self.tabs.into();

        self.tasks.spawn(async move {
            let binding = DBCONN.lock().unwrap();
            let conn = binding.as_ref().unwrap();

            let maybe_reading_history = get_history(GetHistoryArgs {
                conn,
                hist_type: history_type,
                page,
                search: SearchTerm::trimmed_lowercased(&search_term),
                items_per_page: 10,
            });

            match maybe_reading_history {
                Ok(history) => {
                    tx.send(FeedEvents::LoadHistory(Some(history))).ok();
                },
                Err(e) => {
                    write_to_error_log(ErrorType::FromError(Box::new(e)));
                    tx.send(FeedEvents::LoadHistory(None)).ok();
                },
            }
        });
    }

    fn search_next_page(&mut self) {
        if let Some(history) = self.history.as_mut() {
            history.next_page();
            self.search_history();
        }
    }

    fn search_previous_page(&mut self) {
        if let Some(history) = self.history.as_mut() {
            history.previous_page();
            self.search_history();
        }
    }

    fn load_history(&mut self, maybe_history: Option<MangaHistoryResponse>) {
        match maybe_history.filter(|history| !history.mangas.is_empty()) {
            Some(history) => {
                self.history = Some(HistoryWidget::from_database_response(history));
                self.state = FeedState::DisplayingHistory;
                self.local_event_tx.send(FeedEvents::SearchRecentChapters).ok();
            },
            None => {
                self.state = FeedState::HistoryNotFound;
                self.history = None;
            },
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
                self.state = FeedState::SearchingMangaPage;
                let tx = self.global_event_tx.as_ref().cloned().unwrap();
                let loca_tx = self.local_event_tx.clone();
                let manga_id = currently_selected_manga.id.clone();

                self.loading_state = Some(ThrobberState::default());
                self.tasks.spawn(async move {
                    let response = MangadexClient::global().get_one_manga(&manga_id).await;
                    match response {
                        Ok(res) => {
                            if let Ok(manga) = res.json::<OneMangaResponse>().await {
                                let manga_found = from_manga_response(manga.data);
                                tx.send(Events::GoToMangaPage(MangaItem::new(manga_found))).ok();
                            }
                        },
                        Err(e) => {
                            write_to_error_log(ErrorType::FromError(Box::new(e)));
                            loca_tx.send(FeedEvents::ErrorSearchingMangaData).ok();
                        },
                    }
                });
            }
        }
    }

    fn toggle_focus_search_bar(&mut self) {
        self.is_typing = !self.is_typing;
    }

    fn switch_tabs(&mut self) {
        self.tabs = self.tabs.cycle();
        if let Some(history) = self.history.as_mut() {
            history.page = 1;
        }
        self.search_history();
    }

    fn handle_mouse_event(&mut self, mouse_event: MouseEvent) {
        match mouse_event.kind {
            MouseEventKind::ScrollUp => {
                self.local_action_tx.send(FeedActions::ScrollHistoryUp).ok();
            },
            MouseEventKind::ScrollDown => {
                self.local_action_tx.send(FeedActions::ScrollHistoryDown).ok();
            },
            _ => {},
        }
    }

    #[cfg(test)]
    fn get_history(&self) -> HistoryWidget {
        self.history.as_ref().cloned().unwrap()
    }
}

impl Component for Feed {
    type Actions = FeedActions;

    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) {
        let layout = Layout::vertical([Constraint::Percentage(20), Constraint::Percentage(80)]);

        let [tabs_area, history_area] = layout.areas(area);

        self.render_top_area(tabs_area, frame);

        self.render_history(history_area, frame.buffer_mut());
    }

    fn update(&mut self, action: Self::Actions) {
        if self.state != FeedState::SearchingMangaPage {
            match action {
                FeedActions::ToggleSearchBar => self.toggle_focus_search_bar(),
                FeedActions::NextPage => self.search_next_page(),
                FeedActions::PreviousPage => self.search_previous_page(),
                FeedActions::GoToMangaPage => self.go_to_manga_page(),
                FeedActions::ScrollHistoryUp => self.select_previous_manga(),
                FeedActions::ScrollHistoryDown => self.select_next_manga(),
                FeedActions::SwitchTab => self.switch_tabs(),
            }
        }
    }

    fn clean_up(&mut self) {
        self.search_bar.reset();
        self.history = None;
        self.loading_state = None;
    }

    fn handle_events(&mut self, events: crate::backend::tui::Events) {
        match events {
            Events::Key(key_event) => {
                self.handle_key_events(key_event);
            },
            Events::Mouse(mouse_event) => self.handle_mouse_event(mouse_event),
            Events::Tick => self.tick(),
            _ => {},
        }
    }
}

#[cfg(test)]
mod tests {
    use core::panic;

    use pretty_assertions::{assert_eq, assert_ne};

    use super::*;
    use crate::backend::api_responses::ChapterData;
    use crate::backend::database::MangaHistory;
    use crate::view::widgets::press_key;

    fn history_data() -> MangaHistoryResponse {
        let mangas_in_history = vec![MangaHistory::default(), MangaHistory::default()];
        let total_items = mangas_in_history.len();

        MangaHistoryResponse {
            mangas: mangas_in_history,
            page: 1,
            total_items: total_items as u32,
        }
    }

    #[test]
    fn search_for_history_when_instantiated() {
        let mut feed_page = Feed::new();

        let expected_event = FeedEvents::SearchHistory;

        feed_page.init_search();

        let event = feed_page.local_event_rx.blocking_recv().expect("the event was not sent");

        assert_eq!(expected_event, event);
    }

    #[test]
    fn history_is_loaded() {
        let mut feed_page = Feed::new();
        let response_from_database = history_data();

        let expected_widget = HistoryWidget::from_database_response(response_from_database.clone());

        feed_page.load_history(Some(response_from_database));

        assert!(feed_page.history.is_some());

        assert_eq!(FeedState::DisplayingHistory, feed_page.state);

        let history = feed_page.get_history();

        assert_eq!(expected_widget.mangas, history.mangas);
        assert_eq!(expected_widget.total_results, history.total_results);
        assert_eq!(expected_widget.page, history.page);
    }

    #[test]
    fn send_events_after_history_is_loaded() {
        let mut feed_page = Feed::new();
        let response_from_database = history_data();

        feed_page.load_history(Some(response_from_database));

        let expected_event = FeedEvents::SearchRecentChapters;

        let event_sent = feed_page.local_event_rx.blocking_recv().expect("no event was sent");

        assert_eq!(expected_event, event_sent);
    }

    #[test]
    fn load_no_mangas_found_from_database() {
        let mut feed_page = Feed::new();

        let mut some_empty_response = history_data();

        some_empty_response.mangas = vec![];

        feed_page.load_history(Some(some_empty_response));

        assert_eq!(feed_page.state, FeedState::HistoryNotFound);

        feed_page.load_history(None);

        assert_eq!(feed_page.state, FeedState::HistoryNotFound);
    }

    #[test]
    fn load_chapters_of_manga() {
        let mut feed_page = Feed::new();

        let mut history = history_data();

        let manga_id = "some_manga_id";

        history.mangas.push(MangaHistory {
            id: "some_manga_id".to_string(),
            title: "some_title".to_string(),
        });

        feed_page.load_history(Some(history));
        let chapter_response = ChapterResponse {
            data: vec![ChapterData::default(), ChapterData::default()],
            ..Default::default()
        };

        feed_page.load_recent_chapters(manga_id.to_string(), Some(chapter_response));

        let expected_result = feed_page.get_history();
        let expected_result = expected_result
            .mangas
            .iter()
            .find(|manga| manga.id == manga_id)
            .expect("manga was not loaded");

        assert!(!expected_result.recent_chapters.is_empty());
    }

    #[tokio::test]
    async fn switch_between_tabs() {
        let mut feed_page = Feed::new();

        assert_eq!(feed_page.tabs, FeedTabs::History);

        feed_page.switch_tabs();

        assert_eq!(feed_page.tabs, FeedTabs::PlantToRead);

        feed_page.switch_tabs();

        assert_eq!(feed_page.tabs, FeedTabs::History);
    }

    #[tokio::test]
    async fn search_history_in_database() {
        let mut feed_page = Feed::new();

        feed_page.search_history();

        assert_eq!(feed_page.state, FeedState::SearchingHistory);

        let event_sent = feed_page.local_event_rx.recv().await.expect("no event was sent");

        match event_sent {
            FeedEvents::LoadHistory(_) => {},
            _ => panic!("expected event LoadHistory "),
        }
    }

    #[tokio::test]
    async fn listen_key_event_to_switch_tabs() {
        let mut feed_page = Feed::new();

        let initial_tab = feed_page.tabs;

        press_key(&mut feed_page, KeyCode::Tab);

        let action_sent = feed_page.local_action_rx.recv().await.expect("no key event was sent");

        feed_page.update(action_sent);

        assert_eq!(feed_page.state, FeedState::SearchingHistory);
        assert_ne!(feed_page.tabs, initial_tab);
    }
}
