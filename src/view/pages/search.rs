use crossterm::event::{self, KeyCode, KeyEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Offset, Rect};
use ratatui::widgets::{Block, Paragraph, Widget, WidgetRef};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

use crate::backend::tui::{Action, Events};

pub enum SearchPageActions {
    StartTyping,
    Search,
    Load,
}

#[derive(Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum InputMode {
    Typing,
    #[default]
    Idle,
}

///This is the "page" where the user can search for a manga
pub struct SearchPage {
    action_tx: UnboundedSender<SearchPageActions>,
    pub action_rx: UnboundedReceiver<SearchPageActions>,
    search_term: String,
    pub input_mode: InputMode,
    search_bar: Input,
}

impl WidgetRef for SearchPage {
    fn render_ref(&self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let search_page_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Max(5), Constraint::Min(20)]);

        let [input_area, manga_area] = search_page_layout.areas(area);

        self.render_input_area(input_area, buf);

        self.render_manga_area(manga_area, buf);
    }
}

impl SearchPage {
    pub fn init() -> Self {
        let (action_tx, action_rx) = mpsc::unbounded_channel::<SearchPageActions>();

        Self {
            action_tx,
            action_rx,
            search_term: String::default(),
            input_mode: InputMode::default(),
            search_bar: Input::default(),
        }
    }

    fn render_input_area(&self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Max(2), Constraint::Max(5)])
            .split(area);

        let parag = Paragraph::new(match self.input_mode {
            InputMode::Idle => "Press s to start searching".to_string(),
            InputMode::Typing => self.search_term.clone(),
        });

        parag.render(layout[0], buf);

        let input_bar = Paragraph::new(self.search_bar.value()).block(Block::bordered());

        input_bar.render(layout[1], buf);
    }

    fn render_manga_area(&self, area: Rect, buf: &mut Buffer) {}
}

impl SearchPage {
    pub fn focus_search_bar(&mut self) {
        self.input_mode = InputMode::Typing;
    }

    pub fn update(&mut self, action: SearchPageActions) {
        match action {
            SearchPageActions::StartTyping => self.input_mode = InputMode::Typing,
            SearchPageActions::Search => {}
            SearchPageActions::Load => {}
        }
    }

    pub fn handle_events(&mut self, events: Events) {
        if let Events::Key(key_event) = events {
            match self.input_mode {
                InputMode::Idle => match key_event.code {
                    KeyCode::Up => self.action_tx.send(SearchPageActions::StartTyping).unwrap(),
                    _ => {}
                },
                InputMode::Typing => {
                    self.search_bar.handle_event(&event::Event::Key(key_event));
                }
            }
        }
    }
}
