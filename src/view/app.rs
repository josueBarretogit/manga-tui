use std::sync::Arc;

use ::crossterm::event::{EventStream, KeyCode, KeyEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::{self, Constraint, Layout, Rect};
use ratatui::style::Color;
use ratatui::widgets::{Block, Borders, Tabs, Widget, WidgetRef};
use ratatui::{Frame, Terminal};
use ratatui_image::picker::Picker;
use reqwest::Client;
use strum::IntoEnumIterator;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::backend::fetch::MangadexClient;
use crate::backend::tui::{Action, Events};
use crate::view::pages::*;

use self::manga::MangaPage;
use self::search::{InputMode, SearchPage};

use super::widgets::Component;

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum AppState {
    Runnning,
    Done,
}

pub struct App {
    picker: Picker,
    pub global_action_tx: UnboundedSender<Action>,
    pub global_action_rx: UnboundedReceiver<Action>,
    pub global_event_tx: UnboundedSender<Events>,
    pub global_event_rx: UnboundedReceiver<Events>,
    pub state: AppState,
    pub current_tab: SelectedTabs,
    pub manga_page: Option<MangaPage>,
    pub search_page: SearchPage,
    fetch_client: Arc<MangadexClient>,
}

impl Component for App {
    type Actions = Action;
    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) {
        let main_layout = Layout::default()
            .direction(layout::Direction::Vertical)
            .constraints([Constraint::Percentage(6), Constraint::Percentage(94)]);

        let [top_tabs_area, page_area] = main_layout.areas(area);

        self.render_top_tabs(top_tabs_area, frame.buffer_mut());

        self.render_pages(page_area, frame);
    }

    fn handle_events(&mut self, events: Events) {
        match events {
            Events::Key(key_event) => {
                if self.search_page.input_mode != InputMode::Typing {
                    match key_event.code {
                        KeyCode::Char('q') => self.global_action_tx.send(Action::Quit).unwrap(),
                        KeyCode::Tab => {
                            if let SelectedTabs::MangaTab = self.current_tab {
                                self.global_action_tx.send(Action::GoToSearchPage).unwrap();
                            }
                        }
                        _ => {}
                    }
                }
            }
            Events::GoToMangaPage(manga) => {
                self.current_tab = SelectedTabs::MangaTab;
                self.manga_page = Some(MangaPage::new(
                    manga.id,
                    manga.title,
                    manga.description,
                    manga.tags,
                    manga.img_url,
                    manga.image_state,
                    self.global_event_tx.clone(),
                ))
            }

            _ => {}
        }
    }

    fn update(&mut self, action: Action) {
        match action {
            Action::Quit => {
                self.state = AppState::Done;
            }
            Action::PreviousTab => self.previous_tab(),
            Action::NextTab => self.next_tab(),
            Action::GoToSearchPage => self.current_tab = SelectedTabs::Search,
            _ => {}
        }
    }
}

impl App {
    pub fn new() -> Self {
        let user_agent = format!(
            "manga-tui/0.beta-testing1.0 ({}/{}/{})",
            std::env::consts::FAMILY,
            std::env::consts::OS,
            std::env::consts::ARCH
        );

        let mut picker = Picker::from_termios().unwrap();

        picker.guess_protocol();

        let mangadex_client = Arc::new(MangadexClient::new(
            Client::builder().user_agent(user_agent).build().unwrap(),
        ));

        let (global_action_tx, global_action_rx) = unbounded_channel::<Action>();
        let (global_event_tx, global_event_rx) = unbounded_channel::<Events>();

        App {
            picker,
            current_tab: SelectedTabs::default(),
            search_page: SearchPage::init(
                Arc::clone(&mangadex_client),
                picker,
                global_event_tx.clone(),
            ),
            manga_page: None,
            global_action_tx,
            global_action_rx,
            global_event_tx,
            global_event_rx,
            state: AppState::Runnning,
            fetch_client: mangadex_client,
        }
    }

    pub fn render_top_tabs(&self, area: Rect, buf: &mut Buffer) {
        let titles: Vec<&str> = if self.current_tab == SelectedTabs::MangaTab {
            match self.manga_page.as_ref() {
                Some(page) => vec!["Search", &page.title],
                None => vec!["Search"],
            }
        } else {
            vec!["Search"]
        };

        let tabs_block = Block::default().borders(Borders::BOTTOM);

        let current_page = self.current_tab as usize;

        Tabs::new(titles)
            .block(tabs_block)
            .highlight_style(Color::Yellow)
            .select(current_page)
            .padding("", "")
            .divider(" | ")
            .render(area, buf);
    }

    pub fn render_pages(&mut self, area: Rect, frame: &mut Frame<'_>) {
        match self.current_tab {
            SelectedTabs::Search => self.render_search_page(area, frame),
            SelectedTabs::MangaTab => self.render_manga_page(area, frame),
        }
    }

    pub fn render_search_page(&mut self, area: Rect, frame: &mut Frame<'_>) {
        self.search_page.render(area, frame);
    }

    pub fn render_manga_page(&mut self, area: Rect, frame: &mut Frame<'_>) {
        if let Some(page) = self.manga_page.as_mut() {
            page.render(area, frame);
        }
    }

    pub fn render_home_page(&self, area: Rect, buf: &mut Buffer) {}

    pub fn next_tab(&mut self) {
        self.current_tab = self.current_tab.next();
    }

    pub fn previous_tab(&mut self) {
        self.current_tab = self.current_tab.previous();
    }
}
