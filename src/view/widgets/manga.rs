use crate::backend::filter::Languages;
use crate::backend::ChapterResponse;
use crate::utils::display_dates_since_publication;
use ratatui::{prelude::*, widgets::*};
use tui_widget_list::PreRender;

#[derive(Clone)]
pub struct ChapterItem {
    pub id: String,
    pub title: String,
    pub readable_at: String,
    pub scanlator: String,
    pub chapter_number: String,
    pub is_read: bool,
    pub is_downloaded: bool,
    pub download_loading_state: Option<f64>,
    pub translated_language: Languages,
    style: Style,
}

impl Widget for ChapterItem {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::horizontal([
            Constraint::Percentage(50),
            Constraint::Percentage(30),
            Constraint::Percentage(20),
        ]);

        Block::bordered().border_style(self.style).render(area, buf);

        let [title_area, scanlator_area, readable_at_area] = layout.areas(area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        }));

        let is_read_icon = if self.is_read { "👀" } else { " " };

        let is_downloaded_icon = if self.is_downloaded { "📥" } else { " " };

        Paragraph::new(Line::from(vec![
            is_read_icon.into(),
            " ".into(),
            is_downloaded_icon.into(),
            " ".into(),
            self.translated_language.as_emoji().into(),
            format!(" Ch. {} ", self.chapter_number).into(),
            self.title.into(),
        ]))
        .wrap(Wrap { trim: true })
        .style(self.style)
        .render(title_area, buf);

        match self.download_loading_state.as_ref() {
            Some(progress) => {
                LineGauge::default()
                    .block(Block::bordered().title("Downloading please wait a moment"))
                    .filled_style(
                        Style::default()
                            .fg(Color::Blue)
                            .bg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    )
                    .line_set(symbols::line::THICK)
                    .ratio(*progress)
                    .render(scanlator_area, buf);
            }
            None => {
                Paragraph::new(self.scanlator)
                    .wrap(Wrap { trim: true })
                    .render(scanlator_area, buf);

                Paragraph::new(self.readable_at)
                    .wrap(Wrap { trim: true })
                    .render(readable_at_area, buf);
            }
        }
    }
}

impl PreRender for ChapterItem {
    fn pre_render(&mut self, context: &tui_widget_list::PreRenderContext) -> u16 {
        if context.is_selected {
            self.style = Style::new().fg(Color::Yellow);
        }

        if self.download_loading_state.is_some() {
            5
        } else {
            3
        }
    }
}

impl ChapterItem {
    pub fn new(
        id: String,
        title: String,
        chapter_number: String,
        readable_at: String,
        scanlator: String,
        translated_language: Languages,
    ) -> Self {
        Self {
            id,
            title,
            readable_at,
            scanlator,
            chapter_number,
            is_read: false,
            is_downloaded: false,
            download_loading_state: None,
            translated_language,
            style: Style::default(),
        }
    }
}

#[derive(Clone)]
pub struct ChaptersListWidget {
    pub chapters: Vec<ChapterItem>,
}

impl ChaptersListWidget {
    pub fn from_response(response: &ChapterResponse) -> Self {
        let mut chapters: Vec<ChapterItem> = vec![];

        let today = chrono::offset::Local::now().date_naive();
        for chapter in response.data.iter() {
            let id = chapter.id.clone();
            let title = chapter
                .attributes
                .title
                .clone()
                .unwrap_or("No title".to_string());

            let chapter_number = chapter
                .attributes
                .chapter
                .clone()
                .unwrap_or("0".to_string());

            let translated_language: Languages = chapter
                .attributes
                .translated_language
                .as_str()
                .try_into()
                .unwrap();

            let parse_date = chrono::DateTime::parse_from_rfc3339(&chapter.attributes.readable_at)
                .unwrap_or_default();

            let difference = today - parse_date.date_naive();

            let scanlator = chapter
                .relationships
                .iter()
                .find(|rel| rel.type_field == "scanlation_group")
                .map(|rel| rel.attributes.as_ref().unwrap().name.to_string());

            chapters.push(ChapterItem::new(
                id,
                title,
                chapter_number,
                display_dates_since_publication(difference.num_days()),
                scanlator.unwrap_or_default(),
                translated_language,
            ))
        }

        Self { chapters }
    }
}

impl StatefulWidget for ChaptersListWidget {
    type State = tui_widget_list::ListState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let chapters_list = tui_widget_list::List::new(self.chapters);
        StatefulWidget::render(chapters_list, area, buf, state);
    }
}
