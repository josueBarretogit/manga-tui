use crate::backend::{ChapterResponse, Languages};
use ratatui::{prelude::*, widgets::*};
use tui_widget_list::PreRender;

#[derive(Clone)]
pub struct ChapterItem {
    pub id: String,
    title: String,
    chapter_number: String,
    is_read: bool,
    translated_language: String,
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

        let [title_area, chapter_number_area, translated_language_area] = layout.areas(area);

        Paragraph::new(self.title)
            .style(self.style)
            .render(title_area, buf);

        let translated_language: Languages = self.translated_language.as_str().into();
        Paragraph::new(self.chapter_number).render(chapter_number_area, buf);
        Paragraph::new(translated_language.to_string()).render(translated_language_area, buf);
    }
}

impl PreRender for ChapterItem {
    fn pre_render(&mut self, context: &tui_widget_list::PreRenderContext) -> u16 {
        if context.is_selected {
            self.style = Style::new().bg(Color::Blue);
        }
        2
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
