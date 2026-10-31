use ratatui::layout::Margin;
use ratatui::widgets::Widget;

use super::filter_state::ComizyFiltersProvider;
use crate::backend::manga_provider::FiltersWidget;
use crate::view::widgets::StatefulWidgetFrame;

#[derive(Debug, Clone, Default)]
pub struct ComizyFilterWidget {}

impl ComizyFilterWidget {
    pub fn new() -> Self {
        Self {}
    }
}

impl FiltersWidget for ComizyFilterWidget {
    type FilterState = ComizyFiltersProvider;
}

impl StatefulWidgetFrame for ComizyFilterWidget {
    type State = ComizyFiltersProvider;

    fn render(&mut self, area: ratatui::prelude::Rect, frame: &mut ratatui::Frame<'_>, _state: &mut Self::State) {
        let buf = frame.buffer_mut();
        "No filters available for Comizy".render(
            area.inner(Margin {
                horizontal: 2,
                vertical: 2,
            }),
            buf,
        );
    }
}
