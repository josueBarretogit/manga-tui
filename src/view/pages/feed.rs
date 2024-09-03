use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
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

use crate::backend::database::{get_history, MangaHistoryResponse, MangaHistoryType};
use crate::backend::error_log::{write_to_error_log, ErrorType};
use crate::backend::feed::OneMangaResponse;
use crate::backend::fetch::MangadexClient;
use crate::backend::tui::Events;
use crate::backend::ChapterResponse;
use crate::global::{ERROR_STYLE, INSTRUCTIONS_STYLE};
use crate::utils::{from_manga_response, render_search_bar};
use crate::view::widgets::feed::{FeedTabs, HistoryWidget, MangasRead};
use crate::view::widgets::search::MangaItem;
use crate::view::widgets::Component;

#[derive(Eq, PartialEq)]
pub enum FeedState {
    SearchingHistory,
    ErrorSearchingHistory,
    SearchingMangaPage,
    MangaPageNotFound,
    DisplayingHistory,
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
    LoadHistory(Option<MangaHistoryResponse>),
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
            state: FeedState::DisplayingHistory,
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

    pub fn is_typing(&self) -> bool {
        self.is_typing
    }

    fn render_history(&mut self, area: Rect, buf: &mut Buffer) {
        match self.history.as_mut() {
            Some(history) => {
                if history.mangas.is_empty() {
                    Paragraph::new("It seems you have no mangas stored here, try reading some").render(area, buf);
                } else {
                    StatefulWidget::render(history.clone(), area, buf, &mut history.state);
                }
            },
            None => {
                if self.state == FeedState::ErrorSearchingHistory {
                    Paragraph::new(
                        "Cannot get your reading history due to some issues, please check error logs"
                            .to_span()
                            .style(*ERROR_STYLE),
                    )
                    .render(area, buf);
                }
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
                    self.local_action_tx.send(FeedActions::ChangeTab).ok();
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

        let history_type = match self.tabs {
            FeedTabs::History => MangaHistoryType::ReadingHistory,
            FeedTabs::PlantToRead => MangaHistoryType::PlanToRead,
        };

        self.tasks.spawn(async move {
            let maybe_reading_history = get_history(history_type, page, &search_term);

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
        match maybe_history {
            Some(history) => {
                self.history = Some(HistoryWidget {
                    page: history.page,
                    total_results: history.total_items,
                    mangas: history
                        .mangas
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
                self.state = FeedState::DisplayingHistory;
                self.local_event_tx.send(FeedEvents::SearchRecentChapters).ok();
            },
            None => {
                self.state = FeedState::ErrorSearchingHistory;
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
                let tx = self.global_event_tx.clone();
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
                FeedActions::ChangeTab => {
                    if let Some(history) = self.history.as_mut() {
                        history.page = 1;
                    }
                    self.change_tab();
                    self.search_history();
                },
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
