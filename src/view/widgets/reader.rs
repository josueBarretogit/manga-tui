use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Paragraph, StatefulWidget, Widget, Wrap};
use throbber_widgets_tui::{Throbber, ThrobberState};
use tui_widget_list::PreRender;

use crate::global::CURRENT_LIST_ITEM_STYLE;

#[derive(PartialEq, Eq, Clone)]
pub enum PageItemState {
    Loading,
    FinishedLoad,
    _NotFound,
}

#[derive(Clone)]
pub struct PagesItem {
    pub number: usize,
    pub state: PageItemState,
    pub loading_state: ThrobberState,
    pub style: Style,
}

impl Widget for PagesItem {
    fn render(mut self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]);
        let [chapter_number_area, loader_area] = layout.areas(area);

        Block::default().style(self.style).render(area, buf);
        let page = Paragraph::new(format!("Page {}", self.number)).wrap(Wrap { trim: true });

        if self.state == PageItemState::Loading {
            let loader = Throbber::default()
                .label("Loading")
                .style(Style::default().fg(Color::Yellow))
                .throbber_set(throbber_widgets_tui::BRAILLE_SIX)
                .use_type(throbber_widgets_tui::WhichUse::Spin);

            page.render(chapter_number_area, buf);

            StatefulWidget::render(loader, loader_area, buf, &mut self.loading_state);
        } else {
            page.render(area, buf);
        }
    }
}

impl PreRender for PagesItem {
    fn pre_render(&mut self, context: &tui_widget_list::PreRenderContext) -> u16 {
        if context.is_selected {
            self.style = *CURRENT_LIST_ITEM_STYLE;
        }
        2
    }
}

impl PagesItem {
    pub fn new(number: usize) -> Self {
        Self {
            number,
            state: PageItemState::Loading,
            loading_state: ThrobberState::default(),
            style: Style::default(),
        }
    }

    pub fn on_tick(&mut self) {
        self.loading_state.calc_next();
    }
}

#[derive(Clone, Default)]
pub struct PagesList {
    pub pages: Vec<PagesItem>,
}

impl PagesList {
    pub fn new(pages: Vec<PagesItem>) -> Self {
        Self { pages }
    }

    pub fn on_tick(&mut self) {
        for page in self.pages.iter_mut() {
            page.on_tick();
        }
    }
}

impl StatefulWidget for PagesList {
    type State = tui_widget_list::ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let items = tui_widget_list::List::new(self.pages);
        StatefulWidget::render(items, area, buf, state)
    }
}
