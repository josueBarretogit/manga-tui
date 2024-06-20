use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, List, ListDirection, ListItem, ListState, StatefulWidget, StatefulWidgetRef, Widget};

use crate::backend::{Data};

#[derive(Default, Clone)]
pub struct MangaItem {
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub img_url: Option<String>,
    pub is_selected: bool,
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
            Some(data) => match &data.attributes {
                Some(cover_img_attributes) => Some(cover_img_attributes.file_name.clone()),
                None => None,
            },
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
            is_selected: false,
        }
    }
}

impl From<MangaItem> for ListItem<'_> {
    fn from(val: MangaItem) -> Self {
        let line = if val.is_selected {
            Line::from(val.title.bold().blue())
        } else {
            Line::from(val.title)
        };

        ListItem::new(line)
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
}

impl StatefulWidgetRef for ListMangasFoundWidget {
    type State = ListState;
    fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let list = List::new(self.mangas.clone())
            .block(Block::bordered().title("Mangas found"))
            .highlight_style(Style::default().fg(ratatui::style::Color::Cyan))
            .direction(ListDirection::TopToBottom);

        StatefulWidget::render(list, area, buf, state);
    }
}

#[derive(Default)]
pub struct MangaPreview<'a> {
    title: String,
    description: String,
    image_data: &'a [u8],
}

impl<'a> MangaPreview<'a> {
    pub fn new(title: String, description: String, image_data: &'a [u8]) -> Self {
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
        let block = Block::bordered().title("Preview");

        block.render(area, buf);
    }
}
