use std::sync::mpsc::Sender;

use ratatui::widgets::StatefulWidget;
use ratatui::Frame;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::Resize;

use crate::backend::tui::Events;

pub mod search;
pub mod manga;


pub trait Component {
    type Actions;
    ///Handles the logic for drawing to the screen
    fn render(&mut self, area: ratatui::prelude::Rect, frame : &mut Frame<'_>);
    fn handle_events(&mut self, events : Events);
    fn update(&mut self, action : Self::Actions);
}

/// A widget that uses a custom ThreadProtocol as state to offload resizing and encoding to a
/// background thread.
pub struct ThreadImage {
    resize: Resize,
}

impl ThreadImage {
    pub fn new() -> ThreadImage {
        ThreadImage {
            resize: Resize::Fit(None),
        }
    }

    pub fn resize(mut self, resize: Resize) -> ThreadImage {
        self.resize = resize;
        self
    }
}

impl StatefulWidget for ThreadImage {
    type State = ThreadProtocol;

    fn render(
        self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::buffer::Buffer,
        state: &mut Self::State,
    ) {
        state.inner = match state.inner.take() {
            // We have the `protocol` and should either resize or render.
            Some(mut protocol) => {
                // If it needs resizing (grow or shrink) then send it away instead of rendering.
                if let Some(rect) = protocol.needs_resize(&self.resize, area) {
                    state.tx.send((protocol, self.resize, rect)).unwrap();
                    None
                } else {
                    protocol.render(area, buf);
                    Some(protocol)
                }
            }
            // We are waiting to get back the protocol.
            None => None,
        };
    }
}

/// The state of a ThreadImage.
///
/// Has `inner` [ResizeProtocol] that is sent off to the `tx` mspc channel to do the
/// `resize_encode()` work.
#[derive(Clone)]
pub struct ThreadProtocol {
    pub inner: Option<Box<dyn StatefulProtocol>>,
    pub tx: Sender<(Box<dyn StatefulProtocol>, Resize, ratatui::prelude::Rect)>,
}

impl ThreadProtocol {
    pub fn new(
        tx: Sender<(Box<dyn StatefulProtocol>, Resize, ratatui::prelude::Rect)>,
        inner: Box<dyn StatefulProtocol>,
    ) -> ThreadProtocol {
        ThreadProtocol {
            inner: Some(inner),
            tx,
        }
    }
}
