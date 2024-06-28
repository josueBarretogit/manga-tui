use crate::backend::Data;
use ratatui::{prelude::*, widgets::*};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::Resize;
use std::sync::mpsc::Sender;
use tui_widget_list::PreRender;

pub struct MangaPreview {
    title: String,
    description: String,
    categories: Vec<String>,
}

impl MangaPreview {
    pub fn new(title: String, description: String, categories: Vec<String>) -> Self {
        Self {
            title,
            description,
            categories,
        }
    }
}

impl StatefulWidget for MangaPreview {
    type State = Option<ThreadProtocol>;

    fn render(self, area: ratatui::prelude::Rect, buf: &mut Buffer, state: &mut Self::State) {
        let layout = Layout::default()
            .direction(layout::Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)]);

        let [cover_area, details_area] = layout.areas(area);

        match state {
            Some(image_state) => {
                let cover = ThreadImage::new().resize(Resize::Fit(None));
                StatefulWidget::render(cover, cover_area, buf, image_state)
            }
            None => {
                //Loading cover
                Block::bordered().render(cover_area, buf);
            }
        };

        // Manga details
        Block::bordered()
            .title(self.title)
            .render(details_area, buf);

        let inner = details_area.inner(&layout::Margin {
            horizontal: 1,
            vertical: 2,
        });

        Paragraph::new(self.description)
            .wrap(Wrap { trim: true })
            .render(inner, buf);
    }
}

/// A widget that uses a custom ThreadProtocol as state to offload resizing and encoding to a
/// background thread.
pub struct ThreadImage {
    resize: Resize,
}

impl ThreadImage {
    fn new() -> ThreadImage {
        ThreadImage {
            resize: Resize::Fit(None),
        }
    }

    pub fn resize(mut self, resize: Resize) -> ThreadImage {
        self.resize = resize;
        self
    }
}

impl StatefulWidget for ThreadImage {
    type State = ThreadProtocol;

    fn render(
        self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::buffer::Buffer,
        state: &mut Self::State,
    ) {
        state.inner = match state.inner.take() {
            // We have the `protocol` and should either resize or render.
            Some(mut protocol) => {
                // If it needs resizing (grow or shrink) then send it away instead of rendering.
                if let Some(rect) = protocol.needs_resize(&self.resize, area) {
                    state.tx.send((protocol, self.resize, rect)).unwrap();
                    None
                } else {
                    protocol.render(area, buf);
                    Some(protocol)
                }
            }
            // We are waiting to get back the protocol.
            None => None,
        };
    }
}

/// The state of a ThreadImage.
///
/// Has `inner` [ResizeProtocol] that is sent off to the `tx` mspc channel to do the
/// `resize_encode()` work.
#[derive(Clone)]
pub struct ThreadProtocol {
    pub inner: Option<Box<dyn StatefulProtocol>>,
    pub tx: Sender<(Box<dyn StatefulProtocol>, Resize, ratatui::prelude::Rect)>,
}

impl ThreadProtocol {
    pub fn new(
        tx: Sender<(Box<dyn StatefulProtocol>, Resize, ratatui::prelude::Rect)>,
        inner: Box<dyn StatefulProtocol>,
    ) -> ThreadProtocol {
        ThreadProtocol {
            inner: Some(inner),
            tx,
        }
    }
}

#[derive(Default, Clone)]
pub struct MangaItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub img_url: Option<String>,
    pub style: Style,
    pub image_state: Option<ThreadProtocol>,
}

impl Widget for MangaItem {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        Paragraph::new(self.title)
            .block(Block::default().borders(Borders::BOTTOM))
            .style(self.style)
            .render(area, buf);
    }
}

impl PreRender for MangaItem {
    fn pre_render(&mut self, context: &tui_widget_list::PreRenderContext) -> u16 {
        if context.is_selected {
            self.style = Style::default()
                .bg(Color::Rgb(255, 153, 0))
                .fg(Color::Rgb(28, 28, 32));
        }
        2
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
            image_state: None,
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

    pub fn loading() {}

    pub fn not_found() {
        ListMangasFoundWidget::default();
    }
}

impl StatefulWidgetRef for ListMangasFoundWidget {
    type State = tui_widget_list::ListState;
    fn render_ref(&self, area: ratatui::prelude::Rect, buf: &mut Buffer, state: &mut Self::State) {
        let list = tui_widget_list::List::new(self.mangas.clone());
        StatefulWidget::render(list, area, buf, state);
    }
}
