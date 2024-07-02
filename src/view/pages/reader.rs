use ratatui::{prelude::*, widgets::*};

use crate::view::widgets::ThreadProtocol;

pub struct Page {
    image_state : Option<ThreadProtocol>,
    url : String,
    data_save_url : String,
} 



pub struct MangaReader {
    chapter_id : String,
    pages: Vec<Page>,
    state : ListState,
}

impl StatefulWidget for MangaReader {
    type State = usize;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        
    }
}


