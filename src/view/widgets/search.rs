use ratatui::prelude::*;
use ratatui::widgets::*;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};
use throbber_widgets_tui::{Throbber, ThrobberState};
use tui_widget_list::PreRender;

use crate::backend::Data;
use crate::common::Manga;
use crate::global::CURRENT_LIST_ITEM_STYLE;
use crate::utils::{from_manga_response, set_status_style, set_tags_style};
use crate::PICKER;

pub struct MangaPreview<'a> {
    title: &'a str,
    description: &'a str,
    tags: &'a Vec<String>,
    content_rating: &'a str,
    status: &'a str,
    loader_state: ThrobberState,
}

impl<'a> MangaPreview<'a> {
    pub fn new(
        title: &'a str,
        description: &'a str,
        tags: &'a Vec<String>,
        content_rating: &'a str,
        status: &'a str,
        loader_state: ThrobberState,
    ) -> Self {
        Self {
            title,
            description,
            tags,
            content_rating,
            status,
            loader_state,
        }
    }

    pub fn render_cover_and_details_area(&mut self, area: Rect, buf: &mut Buffer, state: &mut Option<Box<dyn StatefulProtocol>>) {
        let layout = Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(80)]);
        let [cover_area, details_area] = layout.areas(area);

        self.render_details(details_area, buf);

        if PICKER.is_some() {
            match state {
                Some(image_state) => {
                    let cover = StatefulImage::new(None).resize(Resize::Fit(None));
                    StatefulWidget::render(cover, cover_area, buf, image_state)
                },
                None => {
                    Block::bordered().render(cover_area, buf);
                    let loader = Throbber::default()
                        .label("Loading cover")
                        .throbber_set(throbber_widgets_tui::BRAILLE_SIX)
                        .use_type(throbber_widgets_tui::WhichUse::Spin);

                    StatefulWidget::render(
                        loader,
                        cover_area.inner(Margin {
                            horizontal: 2,
                            vertical: 2,
                        }),
                        buf,
                        &mut self.loader_state,
                    );
                },
            };
        }
    }

    pub fn render_description_area(self, area: Rect, buf: &mut Buffer) {
        Block::bordered().title(self.title).render(area, buf);

        let inner = area.inner(layout::Margin {
            horizontal: 1,
            vertical: 2,
        });

        Paragraph::new(self.description).wrap(Wrap { trim: true }).render(inner, buf);
    }

    pub fn render_details(&mut self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::vertical([Constraint::Percentage(20), Constraint::Percentage(80)]);
        let [details_area, tags_area] = layout.areas(area);

        let tags_list: Vec<Span<'_>> = self.tags.iter().map(|tag| set_tags_style(tag)).collect();

        let content_rating = set_tags_style(self.content_rating);

        let status = set_status_style(self.status);

        Paragraph::new(Line::from(vec![content_rating, status])).render(details_area, buf);

        Paragraph::new(Line::from(tags_list)).wrap(Wrap { trim: true }).render(tags_area, buf);
    }
}

impl<'a> StatefulWidget for MangaPreview<'a> {
    type State = Option<Box<dyn StatefulProtocol>>;

    fn render(mut self, area: ratatui::prelude::Rect, buf: &mut Buffer, state: &mut Self::State) {
        let [cover_details_area, description_area] =
            Layout::vertical([Constraint::Percentage(40), Constraint::Percentage(60)]).areas(area);

        self.render_cover_and_details_area(cover_details_area, buf, state);
        self.render_description_area(description_area, buf);
    }
}

#[derive(Default, Clone)]
pub struct MangaItem {
    pub manga: Manga,
    pub style: Style,
    pub image_state: Option<Box<dyn StatefulProtocol>>,
}

impl Widget for MangaItem {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        Paragraph::new(self.manga.title)
            .wrap(Wrap { trim: true })
            .style(self.style)
            .render(area, buf);
    }
}

impl PreRender for MangaItem {
    fn pre_render(&mut self, context: &tui_widget_list::PreRenderContext) -> u16 {
        if context.is_selected {
            self.style = *CURRENT_LIST_ITEM_STYLE;
        }

        1
    }
}

impl From<Data> for MangaItem {
    fn from(value: Data) -> Self {
        let manga = from_manga_response(value);
        Self::new(manga, None)
    }
}

impl MangaItem {
    pub fn new(manga: Manga, image_state: Option<Box<dyn StatefulProtocol>>) -> Self {
        Self {
            manga,
            image_state,
            style: Style::default(),
        }
    }
}

#[derive(Default, Clone)]
pub struct ListMangasFoundWidget {
    pub mangas: Vec<MangaItem>,
}

impl ListMangasFoundWidget {
    pub fn from_response(search_response: Vec<Data>) -> Self {
        let mut mangas: Vec<MangaItem> = vec![];

        for manga in search_response {
            mangas.push(MangaItem::from(manga));
        }

        Self { mangas }
    }
}

impl StatefulWidgetRef for ListMangasFoundWidget {
    type State = tui_widget_list::ListState;

    fn render_ref(&self, area: ratatui::prelude::Rect, buf: &mut Buffer, state: &mut Self::State) {
        let list = tui_widget_list::List::new(self.mangas.clone());
        StatefulWidget::render(list, area, buf, state);
    }
}
