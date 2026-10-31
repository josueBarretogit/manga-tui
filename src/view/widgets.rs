#[cfg(test)]
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::{Margin, Rect};
use ratatui::style::{Color, Modifier, Style, Styled};
use ratatui::text::{Line, Span};
use ratatui::widgets::block::Title;
use ratatui::widgets::{Block, Clear, Paragraph, Widget, WidgetRef, Wrap};

use crate::backend::manga_provider::LatestChapter;
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

#[derive(Debug, Clone, PartialEq)]
pub struct NewChapterItem {
    pub manga_id: String,
    pub manga_title: String,
    pub chapter_numbers: String,
    pub chapters: Vec<LatestChapter>,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewChaptersModalState {
    pub items: Vec<NewChapterItem>,
    pub cursor: usize,
    pub status_message: Option<String>,
}

impl NewChaptersModalState {
    pub fn new(items: Vec<NewChapterItem>) -> Self {
        Self {
            items,
            cursor: 0,
            status_message: None,
        }
    }
}

#[derive(Debug)]
pub struct NewChaptersModal<'a> {
    state: &'a NewChaptersModalState,
}

impl<'a> NewChaptersModal<'a> {
    #[inline]
    pub fn new(state: &'a NewChaptersModalState) -> Self {
        Self { state }
    }
}

impl<'a> Widget for NewChaptersModal<'a> {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let area = centered_rect(area, 75, 65);
        Clear.render(area, buf);

        if let Some(status) = &self.state.status_message {
            let block = Block::bordered()
                .title(Line::from(vec![Span::styled(" New chapters ", Style::default().add_modifier(Modifier::BOLD))]))
                .title_bottom(Line::from(vec![Span::styled(
                    " Press <q> or <Esc> to close ",
                    Style::default().add_modifier(Modifier::ITALIC),
                )]));
            let inner = block.inner(area);
            block.render(area, buf);
            Paragraph::new(status.as_str()).wrap(Wrap { trim: true }).render(inner, buf);
            return;
        }

        let selected_count = self.state.items.iter().filter(|i| i.selected).count();
        let total_count = self.state.items.len();

        let block = Block::bordered()
            .title(Line::from(vec![Span::styled(
                format!(" New chapters available ({}/{}) ", selected_count, total_count),
                Style::default().add_modifier(Modifier::BOLD),
            )]))
            .title_bottom(Line::from(vec![Span::styled(
                " <j/k> Navigate | <Space> Toggle | <a> All | <y>/<d> Download | <q> Close ",
                Style::default().fg(Color::Yellow),
            )]));

        let inner = area.inner(Margin {
            horizontal: 2,
            vertical: 1,
        });
        block.render(area, buf);

        if inner.height < 3 || inner.width < 10 {
            return;
        }

        let header =
            Line::from(vec![Span::styled("Select manga to download new chapters:", Style::default().add_modifier(Modifier::BOLD))]);

        let max_items_visible = (inner.height.saturating_sub(2)) as usize;
        let cursor = self.state.cursor;
        let scroll = if max_items_visible == 0 || total_count <= max_items_visible {
            0
        } else if cursor < max_items_visible {
            0
        } else {
            cursor + 1 - max_items_visible
        };

        let mut lines = vec![header, Line::raw("")];

        for (idx, item) in self.state.items.iter().enumerate().skip(scroll).take(max_items_visible) {
            let is_focused = idx == cursor;
            let cursor_span = if is_focused {
                Span::styled("> ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            } else {
                Span::raw("  ")
            };

            let check_span = if item.selected {
                Span::styled("[✓] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
            } else {
                Span::styled("[ ] ", Style::default().fg(Color::DarkGray))
            };

            let title_style = if is_focused {
                if item.selected {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)
                }
            } else if item.selected {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let ch_style = if item.selected { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::DarkGray) };

            let status_extra = if !item.selected { " (skipped)" } else { "" };

            lines.push(Line::from(vec![
                cursor_span,
                check_span,
                Span::styled(item.manga_title.as_str(), title_style),
                Span::styled(format!(": Ch. {}{}", item.chapter_numbers, status_extra), ch_style),
            ]));
        }

        Paragraph::new(lines).render(inner, buf);
    }
}

#[derive(Debug)]
pub struct DownloadQueueModal<'a> {
    progress: Option<&'a crate::backend::download_queue::DownloadQueueProgress>,
}

impl<'a> DownloadQueueModal<'a> {
    pub fn new(progress: Option<&'a crate::backend::download_queue::DownloadQueueProgress>) -> Self {
        Self { progress }
    }
}

impl<'a> Widget for DownloadQueueModal<'a> {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let area = centered_rect(area, 60, 40);
        Clear.render(area, buf);

        let block = Block::bordered()
            .title(Line::from(vec![Span::styled(
                " ⬇ Download Queue ",
                Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan),
            )]))
            .title_bottom(Line::from(vec![Span::styled(
                " <c> Cancel All | <q> or <Esc> or <D> Close ",
                Style::default().fg(Color::Yellow),
            )]));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 3 || inner.width < 10 {
            return;
        }

        let mut lines = Vec::new();
        match self.progress {
            Some(p) if p.is_downloading => {
                lines.push(Line::from(vec![
                    Span::styled("Currently downloading: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(&p.current_manga_title, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                ]));
                lines.push(Line::from(vec![Span::raw(format!(
                    "Chapter: {}  |  Page progress: {:.0}%",
                    p.current_chapter_number, p.current_page_percentage
                ))]));
                lines.push(Line::from(vec![Span::styled(
                    format!(
                        "Batch: {} of {} completed ({} remaining in queue)",
                        p.completed_in_batch, p.total_in_batch, p.remaining_items
                    ),
                    Style::default().fg(Color::Cyan),
                )]));
                lines.push(Line::raw(""));

                // Gauge representation
                let gauge_width = (inner.width.saturating_sub(10)) as usize;
                let filled = ((p.current_page_percentage / 100.0) * gauge_width as f64).round() as usize;
                let filled = filled.min(gauge_width);
                let empty = gauge_width.saturating_sub(filled);
                let bar = format!("[{}{}] {:.0}%", "█".repeat(filled), "░".repeat(empty), p.current_page_percentage);
                lines.push(Line::from(vec![Span::styled(bar, Style::default().fg(Color::Cyan))]));
            },
            Some(p) if p.remaining_items > 0 => {
                lines.push(Line::from(vec![Span::styled(
                    format!("Queue waiting ({} items remaining)", p.remaining_items),
                    Style::default().fg(Color::Yellow),
                )]));
            },
            _ => {
                lines.push(Line::raw("No active downloads in queue."));
                lines.push(Line::raw(""));
                lines.push(Line::from(vec![
                    Span::styled("Tip: ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw("Select chapters with <Space> and press <d> to add them to the queue!"),
                ]));
            },
        }

        Paragraph::new(lines).render(inner, buf);
    }
}

#[cfg(test)]
// Use in testing
pub fn press_key<T>(page: &mut dyn Component<Actions = T>, key: KeyCode) {
    page.handle_events(Events::Key(key.into()));
}
