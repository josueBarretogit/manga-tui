
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, StatefulWidget, Widget, Wrap};
use ratatui_image::Image;
use throbber_widgets_tui::{Throbber, ThrobberState};

use crate::backend::{Data, SearchMangaResponse};
use crate::common::{ImageState, Manga};
use crate::utils::{from_manga_response, set_status_style, set_tags_style};

#[derive(Clone, Default, PartialEq, Eq)]
pub enum CarrouselState {
    #[default]
    Searching,
    Displaying,
    NotFound,
}

#[derive(Clone)]
pub struct CarrouselItem {
    pub manga: Manga,
    pub loader_state: ThrobberState,
}

impl CarrouselItem {
    fn new(manga: Manga, loader_state: ThrobberState) -> Self {
        Self {
            manga,
            loader_state,
        }
    }

    fn render_cover(&mut self, area: Rect, buf: &mut Buffer, state: &mut ImageState) {
        match state.get_image_state(&self.manga.id) {
            Some(image_state) => {
                let cover = Image::new(image_state.as_ref());
                Widget::render(cover, area, buf);
            },
            None => {
                let loader = Throbber::default()
                    .label("Loading cover")
                    .style(Style::default().fg(Color::Yellow))
                    .throbber_set(throbber_widgets_tui::BRAILLE_SIX)
                    .use_type(throbber_widgets_tui::WhichUse::Spin);

                StatefulWidget::render(loader, area, buf, &mut self.loader_state);
            },
        };
    }

    fn render_details(&mut self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::vertical([Constraint::Percentage(20), Constraint::Percentage(80)]).margin(1);

        let [tags_area, description_area] = layout.areas(area);

        Block::bordered()
            .title(self.manga.title.clone())
            .title_bottom(self.manga.author.name.clone())
            .render(area, buf);

        let mut tags: Vec<Span<'_>> = self.manga.tags.iter().map(|tag| set_tags_style(tag)).collect();

        tags.push(set_status_style(&self.manga.status));
        tags.push(set_tags_style(&self.manga.content_rating));

        Paragraph::new(Line::from(tags)).wrap(Wrap { trim: true }).render(tags_area, buf);

        Paragraph::new(self.manga.description.clone())
            .wrap(Wrap { trim: true })
            .render(description_area, buf);
    }

    pub fn from_response(value: Data) -> Self {
        let manga = from_manga_response(value);
        Self::new(manga, ThrobberState::default())
    }

    pub fn tick(&mut self) {
        self.loader_state.calc_next();
    }
}

#[derive(Default, Clone)]
pub struct PopularMangaCarrousel {
    pub items: Vec<CarrouselItem>,
    pub current_item_visible_index: usize,
    pub state: CarrouselState,
    pub img_area: Rect,
    pub can_display_images: bool,
}

impl StatefulWidget for PopularMangaCarrousel {
    type State = ImageState;

    fn render(mut self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let layout = Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)]);
        let [cover_area, details_area] = layout.areas(area);
        match self.state {
            CarrouselState::Searching => {
                Block::bordered().render(area, buf);
                state.set_area(cover_area);
            },
            CarrouselState::Displaying => {
                match self.items.get_mut(self.current_item_visible_index) {
                    Some(item) => {
                        if self.can_display_images {
                            item.render_cover(cover_area, buf, state);
                        }
                        item.render_details(details_area, buf);
                    },
                    None => Block::bordered().title("Loading").render(area, buf),
                };
            },
            CarrouselState::NotFound => {
                Block::bordered().title("Mangas not found").render(area, buf);
            },
        }
    }
}

impl PopularMangaCarrousel {
    pub fn from_response(response: SearchMangaResponse, can_display_images: bool) -> Self {
        let mut items: Vec<CarrouselItem> = vec![];

        for manga in response.data {
            items.push(CarrouselItem::from_response(manga))
        }

        let img_area = Rect::default();

        Self {
            items,
            img_area,
            can_display_images,
            current_item_visible_index: 0,
            state: CarrouselState::Displaying,
        }
    }

