use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::backend::tui::Events;
use crate::view::widgets::feed::{FeedTabs, HistoryWidget};
use crate::view::widgets::Component;

pub enum FeedActions {
    ScrollHistoryUp,
    ScrollHistoryDown,
}

pub enum FeedEvents {
    SearchHistory,
    LoadHistory,
}

pub struct Feed {
    pub tabs: FeedTabs,
    pub history: Option<HistoryWidget>,
    pub manga_read_state: ListState,
    pub global_event_tx: UnboundedSender<Events>,
    pub local_action_tx: UnboundedSender<FeedActions>,
    pub local_action_rx: UnboundedReceiver<FeedActions>,
    pub local_event_tx: UnboundedSender<FeedEvents>,
    pub local_event_rx: UnboundedReceiver<FeedEvents>,
}

impl Feed {
    pub fn render_history(&mut self, area: Rect, buf: &mut Buffer) {
        match self.history.as_mut() {
            Some(history) => StatefulWidget::render(history.clone(), area, buf, &mut history.state),
            None => {
                Paragraph::new("You have not read any mangas yet").render(area, buf);
            }
        }
    }

    pub fn render_plan_to_read(&mut self, area: Rect, buf: &mut Buffer) {}

    pub fn handle_key_events(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.local_action_tx
                    .send(FeedActions::ScrollHistoryDown)
                    .ok();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.local_action_tx.send(FeedActions::ScrollHistoryUp).ok();
            }
            _ => {}
        }
    }

    pub fn tick(&mut self) {
        if let Ok(local_event) = self.local_event_rx.try_recv() {
            match local_event {
                FeedEvents::SearchHistory => todo!(),
                FeedEvents::LoadHistory => todo!(),
            }
        }
    }
}

impl Component for Feed {
    type Actions = FeedActions;
    fn render(&mut self, area: ratatui::prelude::Rect, frame: &mut Frame<'_>) {
        let buf = frame.buffer_mut();
        match self.tabs {
            FeedTabs::History => self.render_history(area, buf),
            FeedTabs::PlantToRead => todo!(),
        }
    }

    fn update(&mut self, action: Self::Actions) {
        match action {
            FeedActions::ScrollHistoryUp => {}
            FeedActions::ScrollHistoryDown => {}
        }
    }

    fn clean_up(&mut self) {
        self.history = None;
    }

    fn handle_events(&mut self, events: crate::backend::tui::Events) {
        match events {
            Events::Key(key_event) => {
                self.handle_key_events(key_event);
            }
            Events::Tick => self.tick(),
            _ => {}
        }
    }
}
