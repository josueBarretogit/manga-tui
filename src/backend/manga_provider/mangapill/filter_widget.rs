use ratatui::layout::Margin;
use ratatui::widgets::Widget;

use super::filter_state::MangaPillFiltersProvider;
use crate::backend::manga_provider::FiltersWidget;
use crate::view::widgets::StatefulWidgetFrame;

/// TODO: implement Weebcentral filters in future release
#[derive(Debug, Clone)]
pub struct MangaPillFilterWidget {}

impl MangaPillFilterWidget {
    pub fn new() -> Self {
        Self {}
    }
}

impl FiltersWidget for MangaPillFilterWidget {
    type FilterState = MangaPillFiltersProvider;
}

impl StatefulWidgetFrame for MangaPillFilterWidget {
    type State = MangaPillFiltersProvider;

    fn render(&mut self, area: ratatui::prelude::Rect, frame: &mut ratatui::Frame<'_>, _state: &mut Self::State) {
        let buf = frame.buffer_mut();
        "no filters available on Weebcentral".render(
            area.inner(Margin {
                horizontal: 2,
                vertical: 2,
            }),
            buf,
        );
    }
}
