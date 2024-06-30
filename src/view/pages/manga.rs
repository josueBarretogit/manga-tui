use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::backend::tui::Events;
use crate::view::widgets::ThreadProtocol;

pub enum MangaPageActions {
    ScrollChapterDown,
    ScrollChapterUp,
}

pub enum MangaPageEvents {
    FetchChapters,
}

pub struct MangaPage {
    pub id: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub img_url: Option<String>,
    pub image_state: Option<ThreadProtocol>,
    global_event_tx: UnboundedSender<Events>,
    local_action_tx: UnboundedSender<MangaPageActions>,
    local_action_rx: UnboundedReceiver<MangaPageActions>,
    local_event_tx: UnboundedSender<MangaPageEvents>,
    local_event_rx: UnboundedReceiver<MangaPageEvents>,
}
