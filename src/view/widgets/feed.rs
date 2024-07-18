use crate::backend::ChapterResponse;
use crate::utils::display_dates_since_publication;
use ratatui::{prelude::*, widgets::*};
use tui_widget_list::PreRender;

pub enum FeedTabs {
    History,
    PlantToRead,
}

#[derive(Clone)]
pub struct RecentChapters {
    pub title: String,
    pub number: String,
    pub readeable_at: String,
}

impl From<RecentChapters> for ListItem<'_> {
    fn from(value: RecentChapters) -> Self {
        let line = Line::from(vec![
            format!("Ch. {} ", value.number).into(),
            value.title.bold(),
            " | ".into(),
            value.readeable_at.into(),
        ]);

        ListItem::new(line)
    }
}

#[derive(Clone)]
pub struct MangasRead {
    pub id: String,
    pub title: String,
    pub style: Style,
    pub recent_chapters: Vec<RecentChapters>,
}

impl Widget for MangasRead {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(2)]);

        let [title_area, recent_chapters_area] = layout.margin(1).areas(area);

        Block::bordered().style(self.style).render(area, buf);

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
            self.style = Style::default().fg(Color::Yellow);
        }
        10
    }
}

#[derive(Clone)]
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
        if self.page as f64 != (self.total_results as f64 / 5_f64).ceil() && !self.mangas.is_empty()
        {
            self.page += 1
        }
    }

    pub fn previous_page(&mut self) {
        if self.page != 1 && !self.mangas.is_empty() {
            self.page -= 1;
        }
    }

    pub fn set_chapter(&mut self, manga_id: String, response: ChapterResponse) {
        if let Some(manga) = self.mangas.iter_mut().find(|manga| manga.id == manga_id) {
            for chapter in response.data {
                let today = chrono::offset::Local::now().date_naive();
                let parse_date =
                    chrono::DateTime::parse_from_rfc3339(&chapter.attributes.readable_at)
                        .unwrap_or_default();

                let difference = today - parse_date.date_naive();

                let num_days = difference.num_days();

                let recent_chapter = RecentChapters {
                    title: chapter.attributes.title.unwrap_or("No title ".to_string()),
                    number: chapter.attributes.chapter.unwrap_or_default(),
                    readeable_at: display_dates_since_publication(num_days),
                };
                manga.recent_chapters.push(recent_chapter);
            }
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
    fn render(
        mut self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
        state: &mut Self::State,
    ) {
        let layout = Layout::vertical([Constraint::Percentage(10), Constraint::Percentage(90)]);
        let [total_results_area, list_area] = layout.areas(area);

        self.render_pagination_data(total_results_area, buf);
        let list = tui_widget_list::List::new(self.mangas);
        StatefulWidget::render(list, list_area, buf, state);
    }
}
