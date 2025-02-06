use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, StatefulWidget, Widget, Wrap};
use tui_widget_list::PreRender;

use crate::backend::database::MangaHistoryResponse;
use crate::backend::manga_provider::{Languages, LatestChapter};
use crate::global::CURRENT_LIST_ITEM_STYLE;
use crate::utils::display_dates_since_publication;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FeedTabs {
    History,
    PlantToRead,
}

impl FeedTabs {
    pub fn cycle(self) -> Self {
        match self {
            Self::History => Self::PlantToRead,
            Self::PlantToRead => Self::History,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecentChapters {
    pub id: String,
    pub title: String,
    pub number: String,
    pub translated_language: Languages,
    pub readeable_at: String,
}

impl From<RecentChapters> for ListItem<'_> {
    fn from(value: RecentChapters) -> Self {
        let line = Line::from(vec![
            format!("Ch. {} ", value.number).into(),
            value.title.bold(),
            " | ".into(),
            value.translated_language.as_emoji().into(),
            " ".into(),
            value.translated_language.as_human_readable().into(),
            " | ".into(),
            value.readeable_at.into(),
        ]);

        ListItem::new(line)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MangasRead {
    pub id: String,
    pub title: String,
    pub style: Style,
    pub recent_chapters: Vec<RecentChapters>,
}

impl From<LatestChapter> for RecentChapters {
    fn from(value: LatestChapter) -> Self {
        let id = value.id;
        let today = chrono::offset::Local::now().date_naive();
        let parse_date = chrono::DateTime::parse_from_rfc3339(&value.publication_date).unwrap_or_default();

        let difference = today - parse_date.date_naive();

        let num_days = difference.num_days();

        let translated_language = value.language;

        Self {
            id,
            title: value.title,
            number: value.chapter_number,
            readeable_at: display_dates_since_publication(num_days),
            translated_language,
        }
    }
}

impl Widget for MangasRead {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(2)]);

        let [title_area, recent_chapters_area] = layout.margin(1).areas(area);

        Block::bordered().border_style(self.style).render(area, buf);

        Paragraph::new(self.title)
            .block(Block::default().borders(Borders::RIGHT))
            .wrap(Wrap { trim: true })
            .render(title_area, buf);

        if !self.recent_chapters.is_empty() {
            Widget::render(
                List::new(self.recent_chapters).block(Block::bordered().title("Latest chapters")),
                recent_chapters_area.inner(Margin {
                    horizontal: 1,
                    vertical: 1,
                }),
                buf,
            );
        }
    }
}

impl PreRender for MangasRead {
    fn pre_render(&mut self, context: &tui_widget_list::PreRenderContext) -> u16 {
        if context.is_selected {
            self.style = *CURRENT_LIST_ITEM_STYLE;
        }
        10
    }
}

#[derive(Clone, Debug, Default)]
pub struct HistoryWidget {
    pub page: u32,
    pub total_results: u32,
    pub mangas: Vec<MangasRead>,
    pub state: tui_widget_list::ListState,
}

impl HistoryWidget {
    pub fn select_next(&mut self) {
        self.state.next();
    }

    pub fn select_previous(&mut self) {
        self.state.previous();
    }

    pub fn get_current_manga_selected(&self) -> Option<&MangasRead> {
        match self.state.selected {
            Some(index) => self.mangas.get(index),
            None => None,
        }
    }

    pub fn next_page(&mut self) {
        self.page += 1
    }

    pub fn previous_page(&mut self) {
        self.page -= 1;
    }

    pub fn set_chapter(&mut self, manga_id: String, response: Vec<LatestChapter>) {
        if let Some(manga) = self.mangas.iter_mut().find(|manga| manga.id == manga_id) {
            for chapter in response {
                manga.recent_chapters.push(RecentChapters::from(chapter));
            }
        }
    }

    pub fn can_search_next_page(&self, total_items: f64) -> bool {
        self.page as f64 != (self.total_results as f64 / total_items).ceil() && !self.mangas.is_empty()
    }

    pub fn can_search_previous_page(&self) -> bool {
        !self.mangas.is_empty() && self.page != 1
    }

    pub fn from_database_response(response: MangaHistoryResponse) -> Self {
        Self {
            page: response.page,
            total_results: response.total_items,
            mangas: response
                .mangas
                .iter()
                .map(|history| MangasRead {
                    id: history.id.clone(),
                    title: history.title.clone(),
                    recent_chapters: vec![],
                    style: Style::default(),
                })
                .collect(),
            state: tui_widget_list::ListState::default(),
        }
    }

    fn render_pagination_data(&mut self, area: Rect, buf: &mut Buffer) {
        let amount_pages = self.total_results as f64 / 5_f64;
        Paragraph::new(Line::from(vec![
            "Total results ".into(),
            self.total_results.to_string().into(),
            format!(" page : {} of {} ", self.page, amount_pages.ceil()).into(),
            " Next page: ".into(),
            " <w> ".bold().fg(Color::Yellow),
            " Previous page: ".into(),
            " <b> ".bold().fg(Color::Yellow),
        ]))
        .render(area, buf);
    }
}

impl StatefulWidget for HistoryWidget {
    type State = tui_widget_list::ListState;

    fn render(mut self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let layout = Layout::vertical([Constraint::Percentage(10), Constraint::Percentage(90)]);
        let [total_results_area, list_area] = layout.areas(area);

        self.render_pagination_data(total_results_area, buf);
        let list = tui_widget_list::List::new(self.mangas);
        StatefulWidget::render(list, list_area, buf, state);
    }
}
