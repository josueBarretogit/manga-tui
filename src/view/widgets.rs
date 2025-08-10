#[cfg(test)]
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::{Margin, Rect};
use ratatui::style::Styled;
use ratatui::text::Line;
use ratatui::widgets::block::Title;
use ratatui::widgets::{Block, Clear, Paragraph, Widget, WidgetRef, Wrap};

use crate::backend::tui::Events;
use crate::global::INSTRUCTIONS_STYLE;
use crate::utils::centered_rect;

pub mod feed;
pub mod home;
pub mod manga;
pub mod reader;
pub mod search;

pub trait Component {
    type Actions;
    ///Handles the logic for drawing to the screen
    fn render(&mut self, area: Rect, frame: &mut Frame<'_>);
    fn handle_events(&mut self, events: Events);
    fn update(&mut self, action: Self::Actions);

    /// This is intended for stuff like aborting tasks and clearing vec's
    fn clean_up(&mut self);
}

pub trait StatefulWidgetFrame {
    type State;
    fn render(&mut self, area: Rect, frame: &mut Frame<'_>, state: &mut Self::State);
}

#[derive(Debug, Default)]
pub struct ModalDimensions {
    width: u16,
    height: u16,
}

#[derive(Debug)]
pub struct Modal<T: WidgetRef> {
    dimensions: ModalDimensions,
    child_widget: T,
}

#[derive(Debug, Default)]
pub struct ModalBuilder<T: WidgetRef> {
    dimensions: Option<ModalDimensions>,
    child_widget: T,
}

impl<T: WidgetRef> ModalBuilder<T> {
    #[inline]
    pub fn new(child_widget: T) -> Self {
        Self {
            dimensions: None,
            child_widget,
        }
    }

    #[inline]
    pub fn with_dimensions(mut self, dimensions: ModalDimensions) -> Self {
        self.dimensions = Some(dimensions);
        self
    }

    #[inline]
    pub fn build(self) -> Modal<T> {
        Modal::new(
            self.dimensions.unwrap_or(ModalDimensions {
                width: 50,
                height: 50,
            }),
            self.child_widget,
        )
    }
}

impl<T: WidgetRef> Modal<T> {
    pub fn new(dimensions: ModalDimensions, child_widget: T) -> Self {
        Self {
            dimensions,
            child_widget,
        }
    }
}

impl<T: WidgetRef> WidgetRef for Modal<T> {
    fn render_ref(&self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let area = centered_rect(area, self.dimensions.width, self.dimensions.height);
        Clear.render(area, buf);

        self.child_widget.render_ref(area, buf);
    }
}

impl<T: WidgetRef> Widget for Modal<T> {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        WidgetRef::render_ref(&self, area, buf);
    }
}

#[derive(Debug)]
struct ErrorModalBody<'a> {
    message: &'a str,
}

impl<'a> Widget for ErrorModalBody<'a> {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        self.render_ref(area, buf);
    }
}

impl<'a> WidgetRef for ErrorModalBody<'a> {
    fn render_ref(&self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        Block::bordered()
            .title(Title::from(Line::from(vec![
                "Some error ocurred, press ".into(),
                "<q>".set_style(*INSTRUCTIONS_STYLE),
                " to close this popup".into(),
            ])))
            .render(area, buf);

        let inner = area.inner(Margin {
            horizontal: 2,
            vertical: 2,
        });

        Paragraph::new(self.message).wrap(Wrap { trim: true }).render(inner, buf);
    }
}

#[derive(Debug)]
pub struct ErrorModal<'a> {
    message: &'a str,
}

impl<'a> ErrorModal<'a> {
    #[inline]
    pub fn new(message: &'a str) -> Self {
        Self { message }
    }
}

impl<'a> Widget for ErrorModal<'a> {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let modal = ModalBuilder::new(ErrorModalBody {
            message: self.message,
        })
        .build();

        modal.render_ref(area, buf);
    }
}

#[cfg(test)]
// Use in testing
pub fn press_key<T>(page: &mut dyn Component<Actions = T>, key: KeyCode) {
    page.handle_events(Events::Key(key.into()));
}
