use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui_image::protocol::StatefulProtocol;

use crate::backend::tui::Events;

pub mod feed;
pub mod filter_widget;
pub mod home;
pub mod manga;
pub mod reader;
pub mod search;

pub trait Component {
    type Actions;
    ///Handles the logic for drawing to the screen
    fn render(&mut self, area: ratatui::prelude::Rect, frame: &mut Frame<'_>);
    fn handle_events(&mut self, events: Events);
    fn update(&mut self, action: Self::Actions);

    /// This is intended for stuff like aborting tasks and clearing vec's
    fn clean_up(&mut self);
}

pub trait StatefulWidgetFrame {
    type State;
    fn render(&mut self, area: ratatui::prelude::Rect, frame: &mut Frame<'_>, state: &mut Self::State);
}

pub trait ImageHandler: Send + 'static {
    fn load(image: Box<dyn StatefulProtocol>, id: String) -> Self;
    fn not_found(id: String) -> Self;
}

#[allow(dead_code)]
// Use in testing
pub fn press_key<T>(page: &mut dyn Component<Actions = T>, key: KeyCode) {
    page.handle_events(Events::Key(key.into()));
}
