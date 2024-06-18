use crate::backend::tui::Events;
use crate::view::widgets::search::*;
use crate::view::widgets::Component;
use crossterm::event::{self, KeyCode, KeyEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::{self, Constraint, Direction, Layout, Offset, Rect};
use ratatui::widgets::ListState;
use ratatui::widgets::StatefulWidget;
use ratatui::widgets::{Block, Paragraph, Widget, WidgetRef};
use ratatui::Frame;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

struct MangasFound {
    id: String,
    title: String,
    tags: Vec<String>,
    description: Vec<String>,
}

struct Mangas(Vec<MangasFound>);

pub enum SearchPageActions {
    StartTyping,
    StopTyping,
    Search,
    Load,
}

#[derive(Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum InputMode {
    Typing,
    #[default]
    Idle,
}

///This is the "page" where the user can search for a manga
pub struct SearchPage {
    action_tx: UnboundedSender<SearchPageActions>,
    pub action_rx: UnboundedReceiver<SearchPageActions>,
    pub input_mode: InputMode,
    search_bar: Input,
}

impl Component<SearchPageActions> for SearchPage {
    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) {
        let search_page_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Max(4), Constraint::Min(20)]);

        let [input_area, manga_area] = search_page_layout.areas(area);

        self.render_input_area(input_area, frame);

        self.render_manga_area(manga_area, frame.buffer_mut());
    }
    fn update(&mut self, action: SearchPageActions) {
        match action {
            SearchPageActions::StartTyping => self.focus_search_bar(),
            SearchPageActions::StopTyping => self.input_mode = InputMode::Idle,
            SearchPageActions::Search => {}
            SearchPageActions::Load => {}
        }
    }
    fn handle_events(&mut self, events: Events) {
        if let Events::Key(key_event) = events {
            match self.input_mode {
                InputMode::Idle => {
                    if let KeyCode::Char('s') = key_event.code {
                        self.action_tx.send(SearchPageActions::StartTyping).unwrap()
                    }
                }
                InputMode::Typing => match key_event.code {
                    KeyCode::Enter => {
                        self.action_tx.send(SearchPageActions::Search).unwrap();
                    }
                    KeyCode::Esc => {
                        self.action_tx.send(SearchPageActions::StopTyping).unwrap();
                    }
                    _ => {
                        self.search_bar.handle_event(&event::Event::Key(key_event));
                    }
                },
            }
        }
    }
}

impl SearchPage {
    pub fn init() -> Self {
        let (action_tx, action_rx) = mpsc::unbounded_channel::<SearchPageActions>();

        Self {
            action_tx,
            action_rx,
            input_mode: InputMode::default(),
            search_bar: Input::default(),
        }
    }

    fn render_input_area(&self, area: Rect, frame: &mut Frame<'_>) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Max(1), Constraint::Max(5)])
            .split(area);

        let input_bar = Paragraph::new(self.search_bar.value()).block(Block::bordered().title(
            match self.input_mode {
                InputMode::Idle => "Press <s> to type ",
                InputMode::Typing => "Press <enter> to search,<esc> to stop typing",
            },
        ));

        input_bar.render(layout[1], frame.buffer_mut());

        let width = layout[0].width.max(3) - 3;

        let scroll = self.search_bar.visual_scroll(width as usize);

        match self.input_mode {
            InputMode::Idle => {}
            InputMode::Typing => frame.set_cursor(
                layout[1].x + ((self.search_bar.visual_cursor()).max(scroll) - scroll) as u16 + 1,
                layout[1].y + 1,
            ),
        }
    }

    fn render_manga_area(&self, area: Rect, buf: &mut Buffer) {
        let layout = layout::Layout::default()
            .margin(1)
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)]);

        let [list_mangas_found_area, manga_preview_area] = layout.areas(area);

        let list_mangas_widget = ListMangasFoundWidget::new(vec![
            MangaItem::new("a manga".to_string(), false),
            MangaItem::new("another".to_string(), true),
        ]);

        StatefulWidget::render(list_mangas_widget, list_mangas_found_area, buf, &mut ListState::default());

        let preview = MangaPreview::new(
            "a preview".to_string(),
            "a description".to_string(),
            &[1, 2, 3, 4, 5],
        );

        preview.render(manga_preview_area, buf);
    }

    fn focus_search_bar(&mut self) {
        self.input_mode = InputMode::Typing;
    }
}
