use ratatui::layout::Margin;
use ratatui::widgets::Widget;

use super::filter_state::ManganatoFiltersProvider;
use crate::backend::manga_provider::FiltersWidget;
use crate::view::widgets::StatefulWidgetFrame;

#[derive(Debug, Clone)]
pub struct ManganatoFilterWidget {}

impl FiltersWidget for ManganatoFilterWidget {
    type FilterState = ManganatoFiltersProvider;
}

impl StatefulWidgetFrame for ManganatoFilterWidget {
    type State = ManganatoFiltersProvider;

    fn render(&mut self, area: ratatui::prelude::Rect, frame: &mut ratatui::Frame<'_>, state: &mut Self::State) {
        let buf = frame.buffer_mut();
        "no filters available on manganato".render(
            area.inner(Margin {
                horizontal: 2,
                vertical: 2,
            }),
            buf,
        );
    }
}
