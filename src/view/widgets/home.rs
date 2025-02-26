use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, StatefulWidget, Widget, Wrap};
use ratatui_image::Image;
use throbber_widgets_tui::{Throbber, ThrobberState};

use crate::backend::manga_provider::{PopularManga, RecentlyAddedManga};
use crate::common::ImageState;

#[derive(Clone, Default, PartialEq, Eq)]
pub enum CarrouselState {
    #[default]
    Searching,
    Displaying,
    NotFound,
}

#[derive(Clone)]
pub struct CarrouselItemPopularManga {
    pub manga: PopularManga,
    pub loader_state: ThrobberState,
}

impl CarrouselItemPopularManga {
    fn new(manga: PopularManga, loader_state: ThrobberState) -> Self {
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

        let [tags_area, status_area] =
            Layout::horizontal([Constraint::Percentage(80), Constraint::Percentage(20)]).areas(tags_area);

        Block::bordered().title(self.manga.title.clone()).render(area, buf);

        Paragraph::new(Line::from_iter(self.manga.genres.clone()))
            .wrap(Wrap { trim: true })
            .render(tags_area, buf);

        if let Some(status) = self.manga.status {
            Paragraph::new(Line::from(Span::from(status)))
                .wrap(Wrap { trim: true })
                .render(status_area, buf);
        }

        Paragraph::new(self.manga.description.clone())
            .wrap(Wrap { trim: true })
            .render(description_area, buf);
    }

    pub fn from_response(value: PopularManga) -> Self {
        Self::new(value, ThrobberState::default())
    }

    pub fn tick(&mut self) {
        self.loader_state.calc_next();
    }
}

#[derive(Default, Clone)]
pub struct PopularMangaCarrousel {
    pub items: Vec<CarrouselItemPopularManga>,
    pub current_item_visible_index: usize,
    pub state: CarrouselState,
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
    pub fn from_response(response: Vec<PopularManga>, can_display_images: bool) -> Self {
        let mut items: Vec<CarrouselItemPopularManga> = vec![];

        for manga in response {
            items.push(CarrouselItemPopularManga::from_response(manga))
        }

        Self {
            items,
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

    pub fn get_current_item(&self) -> Option<&CarrouselItemPopularManga> {
        if self.state == CarrouselState::Displaying { self.items.get(self.current_item_visible_index) } else { None }
    }

    pub fn tick(&mut self) {
        self.items.iter_mut().for_each(|item| item.tick());
    }
}

#[derive(Clone)]
pub struct CarrouselItemRecentlyAddedManga {
    pub manga: RecentlyAddedManga,
    pub loader_state: ThrobberState,
}

impl CarrouselItemRecentlyAddedManga {
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

    pub fn tick(&mut self) {
        self.loader_state.calc_next();
    }
}

#[derive(Clone)]
pub struct RecentlyAddedCarrousel {
    pub items: Vec<CarrouselItemRecentlyAddedManga>,
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

        let manga_constraints = if self.can_display_images {
            [Constraint::Percentage(80), Constraint::Percentage(20)]
        } else {
            [Constraint::Percentage(30), Constraint::Percentage(70)]
        };

        match self.state {
            CarrouselState::Displaying => {
                for (index, area_manga) in layout.iter().enumerate() {
                    let margin = area_manga.inner(Margin {
                        horizontal: 1,
                        vertical: 1,
                    });

                    let [top, bottom] = Layout::vertical(manga_constraints).areas(margin);

                    if let Some(item) = self.items.get_mut(index) {
                        if self.can_display_images {
                            item.render_cover(top, buf, state);
                            Paragraph::new(item.manga.title.clone()).render(bottom, buf);
                        } else {
                            Paragraph::new(item.manga.title.clone()).wrap(Wrap { trim: true }).render(top, buf);
                            Paragraph::new(item.manga.description.clone())
                                .wrap(Wrap { trim: true })
                                .render(bottom, buf);
                        }
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
                if self.can_display_images {
                    let margin = layout[0].inner(Margin {
                        horizontal: 1,
                        vertical: 1,
                    });

                    let [cover_area, _b] = Layout::vertical(manga_constraints).areas(margin);
                    state.set_area(cover_area);
                }
            },
            CarrouselState::NotFound => {
                Block::bordered().title("Could not get recent mangas").render(area, buf);
            },
        }
    }
}

impl RecentlyAddedCarrousel {
    pub fn new(can_display_images: bool) -> Self {
        Self {
            can_display_images,
            items: vec![],
            selected_item_index: 0,
            amount_items_per_page: 5,
            state: CarrouselState::default(),
        }
    }

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

    pub fn get_current_selected_manga(&self) -> Option<&CarrouselItemRecentlyAddedManga> {
        if self.state == CarrouselState::Displaying { self.items.get(self.selected_item_index) } else { None }
    }

    pub fn tick(&mut self) {
        self.items.iter_mut().for_each(|item| item.tick());
    }

    pub fn from_response(response: Vec<RecentlyAddedManga>, can_display_images: bool) -> Self {
        let mut items: Vec<CarrouselItemRecentlyAddedManga> = vec![];

        for mangas in response {
            items.push(CarrouselItemRecentlyAddedManga {
                manga: mangas,
                loader_state: ThrobberState::default(),
            });
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
