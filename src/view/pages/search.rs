use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Offset, Rect};
use ratatui::widgets::{Block, Paragraph, Widget, WidgetRef};
use tokio::sync::mpsc::UnboundedSender;
use tui_input::Input;

use crate::backend::tui::{Action, SearchPageActions};

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
            .constraints([Constraint::Percentage(10), Constraint::Percentage(90)]);

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
        let parag = Paragraph::new(match self.input_mode {
            InputMode::Idle => "Press s to to type".to_string(),
            InputMode::Typing => self.search_term.clone(),
        });

        parag.render(area, buf);

        let input_bar = Paragraph::new(self.search_bar.value()).block(Block::bordered());

        input_bar.render(area, buf);
    }

    pub fn render_manga_area(&self, area: Rect, buf: &mut Buffer) {}
}

impl SearchPage {
    pub fn handle_actions(&mut self, tx: UnboundedSender<Action>, key_pressed: KeyCode) {
        match key_pressed {
            KeyCode::Char('s') => self.focus_search_bar(),
            _ => {}
        }
    }

    pub fn focus_search_bar(&mut self) {
        self.input_mode = InputMode::Typing
    }

    pub fn update(&mut self, action: SearchPageActions) {
        match action {
            SearchPageActions::SearchManga => {}
        }
    }
}
