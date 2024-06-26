use std::sync::Arc;

use ::crossterm::event::{EventStream, KeyCode, KeyEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::{self, Constraint, Layout, Rect};
use ratatui::style::Color;
use ratatui::widgets::{Block, Borders, Tabs, Widget, WidgetRef};
use ratatui::{Frame, Terminal};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::StatefulImage;
use reqwest::Client;
use strum::IntoEnumIterator;
use tokio::sync::mpsc::UnboundedSender;

use crate::backend::fetch::MangadexClient;
use crate::backend::tui::{Action, Events};
use crate::view::pages::*;

use self::search::{InputMode, SearchPage};

use super::widgets::Component;

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum AppState {
    Runnning,
    Done,
}

pub struct App {
    picker: Picker,
    pub action_tx: UnboundedSender<Action>,
    pub state: AppState,
    pub current_tab: SelectedTabs,
    pub search_page: SearchPage,
    fetch_client: Arc<MangadexClient>,
}

impl Component<Action> for App {
    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) {
        let main_layout = Layout::default()
            .direction(layout::Direction::Vertical)
            .constraints([Constraint::Percentage(7), Constraint::Percentage(93)]);

        let [top_tabs_area, page_area] = main_layout.areas(area);

        self.render_top_tabs(top_tabs_area, frame.buffer_mut());

        self.render_pages(page_area, frame);
    }

    fn handle_events(&mut self, events: Events) {
        if let Events::Key(key_event) = events {
            match key_event.code {
                KeyCode::Char('q') => {
                    if self.current_tab == SelectedTabs::Search
                        && self.search_page.input_mode != InputMode::Typing
                    {
                        self.action_tx.send(Action::Quit).unwrap();
                    }
                }
                _ => {}
            }
        }
    }

    fn update(&mut self, action: Action) {
        match action {
            Action::Quit => {
                self.state = AppState::Done;
            }
            Action::PreviousTab => self.previous_tab(),
            Action::NextTab => self.next_tab(),
            _ => {}
        }
    }
}

impl App {
    pub fn new(action_tx: UnboundedSender<Action>, event_tx: UnboundedSender<Events>) -> Self {
        let user_agent = format!(
            "manga-tui/0.beta1.0 ({}/{}/{})",
            std::env::consts::FAMILY,
            std::env::consts::OS,
            std::env::consts::ARCH
        );

        let mut picker = Picker::from_termios().unwrap();

        picker.guess_protocol();

        let mangadex_client = Arc::new(MangadexClient::new(
            Client::builder().user_agent(user_agent).build().unwrap(),
        ));

        App {
            picker,
            current_tab: SelectedTabs::default(),
            search_page: SearchPage::init(Arc::clone(&mangadex_client), picker, event_tx),
            action_tx,
            state: AppState::Runnning,
            fetch_client: mangadex_client,
        }
    }

    pub fn render_top_tabs(&self, area: Rect, buf: &mut Buffer) {
        let titles: Vec<String> = SelectedTabs::iter().map(|page| page.to_string()).collect();

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
        }
    }

    pub fn render_search_page(&mut self, area: Rect, frame: &mut Frame<'_>) {
        self.search_page.render(area, frame);
    }

    pub fn render_home_page(&self, area: Rect, buf: &mut Buffer) {}

    pub fn render_manga_page(&self, area: Rect, buf: &mut Buffer) {}

    pub fn next_tab(&mut self) {
        self.current_tab = self.current_tab.next();
    }

    pub fn previous_tab(&mut self) {
        self.current_tab = self.current_tab.previous();
    }
}
