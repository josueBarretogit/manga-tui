use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};

use crate::filter::{ContentRating, Filters};
use crate::utils::centered_rect;

pub struct FilterWidgetState {
    pub is_open: bool,
    pub content_rating_state: Vec<ContentRating>,
}

impl FilterWidgetState {
    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    pub fn handle_key_events(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('f') => self.toggle(),
            KeyCode::Esc => self.toggle(),
            KeyCode::Char('j') => todo!(),
            KeyCode::Char('k') => todo!(),
            KeyCode::Enter => {}
            _ => {}
        }
    }
}

impl Default for FilterWidgetState {
    fn default() -> Self {
        Self {
            is_open: false,
            content_rating_state: Filters::default().content_rating,
        }
    }
}

pub struct FilterWidget<'a> {
    pub block: Option<Block<'a>>,
    pub style: Style,
}

impl<'a> StatefulWidget for FilterWidget<'a> {
    type State = FilterWidgetState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let popup_area = centered_rect(area, 80, 50);

        Clear.render(popup_area, buf);

        if let Some(block) = self.block {
            block.render(popup_area, buf);
        }

        let inner = popup_area.inner(Margin {
            horizontal: 2,
            vertical: 2,
        });

        Tabs::new(vec!["Content Rating", "Publication Status"]).render(inner, buf);
    }
}

impl<'a> FilterWidget<'a> {
    pub fn new() -> Self {
        Self {
            block: None,
            style: Style::default(),
        }
    }

    pub fn block(self, block: Block<'a>) -> Self {
        Self {
            block: Some(block),
            style: self.style,
        }
    }
}
