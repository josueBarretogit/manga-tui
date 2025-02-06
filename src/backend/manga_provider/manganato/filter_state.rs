use crossterm::event::KeyCode;

use crate::backend::manga_provider::{EventHandler, FiltersHandler};
use crate::backend::tui::Events;

#[derive(Debug, Clone)]
pub struct ManganatoFilterState {}

#[derive(Debug, Clone)]
pub struct ManganatoFiltersProvider {
    is_open: bool,
    filter: ManganatoFilterState,
}

impl ManganatoFiltersProvider {
    pub fn new(filter: ManganatoFilterState) -> Self {
        Self {
            is_open: false,
            filter,
        }
    }
}

impl EventHandler for ManganatoFiltersProvider {
    fn handle_events(&mut self, events: crate::backend::tui::Events) {
        match events {
            Events::Key(key) => match key.code {
                KeyCode::Char('f') => self.toggle(),
                _ => {},
            },
            _ => {},
        }
    }
}

impl FiltersHandler for ManganatoFiltersProvider {
    type InnerState = ManganatoFilterState;

    fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    fn is_open(&self) -> bool {
        self.is_open
    }

    fn is_typing(&self) -> bool {
        false
    }

    fn get_state(&self) -> &Self::InnerState {
        &self.filter
    }
}
