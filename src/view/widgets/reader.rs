use crossterm::style;
use ratatui::{prelude::*, widgets::*};
use throbber_widgets_tui::{Throbber, ThrobberState};
use tui_widget_list::PreRender;

#[derive(PartialEq, Eq, Clone)]
pub enum PageItemState {
    Loading,
    FinishedLoad,
    NotFound,
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
        let layout = Layout::horizontal([Constraint::Percentage(20), Constraint::Percentage(80)]);
        let [chapter_number_area, loader_area] = layout.areas(area);

        Block::default().style(self.style).render(area, buf);

        format!("Page {}", self.number).render(chapter_number_area, buf);

        if self.state == PageItemState::Loading {
            let loader = Throbber::default()
                .label("Loading")
                .style(Style::default().fg(Color::Yellow))
                .throbber_set(throbber_widgets_tui::BRAILLE_SIX)
                .use_type(throbber_widgets_tui::WhichUse::Spin);

            StatefulWidget::render(loader, loader_area, buf, &mut self.loading_state);
        }
    }
}

impl PreRender for PagesItem {
    fn pre_render(&mut self, context: &tui_widget_list::PreRenderContext) -> u16 {
        if context.is_selected {
            self.style = Style::default().bg(Color::Blue);
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
            if page.state == PageItemState::Loading {
                page.on_tick();
            }
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
