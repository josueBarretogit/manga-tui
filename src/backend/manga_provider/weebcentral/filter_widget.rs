use ratatui::layout::Margin;
use ratatui::widgets::Widget;

use super::filter_state::WeebcentralFiltersProvider;
use crate::backend::manga_provider::FiltersWidget;
use crate::view::widgets::StatefulWidgetFrame;

/// TODO: implement Weebcentral filters in future release
#[derive(Debug, Clone)]
pub struct WeebcentralFilterWidget {}

impl FiltersWidget for WeebcentralFilterWidget {
    type FilterState = WeebcentralFiltersProvider;
}

impl StatefulWidgetFrame for WeebcentralFilterWidget {
    type State = WeebcentralFiltersProvider;

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
