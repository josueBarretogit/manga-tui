use self::text::ToSpan;

use super::StatefulWidgetFrame;
use crate::utils::{centered_rect, render_search_bar};
use ratatui::{prelude::*, widgets::*};
use state::*;

pub mod state;

pub struct FilterWidget<'a> {
    pub block: Option<Block<'a>>,
    pub style: Style,
}

impl From<MangaFilters> for Line<'_> {
    fn from(value: MangaFilters) -> Self {
        Line::from(value.to_string())
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

impl From<ListItemId> for ListItem<'_> {
    fn from(value: ListItemId) -> Self {
        let line = if value.is_selected {
            Line::from(format!("🟡 {} ", value.name)).fg(Color::Yellow)
        } else {
            Line::from(value.name)
        };
        ListItem::new(line)
    }
}

impl<'a> StatefulWidgetFrame for FilterWidget<'a> {
    type State = FilterState;

    fn render(
        &mut self,
        area: ratatui::prelude::Rect,
        frame: &mut Frame<'_>,
        state: &mut Self::State,
    ) {
        let buf = frame.buffer_mut();
        let popup_area = centered_rect(area, 80, 70);

        Clear.render(popup_area, buf);

        if let Some(block) = self.block.as_ref() {
            block.render(popup_area, buf);
        }

        let [tabs_area, current_filter_area] =
            Layout::vertical([Constraint::Percentage(20), Constraint::Percentage(80)])
                .margin(2)
                .areas(popup_area);

        let tabs: Vec<Line<'_>> = FILTERS
            .iter()
            .map(|filters| {
                let num_filters = match filters {
                    MangaFilters::ContentRating => state.content_rating.num_filters_active(),
                    MangaFilters::SortBy => state.sort_by_state.num_filters_active(),
                    MangaFilters::Languages => state.lang_state.num_filters_active(),
                    MangaFilters::PublicationStatus => {
                        state.publication_status.num_filters_active()
                    }
                    MangaFilters::MagazineDemographic => {
                        state.magazine_demographic.num_filters_active()
                    }

                    _ => 1,
                };

                Line::from(vec![
                    filters.to_string().into(),
                    " ".into(),
                    if num_filters != 0 {
                        Span::raw(format!("{}+", num_filters))
                            .bold()
                            .underlined()
                            .style(Color::Yellow)
                    } else {
                        "".into()
                    },
                ])
            })
            .collect();

        Tabs::new(tabs)
            .select(state.id_filter)
            .highlight_style(Style::default().fg(Color::Yellow))
            .render(tabs_area, buf);

        if let Some(filter) = FILTERS.get(state.id_filter) {
            match filter {
                MangaFilters::PublicationStatus => {
                    render_filter_list(
                        state.publication_status.items.clone(),
                        current_filter_area,
                        buf,
                        &mut state.publication_status.state,
                    );
                }
                MangaFilters::ContentRating => {
                    render_filter_list(
                        state.content_rating.items.clone(),
                        current_filter_area,
                        buf,
                        &mut state.content_rating.state,
                    );
                }
                MangaFilters::SortBy => {
                    render_filter_list(
                        state.sort_by_state.items.clone(),
                        current_filter_area,
                        buf,
                        &mut state.sort_by_state.state,
                    );
                }
                MangaFilters::Tags => {
                    let [list_area, input_area] =
                        Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)])
                            .areas(current_filter_area);

                    if let Some(tags) = state.tags.items.as_ref().cloned() {
                        if state.tags.is_search_bar_empty() {
                            render_filter_list(tags, list_area, buf, &mut state.tags.state);
                        } else {
                            let filtered_tags: Vec<ListItemId> = tags
                                .iter()
                                .filter_map(|tag| {
                                    if tag
                                        .name
                                        .to_lowercase()
                                        .contains(&state.tags.search_bar.value().to_lowercase())
                                    {
                                        return Some(tag.clone());
                                    }
                                    None
                                })
                                .collect();

                            render_filter_list(
                                filtered_tags,
                                list_area,
                                buf,
                                &mut state.tags.state,
                            );
                        }
                        let input_help = if state.is_typing {
                            Line::from(vec![
                                "Press ".into(),
                                " <esc> ".bold().yellow(),
                                "to stop typing".into(),
                            ])
                        } else {
                            Line::from(vec![
                                "Press".into(),
                                " <l> ".bold().yellow(),
                                "to filter tags".into(),
                            ])
                        };

                        render_search_bar(
                            state.is_typing,
                            input_help,
                            &state.tags.search_bar,
                            frame,
                            input_area,
                        );
                    }
                }
                MangaFilters::MagazineDemographic => {
                    render_filter_list(
                        state.magazine_demographic.items.clone(),
                        current_filter_area,
                        buf,
                        &mut state.magazine_demographic.state,
                    );
                }
                MangaFilters::Authors => {
                    let [list_area, input_area] =
                        Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)])
                            .areas(current_filter_area);

                    match state.author_state.items.as_mut() {
                        Some(authors) => {
                            render_filter_list(
                                authors.clone(),
                                list_area,
                                buf,
                                &mut state.author_state.state,
                            );
                        }
                        None => {
                            state.author_state.message.clone().render(list_area, buf);
                        }
                    }

                    let input_help = if state.is_typing {
                        Line::from(vec![
                            "Press ".into(),
                            " <Enter> ".bold().yellow(),
                            "to search ".into(),
                            " <Esc> ".bold().yellow(),
                            "to stop typing".into(),
                        ])
                    } else {
                        Line::from(vec![
                            "Press".into(),
                            " <l> ".bold().yellow(),
                            "to search authors".into(),
                        ])
                    };

                    render_search_bar(
                        state.is_typing,
                        input_help,
                        &state.author_state.search_bar,
                        frame,
                        input_area,
                    );
                }
                MangaFilters::Artists => {
                    let [list_area, input_area] =
                        Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)])
                            .areas(current_filter_area);

                    match state.artist_state.items.as_mut() {
                        Some(authors) => {
                            render_filter_list(
                                authors.clone(),
                                list_area,
                                buf,
                                &mut state.artist_state.state,
                            );
                        }
                        None => {
                            state.artist_state.message.clone().render(list_area, buf);
                        }
                    }

                    let input_help = if state.is_typing {
                        Line::from(vec![
                            "Press ".into(),
                            " <Enter> ".bold().yellow(),
                            "to search ".into(),
                            " <Esc> ".bold().yellow(),
                            "to stop typing".into(),
                        ])
                    } else {
                        Line::from(vec![
                            "Press".into(),
                            " <l> ".bold().yellow(),
                            "to search artists".into(),
                        ])
                    };

                    render_search_bar(
                        state.is_typing,
                        input_help,
                        &state.artist_state.search_bar,
                        frame,
                        input_area,
                    );
                }
                MangaFilters::Languages => {
                    render_filter_list(
                        state.lang_state.items.clone(),
                        current_filter_area,
                        buf,
                        &mut state.lang_state.state,
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

fn render_filter_list<'a, T>(items: T, area: Rect, buf: &mut Buffer, state: &mut ListState)
where
    T: IntoIterator,
    T::Item: Into<ListItem<'a>>,
{
    let list_block = Block::bordered().title(Line::from(vec![
        " Up/Down ".into(),
        " <j>/<k> ".bold().yellow(),
        " Select ".into(),
        "<s>".bold().yellow(),
    ]));

    let list = List::new(items)
        .block(list_block)
        .highlight_spacing(HighlightSpacing::Always)
        .highlight_symbol(">> ");

    StatefulWidget::render(list, area, buf, state);
}
