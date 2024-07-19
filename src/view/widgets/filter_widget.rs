use ratatui::widgets::{Clear, Widget};
use tui_menu::{Menu, MenuEvent, MenuItem, MenuState};

use crate::filter::ContentRating;
use crate::utils::centered_rect;

pub struct FilterWidget {
    pub is_open: bool,
    pub content_rating: ContentRatingInput,
}

pub struct ContentRatingInput {
    pub state: MenuState<ContentRating>,
    pub widget: Menu<ContentRating>,
}

impl Widget for FilterWidget {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let popup_are = centered_rect(area, 70, 50);


        Clear.render(popup_are, buf);
    }
}

impl FilterWidget {
    pub fn new() -> Self {
        let content_rating_items = vec![
            MenuItem::item("safe", ContentRating::Safe),
            MenuItem::item("suggestive", ContentRating::Suggestive),
            MenuItem::item("erotic", ContentRating::Erotic),
            MenuItem::item("pornographic", ContentRating::Pornographic),
        ];
        Self {
            is_open: false,
            content_rating: ContentRatingInput {
                state: MenuState::new(content_rating_items),
                widget: Menu::<ContentRating>::new(),
            },
        }
    }

    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }
}
