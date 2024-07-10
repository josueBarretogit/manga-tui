use crate::backend::tui::Events;
use ratatui::Frame;

pub mod manga;
pub mod reader;
pub mod search;
pub mod home;

pub trait Component {
    type Actions;
    ///Handles the logic for drawing to the screen
    fn render(&mut self, area: ratatui::prelude::Rect, frame: &mut Frame<'_>);
    fn handle_events(&mut self, events: Events);
    fn update(&mut self, action: Self::Actions);

    /// This is intended for stuff like aborting tasks and clearing vec's
    fn clean_up(&mut self);
}
