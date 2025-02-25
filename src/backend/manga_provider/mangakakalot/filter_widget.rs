use ratatui::layout::Margin;
use ratatui::widgets::Widget;

use super::filter_state::MangakakalotFiltersProvider;
use crate::backend::manga_provider::FiltersWidget;
use crate::view::widgets::StatefulWidgetFrame;

/// TODO: implement manganato filters in future release
#[derive(Debug, Clone)]
pub struct MangakakalotFilterWidget {}

impl FiltersWidget for MangakakalotFilterWidget {
    type FilterState = MangakakalotFiltersProvider;
}

impl StatefulWidgetFrame for MangakakalotFilterWidget {
    type State = MangakakalotFiltersProvider;

    fn render(&mut self, area: ratatui::prelude::Rect, frame: &mut ratatui::Frame<'_>, _state: &mut Self::State) {
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
