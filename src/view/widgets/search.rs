use ratatui::buffer::Buffer;
use ratatui::layout::{self, Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{
    Block, List, ListDirection, ListItem, ListState, Paragraph, StatefulWidget, StatefulWidgetRef,
    Widget,
};
use tui_widget_list::PreRender;

use crate::backend::Data;

#[derive(Default, Clone)]
pub struct MangaItem {
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub img_url: Option<String>,
    pub style: Style,
}

impl Widget for MangaItem {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::default()
            .direction(layout::Direction::Horizontal)
            .constraints([Constraint::Max(20), Constraint::Fill(1)]);

        let [cover_area, manga_details_area] = layout.areas(area);

        Block::bordered().render(cover_area, buf);
        Block::bordered()
            .title(self.title.clone())
            .style(self.style)
            .render(manga_details_area, buf);
    }
}

impl PreRender for MangaItem {
    fn pre_render(&mut self, context: &tui_widget_list::PreRenderContext) -> u16 {
        if context.is_selected {
            self.style = Style::default()
                .bg(Color::Rgb(255, 153, 0))
                .fg(Color::Rgb(28, 28, 32));
        }
        1
    }
}

impl From<Data> for MangaItem {
    fn from(value: Data) -> Self {
        let title = value.attributes.title.en;

        let description = match value.attributes.description {
            Some(description) => description.en.unwrap_or("No description".to_string()),
            None => String::from("No description"),
        };

        let tags: Vec<String> = value
            .attributes
            .tags
            .iter()
            .map(|tag| tag.attributes.name.en.to_string())
            .collect();

        let img_metadata = value
            .relationships
            .iter()
            .find(|relation| relation.attributes.is_some());

        let img_url = match img_metadata {
            Some(data) => data
                .attributes
                .as_ref()
                .map(|cover_img_attributes| cover_img_attributes.file_name.clone()),
            None => None,
        };

        Self::new(title, description, tags, img_url)
    }
}

impl MangaItem {
    pub fn new(
        title: String,
        description: String,
        tags: Vec<String>,
        img_url: Option<String>,
    ) -> Self {
        Self {
            title,
            description,
            tags,
            img_url,
            style : Style::default(),
        }
    }
}

#[derive(Default, Clone)]
pub struct ListMangasFoundWidget {
    pub mangas: Vec<MangaItem>,
}

impl ListMangasFoundWidget {
    pub fn new(items: Vec<MangaItem>) -> Self {
        Self { mangas: items }
    }

    pub fn from_response(search_response: Vec<Data>) -> Self {
        let mut mangas: Vec<MangaItem> = vec![];

        for manga in search_response {
            mangas.push(MangaItem::from(manga));
        }

        Self { mangas }
    }

    pub fn not_found() {
        ListMangasFoundWidget::default();
    }
}

impl StatefulWidgetRef for ListMangasFoundWidget {
    type State = tui_widget_list::ListState;
    fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let list = tui_widget_list::List::new(self.mangas.clone());
        StatefulWidget::render(list, area, buf, state);
    }
}

#[derive(Default)]
pub struct MangaPreview<'a> {
    title: String,
    description: String,
    image_data: Option<&'a [u8]>,
}

impl<'a> MangaPreview<'a> {
    pub fn new(title: String, description: String, image_data: Option<&'a [u8]>) -> Self {
        Self {
            title,
            description,
            image_data,
        }
    }
}

impl Widget for MangaPreview<'_> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let block = Block::bordered().title(self.title);

        Paragraph::new(self.description)
            .block(block)
            .render(area, buf);
    }
}
