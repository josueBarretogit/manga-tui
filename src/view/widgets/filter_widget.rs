use crate::filter::{ContentRating, Filters, SortBy};
use crate::utils::centered_rect;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use strum::{Display, IntoEnumIterator};

#[derive(Display, PartialEq, Eq)]
enum MangaFilters {
    #[strum(to_string = "Content rating")]
    ContentRating,
    #[strum(to_string = "Sort by")]
    SortBy,
}

impl From<MangaFilters> for Line<'_> {
    fn from(value: MangaFilters) -> Self {
        Line::from(value.to_string())
    }
}

const FILTERS: [MangaFilters; 2] = [MangaFilters::ContentRating, MangaFilters::SortBy];

#[derive(Clone)]
pub struct FilterListItem {
    pub is_selected: bool,
    pub name: String,
}

impl FilterListItem {
    pub fn toggle(&mut self) {
        self.is_selected = !self.is_selected;
    }
}

pub struct ContentRatingState {
    pub items: Vec<FilterListItem>,
    pub state: ListState,
}

impl ContentRatingState {
    pub fn toggle(&mut self) {
        if let Some(index) = self.state.selected() {
            if let Some(content_rating) = self.items.get_mut(index) {
                content_rating.toggle();
            }
        }
    }
}

impl Default for ContentRatingState {
    fn default() -> Self {
        Self {
            items: vec![
                FilterListItem {
                    is_selected: true,
                    name: ContentRating::Safe.to_string(),
                },
                FilterListItem {
                    is_selected: true,
                    name: ContentRating::Suggestive.to_string(),
                },
                FilterListItem {
                    is_selected: false,
                    name: ContentRating::Erotic.to_string(),
                },
                FilterListItem {
                    is_selected: false,
                    name: ContentRating::Pornographic.to_string(),
                },
            ],
            state: ListState::default(),
        }
    }
}

pub struct SortByState {
    pub items: Vec<FilterListItem>,
    pub state: ListState,
}

impl Default for SortByState {
    fn default() -> Self {
        let sort_by_items = SortBy::iter().map(|sort_by_elem| FilterListItem {
            is_selected: sort_by_elem == SortBy::BestMatch,
            name: sort_by_elem.to_string(),
        });

        Self {
            items: sort_by_items.collect(),
            state: ListState::default(),
        }
    }
}

impl From<FilterListItem> for ListItem<'_> {
    fn from(value: FilterListItem) -> Self {
        let line = if value.is_selected {
            Line::from(format!("🟡 {} ", value.name)).fg(Color::Yellow)
        } else {
            Line::from(value.name)
        };
        ListItem::new(line)
    }
}

#[derive(Default)]
pub struct FilterState {
    pub is_open: bool,
    pub id_filter: usize,
    pub filters: Filters,
    pub content_rating_list_state: ContentRatingState,
}

impl FilterState {
    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    pub fn handle_key_events(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('f') => self.toggle(),
            KeyCode::Esc => self.toggle(),
            KeyCode::Char('j') => self.scroll_down_filter_list(),
            KeyCode::Char('k') => self.scroll_up_filter_list(),
            KeyCode::Tab => self.next_filter(),
            KeyCode::BackTab => self.previous_filter(),
            KeyCode::Char('s') => self.toggle_filter_list(),
            _ => {}
        }
    }

    fn next_filter(&mut self) {
        if self.id_filter + 1 < FILTERS.len() {
            self.id_filter += 1;
        } else {
            self.id_filter = 0;
        }
    }

    fn previous_filter(&mut self) {
        if self.id_filter == 0 {
            self.id_filter = FILTERS.len() - 1;
        } else {
            self.id_filter = self.id_filter.saturating_sub(1);
        }
    }

    fn scroll_down_filter_list(&mut self) {
        if let Some(filter) = FILTERS.get(self.id_filter) {
            match filter {
                MangaFilters::ContentRating => {
                    self.content_rating_list_state.state.select_next();
                }
                MangaFilters::SortBy => {
                    todo!()
                }
            }
        }
    }

    fn scroll_up_filter_list(&mut self) {
        if let Some(filter) = FILTERS.get(self.id_filter) {
            match filter {
                MangaFilters::ContentRating => {
                    self.content_rating_list_state.state.select_previous();
                }
                MangaFilters::SortBy => {
                    todo!()
                }
            }
        }
    }

    fn toggle_filter_list(&mut self) {
        if let Some(filter) = FILTERS.get(self.id_filter) {
            match filter {
                MangaFilters::ContentRating => {
                    self.content_rating_list_state.toggle();
                    self.set_content_rating();
                }
                MangaFilters::SortBy => {
                    todo!()
                }
            }
        }
    }

    fn set_content_rating(&mut self) {
        self.filters.set_content_rating(
            self.content_rating_list_state
                .items
                .iter()
                .filter_map(|item| {
                    if item.is_selected {
                        return Some(item.name.as_str().into());
                    }
                    None
                })
                .collect(),
        )
    }
}

pub struct FilterWidget<'a> {
    pub block: Option<Block<'a>>,
    pub style: Style,
}

impl<'a> StatefulWidget for FilterWidget<'a> {
    type State = FilterState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let popup_area = centered_rect(area, 80, 50);

        Clear.render(popup_area, buf);

        if let Some(block) = self.block {
            block.render(popup_area, buf);
        }

        let [tabs_area, current_filter_area] =
            Layout::vertical([Constraint::Percentage(20), Constraint::Percentage(80)])
                .margin(2)
                .areas(popup_area);

        Tabs::new(FILTERS)
            .select(state.id_filter)
            .highlight_style(Style::default().fg(Color::Yellow))
            .render(tabs_area, buf);

        if let Some(filter) = FILTERS.get(state.id_filter) {
            match filter {
                MangaFilters::ContentRating => {
                    let list_block = Block::bordered().title(Line::from(vec![
                        " Up/Down ".into(),
                        " <j>/<k> ".bold().yellow(),
                        " Select ".into(),
                        "<s>".bold().yellow(),
                    ]));

                    let list = List::new(state.content_rating_list_state.items.clone())
                        .block(list_block)
                        .highlight_symbol(" >> ");

                    StatefulWidget::render(
                        list,
                        current_filter_area,
                        buf,
                        &mut state.content_rating_list_state.state,
                    );
                }
                MangaFilters::SortBy => {
                    let list = List::new(vec!["best match", "highest rating"]);
                    StatefulWidget::render(
                        list,
                        current_filter_area,
                        buf,
                        &mut state.content_rating_list_state.state,
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

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }
}
