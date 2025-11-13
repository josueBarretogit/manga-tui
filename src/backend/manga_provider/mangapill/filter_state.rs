use crossterm::event::KeyCode;

use crate::backend::manga_provider::{EventHandler, FiltersHandler};
use crate::backend::tui::Events;

#[derive(Debug, Clone, Default)]
pub struct MangaPillFilterState {}

#[derive(Debug, Clone)]
pub struct MangaPillFiltersProvider {
    is_open: bool,
    filter: MangaPillFilterState,
}

impl MangaPillFiltersProvider {
    pub fn new(filter: MangaPillFilterState) -> Self {
        Self {
            is_open: false,
            filter,
        }
    }
}

impl EventHandler for MangaPillFiltersProvider {
    fn handle_events(&mut self, events: crate::backend::tui::Events) {
        #![allow(clippy::single_match)]
        match events {
            Events::Key(key) => match key.code {
                KeyCode::Char('f') => self.toggle(),
                _ => {},
            },
            _ => {},
        }
    }
}

impl FiltersHandler for MangaPillFiltersProvider {
    type InnerState = MangaPillFilterState;

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
