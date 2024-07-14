use ratatui::{prelude::*, widgets::*};
use tui_widget_list::PreRender;

use crate::backend::database::MangaHistory;

pub enum FeedTabs {
    History,
    PlantToRead,
}

#[derive(Clone)]
pub struct MangasRead {
    pub id: String,
    pub title: String,
    pub style: Style,
    pub recent_chapters: Option<Vec<String>>,
}

impl Widget for MangasRead {
    fn render(mut self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]);

        let [title_area, recent_chapters_area] = layout.margin(1).areas(area);

        Block::bordered().style(self.style).render(area, buf);

        Paragraph::new(self.title)
            .block(Block::default().borders(Borders::RIGHT))
            .wrap(Wrap { trim: true })
            .render(title_area, buf);

        if let Some(chapters) = self.recent_chapters.as_mut() {
            Widget::render(
                List::new(
                    chapters
                        .iter()
                        .map(|chap| chap.to_owned())
                        .collect::<Vec<String>>(),
                ),
                recent_chapters_area,
                buf,
            );
        }
    }
}

impl PreRender for MangasRead {
    fn pre_render(&mut self, context: &tui_widget_list::PreRenderContext) -> u16 {
        if context.is_selected {
            self.style = Style::default().fg(Color::Yellow);
        }
        10
    }
}

#[derive(Clone)]
pub struct HistoryWidget {
    pub mangas_read: Vec<MangasRead>,
    pub state: tui_widget_list::ListState,
}

impl HistoryWidget {
    pub fn select_next(&mut self) {
        self.state.next();
    }

    pub fn select_previous(&mut self) {
        self.state.previous();
    }

    pub fn get_current_manga_selected(&self) -> Option<&MangasRead> {
        match self.state.selected {
            Some(index) => self.mangas_read.get(index),
            None => None,
        }
    }

    pub fn set_manga_recent_chapters(&mut self, id: &str, chapters: Vec<String>) {
        if let Some(manga) = self.mangas_read.iter_mut().find(|manga| manga.id == id) {
            manga.recent_chapters = Some(chapters);
        }
    }
}

impl From<Vec<MangaHistory>> for HistoryWidget {
    fn from(value: Vec<MangaHistory>) -> Self {
        Self {
            mangas_read: value
                .iter()
                .map(|history| MangasRead {
                    id: history.id.clone(),
                    title: history.title.clone(),
                    recent_chapters: None,
                    style: Style::default(),
                })
                .collect(),
            state: tui_widget_list::ListState::default(),
        }
    }
}

impl StatefulWidget for HistoryWidget {
    type State = tui_widget_list::ListState;
    fn render(
        self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
        state: &mut Self::State,
    ) {
        let list = tui_widget_list::List::new(self.mangas_read);
        StatefulWidget::render(list, area, buf, state);
    }
}