    pub fn next_item(&mut self) {
        if self.state == CarrouselState::Displaying {
            if self.current_item_visible_index + 1 >= self.items.len() {
                self.current_item_visible_index = 0
            } else {
                self.current_item_visible_index += 1;
            }
        }
    }

    pub fn previous_item(&mut self) {
        if self.state == CarrouselState::Displaying {
            if self.current_item_visible_index == 0 {
                self.current_item_visible_index = self.items.len() - 1
            } else {
                self.current_item_visible_index = self.current_item_visible_index.saturating_sub(1)
            }
        }
    }

    pub fn get_current_item(&self) -> Option<&CarrouselItem> {
        if self.state == CarrouselState::Displaying { self.items.get(self.current_item_visible_index) } else { None }
    }

    pub fn tick(&mut self) {
        self.items.iter_mut().for_each(|item| item.tick());
    }
}

#[derive(Clone)]
pub struct RecentlyAddedCarrousel {
    pub items: Vec<CarrouselItem>,
    pub selected_item_index: usize,
    pub amount_items_per_page: usize,
    pub state: CarrouselState,
    can_display_images: bool,
}

impl StatefulWidget for RecentlyAddedCarrousel {
    type State = ImageState;

    fn render(mut self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let layout = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Fill(1),
            Constraint::Fill(1),
            Constraint::Fill(1),
            Constraint::Fill(1),
        ])
        .margin(1)
        .split(area);

        match self.state {
            CarrouselState::Displaying => {
                for (index, area_manga) in layout.iter().enumerate() {
                    let inner = area_manga.inner(Margin {
                        horizontal: 1,
                        vertical: 1,
                    });
                    let [cover_area, title_area] =
                        Layout::vertical([Constraint::Percentage(80), Constraint::Percentage(20)]).areas(inner);
                    if let Some(item) = self.items.get_mut(index) {
                        if self.can_display_images {
                            item.render_cover(cover_area, buf, state);
                        }
                        Paragraph::new(item.manga.title.clone()).render(title_area, buf);
                    }

                    if self.selected_item_index == index {
                        Block::bordered()
                            .border_style(Style::default().fg(Color::Yellow))
                            .render(*area_manga, buf);
                    }
                }
            },
            CarrouselState::Searching => {
                Block::bordered().title("Searching recent mangas").render(area, buf);
                let inner = layout[0].inner(Margin {
                    horizontal: 1,
                    vertical: 1,
                });
                let [a, _b] = Layout::vertical([Constraint::Percentage(80), Constraint::Percentage(20)]).areas(inner);
                state.set_area(a);
            },
            CarrouselState::NotFound => {
                Block::bordered().title("Could not get recent mangas").render(area, buf);
            },
        }
    }
}

impl Default for RecentlyAddedCarrousel {
    fn default() -> Self {
        Self {
            can_display_images: false,
            items: vec![],
            selected_item_index: 0,
            amount_items_per_page: 5,
            state: CarrouselState::default(),
        }
    }
}

impl RecentlyAddedCarrousel {
    pub fn select_next(&mut self) {
        if self.state == CarrouselState::Displaying && self.selected_item_index + 1 < self.amount_items_per_page {
            self.selected_item_index += 1;
        }
    }

    pub fn select_previous(&mut self) {
        if self.state == CarrouselState::Displaying {
            self.selected_item_index = self.selected_item_index.saturating_sub(1);
        }
    }

    pub fn get_current_selected_manga(&self) -> Option<&CarrouselItem> {
        if self.state == CarrouselState::Displaying { self.items.get(self.selected_item_index) } else { None }
    }

    pub fn tick(&mut self) {
        self.items.iter_mut().for_each(|item| item.tick());
    }

    pub fn from_response(response: SearchMangaResponse, can_display_images: bool) -> Self {
        let mut items: Vec<CarrouselItem> = vec![];

        for manga in response.data {
            items.push(CarrouselItem::from_response(manga))
        }

        Self {
            can_display_images,
            items,
            selected_item_index: 0,
            amount_items_per_page: 5,
            state: CarrouselState::Displaying,
        }
    }
}
