use crate::backend::Data;
use ratatui::{prelude::*, widgets::*};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};
use tui_widget_list::PreRender;

pub struct MangaPreview {
    title: String,
    description: String,
    tags: Vec<String>,
    content_rating: String,
}

impl MangaPreview {
    pub fn new(
        title: String,
        description: String,
        mut tags: Vec<String>,
        content_rating: String,
    ) -> Self {
        tags.push(content_rating.clone());
        Self {
            title,
            description,
            tags,
            content_rating,
        }
    }

    pub fn render_cover_and_details_area(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut Option<Box<dyn StatefulProtocol>>,
    ) {
        let layout = Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)]);
        let [cover_area, tags_area] = layout.areas(area);

        let tags_list: Vec<ListItem<'_>> = self
            .tags
            .iter()
            .map(|tag| match tag.to_lowercase().as_str() {
                "suggestive" => ListItem::new(tag.to_string().yellow()),
                "erotica" | "pornographic" | "sexual violence" => {
                    ListItem::new(tag.to_string().red())
                }
                _ => ListItem::new(tag.to_string()),
            })
            .collect();

        Widget::render(List::new(tags_list), tags_area, buf);
        match state {
            Some(image_state) => {
                let cover = StatefulImage::new(None).resize(Resize::Fit(None));
                StatefulWidget::render(cover, cover_area, buf, image_state)
            }
            None => {
                //Loading cover
                Block::bordered()
                    .title("Loading cover")
                    .render(cover_area, buf);
            }
        };
    }

    pub fn render_description_area(self, area: Rect, buf: &mut Buffer) {
        // Manga details
        Block::bordered().title(self.title).render(area, buf);

        let inner = area.inner(layout::Margin {
            horizontal: 1,
            vertical: 2,
        });

        Paragraph::new(self.description)
            .wrap(Wrap { trim: true })
            .render(inner, buf);
    }
}

impl StatefulWidget for MangaPreview {
    type State = Option<Box<dyn StatefulProtocol>>;

    fn render(mut self, area: ratatui::prelude::Rect, buf: &mut Buffer, state: &mut Self::State) {
        let layout = Layout::default()
            .direction(layout::Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)]);

        let [cover_details_area, description_area] = layout.areas(area);

        self.render_cover_and_details_area(cover_details_area, buf, state);
        self.render_description_area(description_area, buf);
    }
}

#[derive(Default, Clone)]
pub struct MangaItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub content_rating: String,
    pub img_url: Option<String>,
    pub style: Style,
    pub image_state: Option<Box<dyn StatefulProtocol>>,
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

        let content_rating = value.attributes.content_rating;

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

        Self::new(id, title, description, tags, content_rating, img_url)
    }
}

impl MangaItem {
    pub fn new(
        id: String,
        title: String,
        description: String,
        tags: Vec<String>,
        content_rating: String,
        img_url: Option<String>,
    ) -> Self {
        Self {
            id,
            title,
            description,
            tags,
            img_url,
            content_rating,
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
