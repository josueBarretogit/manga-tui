use ratatui::layout::Rect;
use ratatui::Frame;

use crate::backend::tui::Events;

pub mod search;

pub trait Component<A> {
    ///Handles the logic for drawing to the screen
    fn render(&mut self, area: Rect, frame : &mut Frame<'_>);
    fn handle_events(&mut self, events : Events);
    fn update(&mut self, action : A);
}


