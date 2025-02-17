use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph, StatefulWidget, StatefulWidgetRef, Widget, Wrap};
use ratatui_image::Image;
use throbber_widgets_tui::{Throbber, ThrobberState};
use tui_widget_list::PreRender;

use crate::backend::manga_provider::{Genres, MangaStatus, SearchManga};
use crate::common::ImageState;
use crate::global::CURRENT_LIST_ITEM_STYLE;

pub struct MangaPreview<'a> {
    id: &'a str,
    title: &'a str,
    description: &'a str,
    genres: &'a Vec<Genres>,
    status: Option<MangaStatus>,
    can_display_images: bool,
    loader_state: ThrobberState,
}

#[allow(clippy::too_many_arguments)]
impl<'a> MangaPreview<'a> {
    pub fn new(
        id: &'a str,
        title: &'a str,
        description: &'a str,
        genres: &'a Vec<Genres>,
        status: Option<MangaStatus>,
        can_display_images: bool,
        loader_state: ThrobberState,
    ) -> Self {
        Self {
            id,
            title,
            description,
            genres,
            status,
            can_display_images,
            loader_state,
        }
    }

    pub fn render_cover_and_details_area(&mut self, area: Rect, buf: &mut Buffer, state: &mut ImageState) {
        let layout = Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(80)]);

        let [cover_area, details_area] = layout.areas(area);

        if self.can_display_images {
            self.render_details(details_area, buf);

            match state.get_image_state(self.id) {
                Some(image_state) => {
                    let cover = Image::new(image_state.as_ref());
                    Widget::render(cover, cover_area, buf);
                },
                None => {
                    state.set_area(cover_area);
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
        } else {
            self.render_details(
                area.inner(Margin {
                    horizontal: 1,
                    vertical: 1,
                }),
                buf,
            );
        }
    }

    pub fn render_description_area(self, area: Rect, buf: &mut Buffer) {
        Block::bordered().title(self.title).render(area, buf);

        let inner = area.inner(Margin {
            horizontal: 1,
            vertical: 2,
        });

        Paragraph::new(self.description).wrap(Wrap { trim: true }).render(inner, buf);
    }

    pub fn render_details(&mut self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::vertical([Constraint::Percentage(20), Constraint::Percentage(80)]);
        let [details_area, tags_area] = layout.areas(area);

        if let Some(status) = self.status {
            Paragraph::new(Line::from_iter(vec![status])).render(details_area, buf);
        }

        Paragraph::new(Line::from_iter(self.genres.clone()))
            .wrap(Wrap { trim: true })
            .render(tags_area, buf);
    }
}

impl<'a> StatefulWidget for MangaPreview<'a> {
    type State = ImageState;

    fn render(mut self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let [cover_details_area, description_area] =
            Layout::vertical([Constraint::Percentage(40), Constraint::Percentage(60)]).areas(area);

        self.render_cover_and_details_area(cover_details_area, buf, state);
        self.render_description_area(description_area, buf);
    }
}

#[derive(Default, Clone, Debug, PartialEq)]
pub struct MangaItem {
    pub manga: SearchManga,
    pub style: Style,
}

impl Widget for MangaItem {
    fn render(self, area: Rect, buf: &mut Buffer)
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

impl From<SearchManga> for MangaItem {
    fn from(value: SearchManga) -> Self {
        Self::new(value)
    }
}

impl MangaItem {
    pub fn new(manga: SearchManga) -> Self {
        Self {
            manga,
            style: Style::default(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct ListMangasFoundWidget {
    pub mangas: Vec<MangaItem>,
}

impl ListMangasFoundWidget {
    pub fn from_response(search_response: Vec<SearchManga>) -> Self {
        let mut mangas: Vec<MangaItem> = vec![];

        for manga in search_response {
            mangas.push(MangaItem::from(manga));
        }

        Self { mangas }
    }
}

impl StatefulWidgetRef for ListMangasFoundWidget {
    type State = tui_widget_list::ListState;

    fn render_ref(&self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let list = tui_widget_list::List::new(self.mangas.clone());
        StatefulWidget::render(list, area, buf, state);
    }
}
