use std::collections::HashMap;
use std::fmt::Display;

use ratatui::layout::Rect;
use ratatui_image::protocol::Protocol;

#[derive(Default)]
pub struct ImageState {
    /// save the image loaded for a manga, it will be retrieved by it's id
    image_state: HashMap<String, Box<dyn Protocol>>,
    img_area: Rect,
}

impl ImageState {
    pub fn insert_manga(&mut self, fixed_protocol: Box<dyn Protocol>, id_manga: String) {
        self.image_state.insert(id_manga, fixed_protocol);
    }

    pub fn get_img_area(&self) -> Rect {
        self.img_area
    }

    /// After a manga is rendered it will be know what area the covers fits into
    pub fn set_area(&mut self, area: Rect) {
        self.img_area = area;
    }

    /// get the image cover state given the manga id
    pub fn get_image_state(&mut self, id: &str) -> Option<&mut Box<dyn Protocol>> {
        self.image_state.get_mut(id)
    }

    pub fn is_empty(&self) -> bool {
        self.image_state.is_empty()
    }
}

pub fn format_error_message_tracking_reading_history<A: Display, B: Display, C: Display>(
    chapter: A,
    manga_title: B,
    error: C,
) -> String {
    format!(
        "Could not track reading progress of chapter : {chapter} \n of manga : {manga_title}, more details about the error : \n ERROR | {error}"
    )
}
