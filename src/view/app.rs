use ratatui_image::protocol::StatefulProtocol;

use crate::view::pages::*;


pub struct App {
    pub image: Box<dyn StatefulProtocol>,
    pub image_width: u16,
    pub image_heigth: u16,
}

impl App {
    pub fn zoom_in(&mut self) {
        self.image_heigth += 1;
        self.image_width += 1;
    }
    pub fn zoom_out(&mut self) {
        self.image_width = self.image_width.saturating_sub(1);
        self.image_heigth = self.image_heigth.saturating_sub(1);
    }
}
