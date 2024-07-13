use crate::backend::database::get_manga_history;
use crate::backend::{ChapterResponse, Languages};
use ratatui::{prelude::*, widgets::*};
use tui_widget_list::PreRender;

#[derive(Clone)]
pub struct ChapterItem {
    pub id: String,
    pub title: String,
    pub chapter_number: String,
    pub is_read: bool,
    pub is_downlowaded: bool,
    pub translated_language: String,
    style: Style,
}

impl Widget for ChapterItem {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::horizontal([
            Constraint::Percentage(60),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ]);

        Block::bordered().border_style(self.style).render(area, buf);

        let [title_area, chapter_number_area, translated_language_area] =
            layout.areas(area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            }));

        let translated_language: Languages = self.translated_language.as_str().into();

        let is_read_icon = if self.is_read {
            "👀".to_string()
        } else {
            "".to_string()
        };

        Paragraph::new(Line::from(vec![
            is_read_icon.into(),
            " ".into(),
            translated_language.to_string().into(),
            format!(" Ch. {} ", self.chapter_number).into(),
            self.title.into(),
            // after this goes the user group,
            // when it was uploaded
            // and if the chapters has been downloaded by user
        ]))
        .style(self.style)
        .render(title_area, buf);
    }
}

impl PreRender for ChapterItem {
    fn pre_render(&mut self, context: &tui_widget_list::PreRenderContext) -> u16 {
        if context.is_selected {
            self.style = Style::new().fg(Color::Yellow);
        }
        3
    }
}

impl ChapterItem {
    pub fn new(
        id: String,
        title: String,
        chapter_number: String,
        is_read: bool,
        translated_language: String,
    ) -> Self {
        Self {
            id,
            title,
            chapter_number,
            is_read,
            is_downlowaded: false,
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

            let translated_language = chapter.attributes.translated_language.clone();

            chapters.push(ChapterItem::new(
                id,
                title,
                chapter_number,
                false,
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
