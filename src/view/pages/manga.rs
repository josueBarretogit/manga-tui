use crate::backend::tui::Events;
use crate::view::widgets::{Component, ThreadImage, ThreadProtocol};
use ratatui::{prelude::*, widgets::*};
use ratatui_image::Resize;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

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

impl MangaPage {
    pub fn new(
        id: String,
        title: String,
        description: String,
        tags: Vec<String>,
        img_url: Option<String>,
        image_state: Option<ThreadProtocol>,
        global_event_tx: UnboundedSender<Events>,
    ) -> Self {
        let (local_action_tx, local_action_rx) = mpsc::unbounded_channel::<MangaPageActions>();
        let (local_event_tx, local_event_rx) = mpsc::unbounded_channel::<MangaPageEvents>();

        Self {
            id,
            title,
            description,
            tags,
            img_url,
            image_state,
            global_event_tx,
            local_action_tx,
            local_action_rx,
            local_event_tx,
            local_event_rx,
        }
    }
    fn render_cover_and_tags(&mut self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::default()
            .margin(1)
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)]);

        let [cover_area, manga_information_area] = layout.areas(area);

        match self.image_state.as_mut() {
            Some(state) => {
                Block::bordered().render(cover_area, buf);
                let image = ThreadImage::new().resize(Resize::Fit(None));
                StatefulWidget::render(image, cover_area, buf, state);
            }
            None => {
                Block::bordered().render(cover_area, buf);
            }
        }

        Paragraph::new(self.description.clone()).render(manga_information_area, buf);
    }
}

// Todo! listen to the resize event in handle events
impl Component for MangaPage {
    type Actions = MangaPageActions;
    fn render(&mut self, area: ratatui::prelude::Rect, frame: &mut ratatui::Frame<'_>) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(50)]);

        let [cover_and_description_area, tags_and_chapters_area] = layout.areas(area);

        self.render_cover_and_tags(cover_and_description_area, frame.buffer_mut());
    }
    fn update(&mut self, action: Self::Actions) {
        match action {
            MangaPageActions::ScrollChapterUp => {}
            MangaPageActions::ScrollChapterDown => {}
        }
    }
    fn handle_events(&mut self, events: Events) {}
}
