use filter_provider::{FILTERS, FilterListItem, ListItemId, MangaFilters, MangadexFilterProvider, TagListItem, TagListItemState};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, HighlightSpacing, List, ListItem, ListState, Paragraph, StatefulWidget, Tabs, Widget, Wrap};

use super::StatefulWidgetFrame;
use super::filters::*;
use crate::backend::manga_provider::FiltersWidget;
use crate::global::CURRENT_LIST_ITEM_STYLE;
use crate::utils::render_search_bar;

#[derive(Clone)]
pub struct MangadexFilterWidget {
    pub _style: Style,
}

impl FiltersWidget for MangadexFilterWidget {
    type FilterState = MangadexFilterProvider;
}

impl From<MangaFilters> for Line<'_> {
    fn from(value: MangaFilters) -> Self {
        Line::from(value.to_string())
    }
}

impl From<FilterListItem> for ListItem<'_> {
    fn from(value: FilterListItem) -> Self {
        let line =
            if value.is_selected { Line::from(format!("🟡 {} ", value.name)).fg(Color::Yellow) } else { Line::from(value.name) };
        ListItem::new(line)
    }
}

impl From<ListItemId> for ListItem<'_> {
    fn from(value: ListItemId) -> Self {
        let line =
            if value.is_selected { Line::from(format!("🟡 {} ", value.name)).fg(Color::Yellow) } else { Line::from(value.name) };
        ListItem::new(line)
    }
}

impl From<TagListItem> for ListItem<'_> {
    fn from(value: TagListItem) -> Self {
        let line = match value.state {
            TagListItemState::Included => Line::from(format!(" {} ", value.name).black().on_green()),
            TagListItemState::Excluded => Line::from(format!(" {} ", value.name).black().on_red()),
            TagListItemState::NotSelected => Line::from(value.name),
        };

        ListItem::new(line)
    }
}

impl StatefulWidgetFrame for MangadexFilterWidget {
    type State = MangadexFilterProvider;

    fn render(&mut self, area: Rect, frame: &mut Frame<'_>, state: &mut Self::State) {
        let buf = frame.buffer_mut();
        let [tabs_area, current_filter_area] = Layout::vertical([Constraint::Percentage(20), Constraint::Percentage(80)])
            .margin(2)
            .areas(area);

        let tabs: Vec<Line<'_>> = FILTERS
            .iter()
            .map(|filters| {
                let num_filters = match filters {
                    MangaFilters::ContentRating => state.content_rating.num_filters_active(),
                    MangaFilters::SortBy => state.sort_by_state.num_filters_active(),
                    MangaFilters::Languages => state.lang_state.num_filters_active(),
                    MangaFilters::PublicationStatus => state.publication_status.num_filters_active(),
                    MangaFilters::MagazineDemographic => state.magazine_demographic.num_filters_active(),
                    MangaFilters::Tags => state.tags_state.num_filters_active(),
                    MangaFilters::Authors => state.author_state.num_filters_active(),
                    MangaFilters::Artists => state.artist_state.num_filters_active(),
                };

                Line::from(vec![
                    filters.to_string().into(),
                    " ".into(),
                    if num_filters != 0 {
                        Span::raw(format!("{num_filters}+")).bold().underlined().style(Color::Yellow)
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
                },
                MangaFilters::ContentRating => {
                    render_filter_list(
                        state.content_rating.items.clone(),
                        current_filter_area,
                        buf,
                        &mut state.content_rating.state,
                    );
                },
                MangaFilters::SortBy => {
                    render_filter_list(state.sort_by_state.items.clone(), current_filter_area, buf, &mut state.sort_by_state.state);
                },
                MangaFilters::Tags => self.render_tags_list(current_filter_area, frame, state),
                MangaFilters::MagazineDemographic => {
                    render_filter_list(
                        state.magazine_demographic.items.clone(),
                        current_filter_area,
                        buf,
                        &mut state.magazine_demographic.state,
                    );
                },
                MangaFilters::Authors => {
                    let [list_area, input_area] =
                        Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(current_filter_area);

                    match state.author_state.items.as_mut() {
                        Some(authors) => {
                            render_filter_list(authors.clone(), list_area, buf, &mut state.author_state.state);
                        },
                        None => {
                            Paragraph::new("Search authors").render(list_area, buf);
                        },
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
                        Line::from(vec!["Press".into(), " <l> ".bold().yellow(), "to search authors".into()])
                    };

                    render_search_bar(state.is_typing, input_help, &state.author_state.search_bar, frame, input_area);
                },
                MangaFilters::Artists => {
                    let [list_area, input_area] =
                        Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(current_filter_area);

                    match state.artist_state.items.as_mut() {
                        Some(authors) => {
                            render_filter_list(authors.clone(), list_area, buf, &mut state.artist_state.state);
                        },
                        None => {
                            Paragraph::new("Search artist").render(list_area, buf);
                        },
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
                        Line::from(vec!["Press".into(), " <l> ".bold().yellow(), "to search artists".into()])
                    };

                    render_search_bar(state.is_typing, input_help, &state.artist_state.search_bar, frame, input_area);
                },
                MangaFilters::Languages => {
                    render_filter_list(state.lang_state.items.clone(), current_filter_area, buf, &mut state.lang_state.state);
                },
            }
        }
    }
}

impl MangadexFilterWidget {
    pub fn new() -> Self {
        Self {
            _style: Style::default(),
        }
    }

    fn render_tags_list(&mut self, area: Rect, frame: &mut Frame<'_>, state: &mut MangadexFilterProvider) {
        let buf = frame.buffer_mut();
        let [list_area, input_area] = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(area);

        let [input_area, current_tags_area] = Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)]).areas(input_area);

