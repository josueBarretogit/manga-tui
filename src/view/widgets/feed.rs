use ratatui::{prelude::*, widgets::*};

use crate::backend::database::MangaHistory;

pub enum FeedTabs {
    History,
    PlantToRead,
}

#[derive(Clone)]
pub struct MangasRead {
    id: String,
    title: String,
}

impl From<MangasRead> for ListItem<'_> {
    fn from(value: MangasRead) -> Self {
        let line = Line::from(value.title);
        ListItem::new(line)
    }
}

#[derive(Clone)]
pub struct HistoryWidget {
    pub mangas_read: Vec<MangasRead>,
    pub state: ListState,
}

impl From<Vec<MangaHistory>> for HistoryWidget {
    fn from(value: Vec<MangaHistory>) -> Self {
        Self {
            mangas_read: value
                .iter()
                .map(|history| MangasRead {
                    id: history.id.clone(),
                    title: history.title.clone(),
                })
                .collect(),
            state: ListState::default(),
        }
    }
}

impl StatefulWidget for HistoryWidget {
    type State = ListState;
    fn render(
        self,
        area: ratatui::prelude::Rect,
        buf: &mut ratatui::prelude::Buffer,
        state: &mut Self::State,
    ) {
        let list = List::new(self.mangas_read)
            .highlight_symbol("> ")
            .highlight_style(Style::default().fg(Color::Yellow));
        StatefulWidget::render(list, area, buf, state);
    }
}
