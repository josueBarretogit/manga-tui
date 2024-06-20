use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{Block, List, ListDirection, ListItem, ListState, StatefulWidget, Widget};

use crate::backend::SearchMangaResponse;

pub struct MangaItem {
    title: String,
    is_selected: bool,
}

impl MangaItem {
    pub fn new(title: String) -> Self {
        Self {
            title,
            is_selected: false,
        }
    }
    pub fn from_response(response: &SearchMangaResponse) -> Vec<Self> {
        let mut new_manga_list: Vec<Self> = vec![];
        for mangas in response.data.iter() {
            new_manga_list.push(Self::new(mangas.attributes.title.en.clone()));
        }
        new_manga_list
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

pub struct ListMangasFoundWidget {
    items: Vec<MangaItem>,
}

impl ListMangasFoundWidget {
    pub fn new(items: Vec<MangaItem>) -> Self {
        Self { items }
    }
}

impl StatefulWidget for ListMangasFoundWidget {
    type State = ListState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let list = List::new(self.items)
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
