use crossterm::event::KeyCode;

use crate::backend::manga_provider::{EventHandler, FiltersHandler};
use crate::backend::tui::Events;

#[derive(Debug, Clone, Default)]
pub struct LeercapituloFilterState {}

#[derive(Debug, Clone)]
pub struct LeercapituloFiltersProvider {
    is_open: bool,
    filter: LeercapituloFilterState,
}

impl LeercapituloFiltersProvider {
    pub fn new(filter: LeercapituloFilterState) -> Self {
        Self {
            is_open: false,
            filter,
        }
    }
}

impl EventHandler for LeercapituloFiltersProvider {
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

impl FiltersHandler for LeercapituloFiltersProvider {
    type InnerState = LeercapituloFilterState;

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
