use crate::backend::Data;
use ratatui::{prelude::*, widgets::*};
use ratatui_image::Resize;
use tui_widget_list::PreRender;

use super::{ThreadImage, ThreadProtocol};

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

        let inner = details_area.inner(layout::Margin {
            horizontal: 1,
            vertical: 2,
        });

        Paragraph::new(self.description)
            .wrap(Wrap { trim: true })
            .render(inner, buf);
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
        // Todo! maybe there is a better way to do this
        let title = value.attributes.title.en.unwrap_or(
            value.attributes.title.ja_ro.unwrap_or(
                value.attributes.title.ja.unwrap_or(
                    value.attributes.title.jp.unwrap_or(
                        value.attributes.title.zh.unwrap_or(
                            value
                                .attributes
                                .title
                                .ko
                                .unwrap_or(value.attributes.title.ko_ro.unwrap_or_default()),
                        ),
                    ),
                ),
            ),
        );

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