        if let Some(tags) = state.tags_state.tags.as_ref().cloned() {
            let tags_filtered: Vec<Span<'_>> = tags
                .iter()
                .filter(|tag| tag.state != TagListItemState::NotSelected)
                .map(|tag| tag.set_filter_tags_style())
                .collect();

            Paragraph::new(Line::from(tags_filtered))
                .block(Block::bordered())
                .wrap(Wrap { trim: true })
                .render(current_tags_area, buf);

            if state.tags_state.is_filter_empty() {
                render_tags_list(tags, list_area, buf, &mut state.tags_state.state);
            } else {
                let filtered_tags: Vec<TagListItem> = tags
                    .iter()
                    .filter_map(|tag| {
                        if tag.name.to_lowercase().contains(&state.tags_state.filter_input.value().to_lowercase()) {
                            return Some(tag.clone());
                        }
                        None
                    })
                    .collect();

                render_tags_list(filtered_tags, list_area, buf, &mut state.tags_state.state);
            }

            let input_help = if state.is_typing {
                Line::from(vec!["Press ".into(), " <esc> ".bold().yellow(), "to stop typing".into()])
            } else {
                Line::from(vec!["Press".into(), " <l> ".bold().yellow(), "to filter tags".into()])
            };

            render_search_bar(state.is_typing, input_help, &state.tags_state.filter_input, frame, input_area);
        }
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
        .highlight_style(*CURRENT_LIST_ITEM_STYLE);

    StatefulWidget::render(list, area, buf, state);
}

fn render_tags_list<'a, T>(items: T, area: Rect, buf: &mut Buffer, state: &mut ListState)
where
    T: IntoIterator,
    T::Item: Into<ListItem<'a>>,
{
    let list_block = Block::bordered().title(Line::from(vec![
        " Up/Down ".into(),
        " <j>/<k> ".bold().yellow(),
        " toggle include tag ".into(),
        "<s>".bold().yellow(),
        " toggle exclude tag ".into(),
        "<d>".bold().yellow(),
    ]));
    let list = List::new(items)
        .block(list_block)
        .highlight_spacing(HighlightSpacing::Always)
        .highlight_style(*CURRENT_LIST_ITEM_STYLE);
    StatefulWidget::render(list, area, buf, state);
}
