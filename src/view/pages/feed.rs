use std::thread::JoinHandle;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::backend::database::{get_reading_history, MangaHistory};
use crate::backend::tui::Events;
use crate::view::widgets::feed::{FeedTabs, HistoryWidget};
use crate::view::widgets::Component;

pub enum FeedActions {
    ScrollHistoryUp,
    ScrollHistoryDown,
}

pub enum FeedEvents {
    SearchHistory,
    LoadHistory(Option<Vec<MangaHistory>>),
}

pub struct Feed {
    pub tabs: FeedTabs,
    pub history: Option<HistoryWidget>,
    pub global_event_tx: UnboundedSender<Events>,
    pub local_action_tx: UnboundedSender<FeedActions>,
    pub local_action_rx: UnboundedReceiver<FeedActions>,
    pub local_event_tx: UnboundedSender<FeedEvents>,
    pub local_event_rx: UnboundedReceiver<FeedEvents>,
    tasks: Vec<JoinHandle<()>>,
}

impl Feed {
    pub fn new( global_event_tx: UnboundedSender<Events>) -> Self {
        let (local_action_tx, local_action_rx) = mpsc::unbounded_channel::<FeedActions>();
        let (local_event_tx, local_event_rx) = mpsc::unbounded_channel::<FeedEvents>();
        Self {
            tabs: FeedTabs::History,
            history: None,
            global_event_tx,
            local_action_tx,
            local_action_rx,
            local_event_tx,
            local_event_rx,
            tasks: vec![],
        }
    }

    fn render_history(&mut self, area: Rect, buf: &mut Buffer) {
        match self.history.as_mut() {
            Some(history) => StatefulWidget::render(history.clone(), area, buf, &mut history.state),
            None => {
                Paragraph::new("You have not read any mangas yet").render(area, buf);
            }
        }
    }

    pub fn init_search(&mut self) {
        self.local_event_tx.send(FeedEvents::SearchHistory).ok();
    }

    fn render_plan_to_read(&mut self, area: Rect, buf: &mut Buffer) {}

    fn handle_key_events(&mut self, key_event: KeyEvent) {
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
                FeedEvents::SearchHistory => self.search_history(),
                FeedEvents::LoadHistory(maybe_history) => self.load_history(maybe_history),
            }
        }
    }

    fn search_history(&mut self) {
        let tx = self.local_event_tx.clone();
        self.tasks.push(std::thread::spawn(move || {
            let maybe_reading_history = get_reading_history();
            tx.send(FeedEvents::LoadHistory(maybe_reading_history.ok()))
                .ok();
        }));
    }

    fn load_history(&mut self, maybe_history: Option<Vec<MangaHistory>>) {
        if let Some(history) = maybe_history {
            self.history = Some(HistoryWidget::from(history));
        }
    }

    fn search_plan_to_read(&mut self) {
        todo!()
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
