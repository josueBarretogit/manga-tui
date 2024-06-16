use crossterm::event::{KeyCode, KeyEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Offset, Rect};
use ratatui::widgets::{Block, Paragraph, Widget, WidgetRef};
use tokio::sync::mpsc::UnboundedSender;
use tui_input::Input;

use crate::backend::tui::{Action, Events, SearchPageActions};

#[derive(Default)]
enum InputMode {
    Typing,
    #[default]
    Idle,
}

///This is the "page" where the user can search for a manga
#[derive(Default)]
pub struct SearchPage {
    search_term: String,
    input_mode: InputMode,
    search_bar: Input,
}

impl WidgetRef for SearchPage {
    fn render_ref(&self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let search_page_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Max(10), Constraint::Min(20)]);

        let [input_area, manga_area] = search_page_layout.areas(area);

        self.render_input_area(input_area, buf);

        self.render_manga_area(manga_area, buf);
    }
}

impl SearchPage {
    pub fn init() -> Self {
        Self::default()
    }

    pub fn render_input_area(&self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(10), Constraint::Percentage(90)])
            .split(area);

        let parag = Paragraph::new(match self.input_mode {
            InputMode::Idle => "Press s to to type".to_string(),
            InputMode::Typing => self.search_term.clone(),
        });

        parag.render(layout[0], buf);

        let input_bar = Paragraph::new(self.search_bar.value()).block(Block::bordered());

        input_bar.render(layout[1], buf);
    }

    pub fn render_manga_area(&self, area: Rect, buf: &mut Buffer) {}
}

impl SearchPage {


    pub fn focus_search_bar(&mut self) {
        self.input_mode = InputMode::Typing;
        self.search_term = String::from("typing mode");
    }

    pub fn update(&mut self, action: SearchPageActions) {
        match action {
            SearchPageActions::SearchManga => {}
        }
    }

    pub fn handle_events(&mut self, events: Events) {
        if let Events::Key(key_event) = events {
            if key_event.kind == KeyEventKind::Press {
                if let KeyCode::Up = key_event.code {
                    self.focus_search_bar();
                }
            }
        }
    }
}
