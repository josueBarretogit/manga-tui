use crate::filter::Filters;
use crate::utils::centered_rect;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use strum::Display;

#[derive(Display)]
enum FilterTypes {
    ContentRating,
    SortBy,
}

impl From<FilterTypes> for Line<'_> {
    fn from(value: FilterTypes) -> Self {
        Line::from(value.to_string())
    }
}

const FILTERS: [FilterTypes; 2] = [FilterTypes::ContentRating, FilterTypes::SortBy];

struct ContentRatingList {
    is_selected: bool,
}

#[derive(Default)]
pub struct FilterWidgetState {
    pub is_open: bool,
    pub id_filter: usize,
    pub filters: Filters,
    pub content_rating_list_state: ListState,
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
            KeyCode::Tab => self.next_filter(),
            KeyCode::BackTab => self.previous_filter(),
            KeyCode::Enter => {}
            _ => {}
        }
    }

    pub fn next_filter(&mut self) {
        self.id_filter += 1;
    }

    pub fn previous_filter(&mut self) {
        self.id_filter -= 1;
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

        let [tabs_area, current_filter_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)])
                .margin(1)
                .areas(popup_area);

        Tabs::new(FILTERS)
            .select(state.id_filter)
            .highlight_style(Style::default().bg(Color::Yellow))
            .render(tabs_area, buf);

        if let Some(filter) = FILTERS.get(state.id_filter) {
            match filter {
                FilterTypes::ContentRating => {
                    let list = List::new(vec!["safe", "suggestive", "erotica"]);
                    StatefulWidget::render(
                        list,
                        current_filter_area,
                        buf,
                        &mut state.content_rating_list_state,
                    );
                }
                FilterTypes::SortBy => {
                    let list = List::new(vec!["best match", "highest rating"]);
                    StatefulWidget::render(
                        list,
                        current_filter_area,
                        buf,
                        &mut state.content_rating_list_state,
                    );
                }
            }
        }
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
