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

use crate::backend::api_responses::ChapterResponse;
use crate::backend::database::{get_history, GetHistoryArgs, MangaHistoryResponse, MangaHistoryType, DBCONN};
use crate::backend::error_log::{write_to_error_log, ErrorType};
use crate::backend::fetch::ApiClient;
use crate::backend::tui::Events;
use crate::global::{ERROR_STYLE, INSTRUCTIONS_STYLE};
use crate::utils::render_search_bar;
use crate::view::tasks::feed::{search_latest_chapters, search_manga};
use crate::view::widgets::feed::{FeedTabs, HistoryWidget};
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

pub struct Feed<T: ApiClient> {
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
    items_per_page: u32,
    tasks: JoinSet<()>,
    api_client: Option<T>,
}

impl<T: ApiClient> Feed<T> {
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
            items_per_page: 5,
            is_typing: false,
            api_client: None,
        }
    }

    pub fn is_typing(&self) -> bool {
        self.is_typing
    }

    pub fn with_global_sender(mut self, sender: UnboundedSender<Events>) -> Self {
        self.global_event_tx = Some(sender);
        self
    }

    pub fn with_api_client(mut self, api_client: T) -> Self {
        self.api_client = Some(api_client);
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
                let sender = self.local_event_tx.clone();
                let api_client = self.api_client.as_ref().cloned().unwrap();
                self.tasks.spawn(search_latest_chapters(api_client, manga_id, sender));
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

        let items_per_page = self.items_per_page;

        let history_type: MangaHistoryType = self.tabs.into();

        self.tasks.spawn(async move {
            let binding = DBCONN.lock().unwrap();
            let conn = binding.as_ref().unwrap();

            let maybe_reading_history = get_history(GetHistoryArgs {
                conn,
                hist_type: history_type,
                page,
                search: SearchTerm::trimmed_lowercased(&search_term),
                items_per_page,
            });

            match maybe_reading_history {
                Ok(history) => {
                    tx.send(FeedEvents::LoadHistory(Some(history))).ok();
                },
                Err(e) => {
                    write_to_error_log(ErrorType::Error(Box::new(e)));
                    tx.send(FeedEvents::LoadHistory(None)).ok();
                },
            }
        });
    }

    fn search_next_page(&mut self) {
        if let Some(history) = self.history.as_mut() {
            if history.can_search_next_page(self.items_per_page as f64) {
                history.next_page();
                self.search_history();
            }
        }
    }

    fn search_previous_page(&mut self) {
        if let Some(history) = self.history.as_mut() {
            if history.can_search_previous_page() {
                history.previous_page();
                self.search_history();
            }
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

    pub fn go_to_manga_page(&mut self) {
        if let Some(history) = self.history.as_mut() {
            if let Some(currently_selected_manga) = history.get_current_manga_selected() {
                self.state = FeedState::SearchingMangaPage;
                let tx = self.global_event_tx.as_ref().cloned().unwrap();
                let local_tx = self.local_event_tx.clone();
                let manga_id = currently_selected_manga.id.clone();

                self.loading_state = Some(ThrobberState::default());

                let api_client = self.api_client.as_ref().cloned().unwrap();

                self.tasks.spawn(search_manga(api_client, manga_id, tx, local_tx));
            }
        }
    }

    fn toggle_focus_search_bar(&mut self) {
        self.is_typing = !self.is_typing;
    }

    fn set_items_per_page(&mut self, items_per_page: u32) {
        self.items_per_page = items_per_page;
    }

    fn switch_tabs(&mut self) {
        self.tabs = self.tabs.cycle();
        self.clean_up();
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

impl<T: ApiClient> Component for Feed<T> {
    type Actions = FeedActions;

    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) {
        let layout = Layout::vertical([Constraint::Percentage(20), Constraint::Percentage(80)]);

        let [tabs_area, history_area] = layout.areas(area);

        self.render_top_area(tabs_area, frame);

        self.render_history(history_area, frame.buffer_mut());
    }

    fn update(&mut self, action: Self::Actions) {
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

    fn clean_up(&mut self) {
        self.search_bar.reset();
        self.history = None;
        self.loading_state = None;
    }

    fn handle_events(&mut self, events: crate::backend::tui::Events) {
        match events {
            Events::Key(key_event) => {
                if self.state != FeedState::SearchingMangaPage {
                    self.handle_key_events(key_event);
                }
            },
            Events::Mouse(mouse_event) => {
                if self.state != FeedState::SearchingMangaPage {
                    self.handle_mouse_event(mouse_event);
                }
            },
            Events::Tick => self.tick(),
            _ => {},
        }
    }
}

#[cfg(test)]
mod tests {
    use core::panic;

    use pretty_assertions::{assert_eq, assert_ne};

    use self::mpsc::unbounded_channel;
    use super::*;
    use crate::backend::api_responses::ChapterData;
    use crate::backend::database::MangaHistory;
    use crate::backend::fetch::fake_api_client::MockMangadexClient;
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

    fn render_history_and_select(feed_page: &mut Feed<MockMangadexClient>) {
        feed_page.load_history(Some(history_data()));

        let area = Rect::new(0, 0, 20, 20);
        let mut buf = Buffer::empty(area);

        feed_page.render_history(area, &mut buf);

        feed_page.select_next_manga();
    }

    #[test]
    fn search_for_history_when_instantiated() {
        let mut feed_page: Feed<MockMangadexClient> = Feed::new();

        let expected_event = FeedEvents::SearchHistory;

        feed_page.init_search();

        let event = feed_page.local_event_rx.blocking_recv().expect("the event was not sent");

        assert_eq!(expected_event, event);
    }

    #[test]
    fn history_is_loaded() {
        let mut feed_page: Feed<MockMangadexClient> = Feed::new();
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
        let mut feed_page: Feed<MockMangadexClient> = Feed::new();
        let response_from_database = history_data();

        feed_page.load_history(Some(response_from_database));

        let expected_event = FeedEvents::SearchRecentChapters;

        let event_sent = feed_page.local_event_rx.blocking_recv().expect("no event was sent");

        assert_eq!(expected_event, event_sent);
    }

    #[test]
    fn load_no_mangas_found_from_database() {
        let mut feed_page: Feed<MockMangadexClient> = Feed::new();

        let mut some_empty_response = history_data();

        some_empty_response.mangas = vec![];

        feed_page.load_history(Some(some_empty_response));

        assert_eq!(feed_page.state, FeedState::HistoryNotFound);

        feed_page.load_history(None);

        assert_eq!(feed_page.state, FeedState::HistoryNotFound);
    }

    #[test]
    fn load_chapters_of_manga() {
        let mut feed_page: Feed<MockMangadexClient> = Feed::new();

        let mut history = history_data();

        let manga_id = "some_manga_id";
        let chapter_id = "some_chapter_id";

        history.mangas.push(MangaHistory {
            id: "some_manga_id".to_string(),
            ..Default::default()
        });

        feed_page.load_history(Some(history));

        let chapter_response = ChapterResponse {
            data: vec![
                ChapterData {
                    id: chapter_id.to_string(),
                    ..Default::default()
                },
                ChapterData::default(),
            ],
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

        let chapter_loaded = expected_result.recent_chapters.iter().find(|chap| chap.id == chapter_id);

        assert!(chapter_loaded.is_some())
    }

    #[tokio::test]
    async fn load_chapters_of_manga_with_event() {
        let manga_id = "some_manga_id".to_string();

        let api_client = MockMangadexClient::new().with_chapter_response(ChapterResponse {
            data: vec![ChapterData {
                id: manga_id.clone(),
                ..Default::default()
            }],
            ..Default::default()
        });

        let mut feed_page: Feed<MockMangadexClient> = Feed::new().with_api_client(api_client);

        let mut history = history_data();

        history.mangas.push(MangaHistory {
            id: manga_id.clone(),
            ..Default::default()
        });

        feed_page.load_history(Some(history));

        let max_amounts_ticks = 10;
        let mut count = 0;

        loop {
            if count > max_amounts_ticks {
                break;
            }
            feed_page.tasks.join_next().await;
            feed_page.tick();
            count += 1;
        }
        let history = feed_page.get_history();

        let manga_with_chapters = history
            .mangas
            .iter()
            .find(|manga| manga.id == manga_id)
            .expect("the manga was not loaded");

        assert!(!manga_with_chapters.recent_chapters.is_empty())
    }

    #[tokio::test]
    async fn goes_to_next_page() {
        let mut feed_page: Feed<MockMangadexClient> = Feed::new();

        let mut history = history_data();

        history.mangas = vec![MangaHistory::default(), MangaHistory::default(), MangaHistory::default(), MangaHistory::default()];
        history.total_items = history.mangas.len() as u32;
        history.page = 1;

        feed_page.set_items_per_page(3);

        feed_page.load_history(Some(history));

        feed_page.search_next_page();

        assert_eq!(feed_page.get_history().page, 2);
    }

    #[tokio::test]
    async fn goes_to_previous_history_page() {
        let mut feed_page: Feed<MockMangadexClient> = Feed::new();

        let mut history = history_data();

        history.mangas = vec![MangaHistory::default(), MangaHistory::default(), MangaHistory::default(), MangaHistory::default()];
        history.total_items = history.mangas.len() as u32;
        history.page = 2;

        feed_page.set_items_per_page(3);

        feed_page.load_history(Some(history));

        feed_page.search_previous_page();

        assert_eq!(feed_page.get_history().page, 1);
    }

    #[tokio::test]
    async fn switch_between_tabs() {
        let mut feed_page: Feed<MockMangadexClient> = Feed::new();

        assert_eq!(feed_page.tabs, FeedTabs::History);

        feed_page.switch_tabs();

        assert_eq!(feed_page.tabs, FeedTabs::PlantToRead);

        feed_page.switch_tabs();

        assert_eq!(feed_page.tabs, FeedTabs::History);
    }

    #[tokio::test]
    async fn search_history_in_database() {
        let mut feed_page: Feed<MockMangadexClient> = Feed::new();

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
        let mut feed_page: Feed<MockMangadexClient> = Feed::new();

        let initial_tab = feed_page.tabs;

        press_key(&mut feed_page, KeyCode::Tab);

        let action_sent = feed_page.local_action_rx.recv().await.expect("no key event was sent");

        feed_page.update(action_sent);

        assert_eq!(feed_page.state, FeedState::SearchingHistory);
        assert_ne!(feed_page.tabs, initial_tab);

        assert!(feed_page.history.is_none());

        let current_tab = feed_page.tabs;

        press_key(&mut feed_page, KeyCode::Tab);

        let action_sent = feed_page.local_action_rx.recv().await.expect("no key event was sent");

        feed_page.update(action_sent);

        assert_ne!(feed_page.tabs, current_tab);
    }

    #[tokio::test]
    async fn when_switching_tabs_remove_previous_history() {
        let mut feed_page: Feed<MockMangadexClient> = Feed::new();

        let manga_history = MangaHistoryResponse {
            mangas: vec![MangaHistory::default()],
            ..Default::default()
        };

        feed_page.load_history(Some(manga_history));

        press_key(&mut feed_page, KeyCode::Tab);

        let action_sent = feed_page.local_action_rx.recv().await.expect("no key event was sent");

        feed_page.update(action_sent);

        assert!(feed_page.history.is_none());
    }

    #[tokio::test]
    async fn scrolls_history_up_and_down() {
        let mut feed_page: Feed<MockMangadexClient> = Feed::new();

        let manga_history = MangaHistoryResponse {
            mangas: vec![MangaHistory::default(), MangaHistory::default(), MangaHistory::default()],
            ..Default::default()
        };

        feed_page.load_history(Some(manga_history));

        assert!(feed_page.get_history().state.selected.is_none());

        let area = Rect::new(0, 0, 20, 20);
        let mut buf = Buffer::empty(area);

        feed_page.render_history(area, &mut buf);

        // Scroll up
        press_key(&mut feed_page, KeyCode::Char('j'));

        let action_sent = feed_page.local_action_rx.recv().await.expect("no key event was sent");

        feed_page.update(action_sent);

        assert!(feed_page.get_history().state.selected.is_some_and(|index| index == 0));

        // index selected should be 1
        feed_page.select_next_manga();
        // index selected should be 2
        feed_page.select_next_manga();

        // Scroll up
        press_key(&mut feed_page, KeyCode::Char('k'));

        let action_sent = feed_page.local_action_rx.recv().await.expect("no key event was sent");

        feed_page.update(action_sent);

        assert!(feed_page.get_history().state.selected.is_some_and(|index| index == 1));
    }

    #[tokio::test]
    async fn focus_search_bar_when_pressing_s_and_unfocus_when_pressing_esc() {
        let mut feed_page: Feed<MockMangadexClient> = Feed::new();

        assert!(!feed_page.is_typing(), "search_bar should not be focused by default");

        press_key(&mut feed_page, KeyCode::Char('s'));

        let action_sent = feed_page.local_action_rx.recv().await.expect("no key event was sent");

        feed_page.update(action_sent);

        assert!(feed_page.is_typing(), "search_bar shoud be focused");

        press_key(&mut feed_page, KeyCode::Esc);

        let action_sent = feed_page.local_action_rx.recv().await.expect("no key event was sent");

        feed_page.update(action_sent);

        assert!(!feed_page.is_typing(), "should have unfocused the search bar");
    }

    #[tokio::test]
    async fn type_into_search_bar_when_focused() {
        let mut feed_page: Feed<MockMangadexClient> = Feed::new();

        feed_page.toggle_focus_search_bar();

        press_key(&mut feed_page, KeyCode::Char('s'));
        press_key(&mut feed_page, KeyCode::Char('e'));
        press_key(&mut feed_page, KeyCode::Char('a'));

        while let Ok(action) = feed_page.local_action_rx.try_recv() {
            feed_page.update(action);
        }

        let expected = "sea";

        assert_eq!(expected, feed_page.search_bar.value());
    }

    #[tokio::test]
    async fn when_searching_manga_page_should_not_listen_to_key_events() {
        let (tx, _) = unbounded_channel::<Events>();
        let mut feed_page: Feed<MockMangadexClient> = Feed::new().with_global_sender(tx).with_api_client(MockMangadexClient::new());

        render_history_and_select(&mut feed_page);

        feed_page.go_to_manga_page();

        assert_eq!(feed_page.state, FeedState::SearchingMangaPage);

        press_key(&mut feed_page, KeyCode::Char('j'));

        if feed_page.local_action_rx.try_recv().is_ok() {
            panic!("should not receive events")
        }
    }

    #[tokio::test]
    async fn goes_to_manga_page_when_pressing_r_with_selected_manga() {
        let (tx, mut rx) = unbounded_channel::<Events>();
        let mut feed_page: Feed<MockMangadexClient> = Feed::new().with_global_sender(tx).with_api_client(MockMangadexClient::new());

        render_history_and_select(&mut feed_page);
        press_key(&mut feed_page, KeyCode::Char('r'));

        let key_event = feed_page.local_action_rx.recv().await.expect("no key event was sent");

        feed_page.update(key_event);

        let event_sent = rx.recv().await.expect("no event was sent");

        match event_sent {
            Events::GoToMangaPage(_) => {},
            _ => panic!("wrong event was sent"),
        }
    }

    #[tokio::test]
    async fn show_error_when_searching_manga_failed() {
        let (tx, _) = unbounded_channel::<Events>();

        let failing_api_client = MockMangadexClient::new().with_returning_errors();

        let mut feed_page: Feed<MockMangadexClient> = Feed::new().with_global_sender(tx).with_api_client(failing_api_client);

        render_history_and_select(&mut feed_page);

        feed_page.go_to_manga_page();

        feed_page.tasks.join_next().await;

        // Limit the loop to avoid an infinite loop
        let mut counter = 0;
        let max_ticks = 1000;
        loop {
            feed_page.tick();
            if feed_page.state == FeedState::MangaPageNotFound || counter >= max_ticks {
                break;
            }
            counter += 1;
        }

        assert_eq!(feed_page.state, FeedState::MangaPageNotFound);
    }
}
