use ratatui::{prelude::*, widgets::*};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};

use crate::backend::{Data, SearchMangaResponse};
use crate::utils::set_tags_style;

#[derive(Clone)]
pub struct CarrouselItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub content_rating: String,
    pub status: String,
    pub img_url: Option<String>,
    pub author: Option<String>,
    pub artist: Option<String>,
    pub style: Style,
    pub cover_state: Option<Box<dyn StatefulProtocol>>,
}

impl CarrouselItem {
    fn new(
        id: String,
        title: String,
        description: String,
        tags: Vec<String>,
        content_rating: String,
        status: String,
        img_url: Option<String>,
        author: Option<String>,
        artist: Option<String>,
        style: Style,
        cover_state: Option<Box<dyn StatefulProtocol>>,
    ) -> Self {
        Self {
            id,
            title,
            description,
            tags,
            content_rating,
            status,
            img_url,
            author,
            artist,
            style,
            cover_state,
        }
    }

    fn render_cover(&mut self, area: Rect, buf: &mut Buffer) {
        match self.cover_state {
            Some(ref mut image_state) => {
                let cover = StatefulImage::new(None).resize(Resize::Fit(None));

                StatefulWidget::render(cover, area, buf, image_state)
            }
            None => {
                //Loading cover
                Block::bordered().title("Loading cover").render(area, buf);
            }
        };
    }
    fn render_details(&mut self, area: Rect, buf: &mut Buffer) {
        let layout =
            Layout::vertical([Constraint::Percentage(20), Constraint::Percentage(80)]).margin(1);

        let [tags_area, description_area] = layout.areas(area);

        Block::bordered()
            .title(self.title.clone())
            .title_bottom(self.author.as_deref().unwrap_or_default())
            .render(area, buf);

        let mut tags: Vec<Span<'_>> = self.tags.iter().map(|tag| set_tags_style(tag)).collect();

        tags.push(set_tags_style(&self.status));
        tags.push(set_tags_style(&self.content_rating));

        Paragraph::new(Line::from(tags)).render(tags_area, buf);

        Paragraph::new(self.description.clone())
            .wrap(Wrap { trim: true })
            .render(description_area, buf);
    }

    pub fn from_response(value: Data) -> Self {
        let id = value.id.clone();
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

        let content_rating = value.attributes.content_rating;

        let tags: Vec<String> = value
            .attributes
            .tags
            .iter()
            .map(|tag| tag.attributes.name.en.to_string())
            .collect();

        let mut img_url: Option<String> = Option::default();
        let mut author: Option<String> = Option::default();
        let mut artist: Option<String> = Option::default();

        for rel in &value.relationships {
            if let Some(attributes) = &rel.attributes {
                match rel.type_field.as_str() {
                    "author" => author = Some(attributes.name.as_ref().unwrap().to_string()),
                    "artist" => artist = Some(attributes.name.as_ref().unwrap().to_string()),
                    "cover_art" => {
                        img_url = Some(attributes.file_name.as_ref().unwrap().to_string())
                    }
                    _ => {}
                }
            }
        }

        let status = value.attributes.status;

        Self::new(
            id,
            title,
            description,
            tags,
            content_rating,
            status,
            img_url,
            author,
            artist,
            Style::default(),
            None,
        )
    }
}

impl Widget for CarrouselItem {
    fn render(mut self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)]);

        let [cover_area, details_area] = layout.areas(area);

        self.render_cover(cover_area, buf);
        self.render_details(details_area, buf);
    }
}

#[derive(Default, Clone)]
pub struct Carrousel {
    pub items: Vec<CarrouselItem>,
    pub current_item_visible_index: usize,
}

impl StatefulWidget for Carrousel {
    type State = usize;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let layout = Layout::vertical([Constraint::Percentage(80), Constraint::Percentage(20)]);
        let [items_area, helper_area] = layout.areas(area);

        match self.items.get(*state) {
            Some(item) => item.clone().render(items_area, buf),
            None => Block::bordered().title("loading").render(area, buf),
        };

        Span::raw(format!(
            "Next  <w> | previous  <b> | read <r>  No.{}  Total : {}",
            self.current_item_visible_index + 1,
            self.items.len()
        ))
        .render(helper_area, buf);
    }
}

impl Carrousel {
    pub fn from_response(response: SearchMangaResponse) -> Self {
        let mut items: Vec<CarrouselItem> = vec![];

        for manga in response.data {
            items.push(CarrouselItem::from_response(manga))
        }

        Self {
            items,
            current_item_visible_index: 0,
        }
    }
    pub fn next(&mut self) {
        if self.current_item_visible_index + 1 >= self.items.len() {
            self.current_item_visible_index = 0
        } else {
            self.current_item_visible_index += 1;
        }
    }

    pub fn previous(&mut self) {
        if self.current_item_visible_index == 0 {
            self.current_item_visible_index = self.items.len() - 1
        } else {
            self.current_item_visible_index = self.current_item_visible_index.saturating_sub(1)
        }
    }

    pub fn get_current_item(&mut self) -> Option<&CarrouselItem> {
        self.items.get(self.current_item_visible_index)
    }
}
