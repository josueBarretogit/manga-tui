use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Widget, WidgetRef};
use tokio::sync::mpsc::UnboundedSender;

use crate::backend::tui::{Action, SearchPageActions};

#[derive(Default)]
enum InputMode {
    Typing,
    #[default]
    Idle,
}

///This is the "page" where the user can search for a manga
pub struct SearchPage {
    search_term: String,
    input_mode: InputMode,
}

impl WidgetRef for SearchPage {
    fn render_ref(&self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
        let search_page_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(15), Constraint::Percentage(85)]);

        let [input_area, manga_area] = search_page_layout.areas(area);

        self.render_input_area(input_area, buf);

        self.render_manga_area(manga_area, buf);
    }
}

#[allow(clippy::new_without_default)]
impl SearchPage {
    pub fn new() -> Self {
        Self {
            search_term: String::default(),
            input_mode: InputMode::default(),
        }
    }

    pub fn render_input_area(&self, area: Rect, buf: &mut Buffer) {}

    pub fn render_manga_area(&self, area: Rect, buf: &mut Buffer) {}
}

impl SearchPage {
    pub fn handle_events(tx: UnboundedSender<Action>) {}

    pub fn update(&mut self, action: SearchPageActions) {
        match action {
            SearchPageActions::SearchManga => {

            },
        }
    }
}
