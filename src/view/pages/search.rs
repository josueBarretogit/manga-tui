use crossterm::event::{self, KeyCode, KeyEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Offset, Rect};
use ratatui::widgets::{Block, Paragraph, Widget, WidgetRef};
use ratatui::Frame;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

use crate::backend::tui::Events;
use crate::view::widgets::Component;

pub enum SearchPageActions {
    StartTyping,
    StopTyping,
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
    pub input_mode: InputMode,
    search_bar: Input,
}

impl Component<SearchPageActions> for SearchPage {
    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) {
        let search_page_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Max(4), Constraint::Min(20)]);

        let [input_area, manga_area] = search_page_layout.areas(area);

        self.render_input_area(input_area, frame);

        self.render_manga_area(manga_area, frame.buffer_mut());
    }
    fn update(&mut self, action: SearchPageActions) {
        match action {
            SearchPageActions::StartTyping => self.focus_search_bar(),
            SearchPageActions::StopTyping => self.input_mode = InputMode::Idle,
            SearchPageActions::Search => {}
            SearchPageActions::Load => {}
        }
    }
    fn handle_events(&mut self, events: Events) {
        if let Events::Key(key_event) = events {
            match self.input_mode {
                InputMode::Idle => {
                    if let KeyCode::Char('s') = key_event.code {
                        self.action_tx.send(SearchPageActions::StartTyping).unwrap()
                    }
                }
                InputMode::Typing => match key_event.code {
                    KeyCode::Enter => {
                        self.action_tx.send(SearchPageActions::Search).unwrap();
                    }
                    KeyCode::Esc => {
                        self.action_tx.send(SearchPageActions::StopTyping).unwrap();
                    }
                    _ => {
                        self.search_bar.handle_event(&event::Event::Key(key_event));
                    }
                },
            }
        }
    }
}

impl SearchPage {
    pub fn init() -> Self {
        let (action_tx, action_rx) = mpsc::unbounded_channel::<SearchPageActions>();

        Self {
            action_tx,
            action_rx,
            input_mode: InputMode::default(),
            search_bar: Input::default(),
        }
    }

    fn render_input_area(&self, area: Rect, frame: &mut Frame<'_>) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Max(1), Constraint::Max(5)])
            .split(area);

        let helper = Paragraph::new(match self.input_mode {
            InputMode::Idle => "Press s to type",
            InputMode::Typing => "Press <esc> to stop typing",
        });

        helper.render(layout[0], frame.buffer_mut());

        let input_bar = Paragraph::new(self.search_bar.value()).block(Block::bordered());

        input_bar.render(layout[1], frame.buffer_mut());

        let width = layout[0].width.max(3) - 3;

        let scroll = self.search_bar.visual_scroll(width as usize);

        match self.input_mode {
            InputMode::Idle => {}
            InputMode::Typing => frame.set_cursor(
                layout[1].x + ((self.search_bar.visual_cursor()).max(scroll) - scroll) as u16 + 1,
                layout[1].y + 1,
            ),
        }
    }

    fn render_manga_area(&self, area: Rect, buf: &mut Buffer) {}

    fn focus_search_bar(&mut self) {
        self.input_mode = InputMode::Typing;
    }
}
