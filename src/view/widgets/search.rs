use std::io::Cursor;

use bytes::Bytes;
use ratatui::buffer::Buffer;
use ratatui::layout::{self, Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::{
    Block, List, ListDirection, ListItem, ListState, Paragraph, StatefulWidget, StatefulWidgetRef,
    Widget, Wrap,
};
use ratatui_image::picker::Picker;
use ratatui_image::StatefulImage;
use tui_widget_list::PreRender;

use crate::backend::Data;

#[derive(Default, Clone)]
pub struct MangaItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub img_url: Option<String>,
    pub img_bytes: Option<Bytes>,
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

        if let Some(bytes) = self.img_bytes {
            let mut picker = Picker::from_termios().unwrap();
            // Guess the protocol.
            picker.guess_protocol();

            // Load an image with the image crate.

            let dyn_img = image::io::Reader::new(Cursor::new(bytes))
                .with_guessed_format()
                .unwrap();

            // Create the Protocol which will be used by the widget.
            let mut image = picker.new_resize_protocol(dyn_img.decode().unwrap());

            let image_wid = StatefulImage::new(None);

            StatefulWidget::render(image_wid, cover_area, buf, &mut image);
        }

        Block::bordered()
            .title(self.title)
            .style(self.style)
            .render(manga_details_area, buf);

        let inner = manga_details_area.inner(&layout::Margin {
            horizontal: 1,
            vertical: 1,
        });

        Paragraph::new(self.img_url.unwrap_or_default())
            .wrap(Wrap { trim: true })
            .render(inner, buf);
    }
}

impl PreRender for MangaItem {
    fn pre_render(&mut self, context: &tui_widget_list::PreRenderContext) -> u16 {
        if context.is_selected {
            self.style = Style::default()
                .bg(Color::Rgb(255, 153, 0))
                .fg(Color::Rgb(28, 28, 32));
        }
        15
    }
}

impl From<Data> for MangaItem {
    fn from(value: Data) -> Self {
        let id = value.id;
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

        Self::new(id, title, description, tags, img_url)
    }
}

impl MangaItem {
    pub fn new(
        id: String,
        title: String,
        description: String,
        tags: Vec<String>,
        img_url: Option<String>,
    ) -> Self {
        Self {
            id,
            title,
            description,
            tags,
            img_url,
            img_bytes: None,
            style: Style::default(),
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
