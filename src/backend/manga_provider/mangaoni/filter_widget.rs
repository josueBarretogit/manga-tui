use ratatui::layout::Margin;
use ratatui::widgets::Widget;

use super::filter_state::MangaoniFiltersProvider;
use crate::backend::manga_provider::FiltersWidget;
use crate::view::widgets::StatefulWidgetFrame;

#[derive(Debug, Clone, Default)]
pub struct MangaoniFilterWidget {}

impl MangaoniFilterWidget {
    pub fn new() -> Self {
        Self {}
    }
}

impl FiltersWidget for MangaoniFilterWidget {
    type FilterState = MangaoniFiltersProvider;
}

impl StatefulWidgetFrame for MangaoniFilterWidget {
    type State = MangaoniFiltersProvider;

    fn render(&mut self, area: ratatui::prelude::Rect, frame: &mut ratatui::Frame<'_>, _state: &mut Self::State) {
        let buf = frame.buffer_mut();
        "No filters available for MangaOni".render(
            area.inner(Margin {
                horizontal: 2,
                vertical: 2,
            }),
            buf,
        );
    }
}
