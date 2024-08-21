use ratatui::prelude::*;
use ratatui::widgets::*;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};
use throbber_widgets_tui::{Throbber, ThrobberState};

use crate::backend::{Data, SearchMangaResponse};
use crate::common::Manga;
use crate::utils::{from_manga_response, set_status_style, set_tags_style};
use crate::PICKER;

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
    pub cover_state: Option<Box<dyn StatefulProtocol>>,
    pub loader_state: ThrobberState,
}

impl CarrouselItem {
    fn new(manga: Manga, cover_state: Option<Box<dyn StatefulProtocol>>, loader_state: ThrobberState) -> Self {
        Self {
            manga,
            cover_state,
            loader_state,
        }
    }

    fn render_cover(&mut self, area: Rect, buf: &mut Buffer) {
        match self.cover_state {
            Some(ref mut image_state) => {
                let cover = StatefulImage::new(None).resize(Resize::Fit(None));

                StatefulWidget::render(cover, area, buf, image_state)
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
        Self::new(manga, None, ThrobberState::default())
    }

    pub fn tick(&mut self) {
        if self.cover_state.is_none() {
            self.loader_state.calc_next();
        }
    }

    pub fn render_recently_added(&mut self, area: Rect, buf: &mut Buffer) {
        if PICKER.is_some() {
            let layout = Layout::vertical([Constraint::Percentage(80), Constraint::Percentage(20)]);
            let [cover_area, title_area] = layout.areas(area);
            self.render_cover(cover_area, buf);

            Paragraph::new(self.manga.title.clone()).wrap(Wrap { trim: true }).render(title_area, buf);
        } else {
            let [title_area, description_area] =
                Layout::vertical([Constraint::Percentage(30), Constraint::Percentage(70)]).areas(area);
            Paragraph::new(self.manga.title.clone()).wrap(Wrap { trim: true }).render(title_area, buf);

            Paragraph::new(self.manga.description.clone())
                .wrap(Wrap { trim: true })
                .render(description_area, buf);
        }
    }
}

// This implementation is used for the popular mangas carrousel
impl Widget for CarrouselItem {
    fn render(mut self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)]);

        let [cover_area, details_area] = layout.areas(area);

        if PICKER.is_some() {
            self.render_cover(cover_area, buf);
        }
        self.render_details(details_area, buf);
    }
}

#[derive(Default, Clone)]
pub struct PopularMangaCarrousel {
    pub items: Vec<CarrouselItem>,
    pub current_item_visible_index: usize,
    pub state: CarrouselState,
}

impl StatefulWidget for PopularMangaCarrousel {
    type State = usize;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        match self.state {
            CarrouselState::Searching => {
                Block::bordered().render(area, buf);
            },
            CarrouselState::Displaying => {
                match self.items.get(*state) {
                    Some(item) => item.clone().render(area, buf),
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
    pub fn from_response(response: SearchMangaResponse) -> Self {
        let mut items: Vec<CarrouselItem> = vec![];

        for manga in response.data {
            items.push(CarrouselItem::from_response(manga))
        }

        Self {
            items,
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
}

impl StatefulWidget for RecentlyAddedCarrousel {
    type State = usize;

    fn render(mut self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        match self.state {
            CarrouselState::Displaying => {
                let layout = Layout::horizontal([
                    Constraint::Fill(1),
                    Constraint::Fill(1),
                    Constraint::Fill(1),
                    Constraint::Fill(1),
                    Constraint::Fill(1),
                ])
                .split(area);

                for (index, area_manga) in layout.iter().enumerate() {
                    let inner = area_manga.inner(Margin {
                        horizontal: 1,
                        vertical: 1,
                    });
                    if let Some(item) = self.items.get_mut(index) {
                        item.render_recently_added(inner, buf);
                    }

                    if *state == index {
                        Block::bordered()
                            .border_style(Style::default().fg(Color::Yellow))
                            .render(*area_manga, buf);
                    }
                }
            },
            CarrouselState::Searching => {
                Block::bordered().title("Searching recent mangas").render(area, buf);
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

    pub fn from_response(response: SearchMangaResponse) -> Self {
        let mut items: Vec<CarrouselItem> = vec![];

        for manga in response.data {
            items.push(CarrouselItem::from_response(manga))
        }

        Self {
            items,
            selected_item_index: 0,
            amount_items_per_page: 5,
            state: CarrouselState::Displaying,
        }
    }
}
