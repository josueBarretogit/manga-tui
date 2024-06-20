use core::panic;

use crate::backend::fetch::MangadexClient;
use crate::backend::tui::Events;
use crate::backend::SearchMangaResponse;
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

#[derive(Default)]
struct MangasFound {
    id: String,
    title: String,
    tags: Vec<String>,
    description: Vec<String>,
    img_url: String,
}

/// Determine wheter or not mangas are being searched
/// if so then this should not make a request until the most recent one finishes
#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
enum State {
    Loading,
    SearchingMangas,
    DisplayingMangasFound,
    #[default]
    Normal,
}

#[derive(Default)]
struct Mangas(Vec<MangasFound>);

pub enum SearchPageActions {
    StartTyping,
    StopTyping,
    Search,
    LoadMangasFound(Option<SearchMangaResponse>),
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
    fetch_client: MangadexClient,
    state: State,
    mangas_list_state: ListState,
    mangas_found_list : MangasFoundList,

}

struct MangasFoundList {
    widget: ListMangasFoundWidget,
    data: Vec<String>
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
            SearchPageActions::Search => {
                let tx = self.action_tx.clone();
                let client = self.fetch_client.clone();
                let manga_to_search = self.search_bar.value().to_string();
                tokio::spawn(async move {
                    let search_response = client.search_mangas(&manga_to_search).await;

                    match search_response {
                        Ok(mangas_found) => {
                            if mangas_found.data.is_empty() {
                                tx.send(SearchPageActions::LoadMangasFound(None)).unwrap();
                            } else {
                                tx.send(SearchPageActions::LoadMangasFound(Some(mangas_found)))
                                    .unwrap();
                            }
                        }
                        Err(e) => {
                            tx.send(SearchPageActions::LoadMangasFound(None)).unwrap();
                        }
                    }
                });
            }
            SearchPageActions::LoadMangasFound(response) => {
                self.state = State::DisplayingMangasFound;

                match response {
                    Some(mangas_found) => {
                        self.mangas_found = Some(mangas_found);
                    }
                    None => self.mangas_found = None,
                }
            }
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
                        if self.state != State::SearchingMangas {
                            self.action_tx.send(SearchPageActions::Search).unwrap();
                        }
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
    pub fn init(client: MangadexClient) -> Self {
        let (action_tx, action_rx) = mpsc::unbounded_channel::<SearchPageActions>();

        Self {
            action_tx,
            action_rx,
            input_mode: InputMode::default(),
            search_bar: Input::default(),
            fetch_client: client,
            state: State::default(),
            mangas_list_state: ListState::default(),
            mangas_found: None,
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

        if self.state == State::Normal || self.state == State::Loading {
            Block::bordered().render(list_mangas_found_area, buf);
        } else {
            match self.mangas_found.as_ref() {
                Some(mangas) => {
                    let list_mangas_found =
                        ListMangasFoundWidget::new(MangaItem::from_response(mangas));

                    StatefulWidget::render(
                        list_mangas_found,
                        list_mangas_found_area,
                        buf,
                        &mut self.mangas_list_state.clone(),
                    );
                }
                None => {
                    Block::bordered()
                        .title("No mangas found")
                        .render(list_mangas_found_area, buf);
                }
            }
        }

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

    pub fn scroll_down(&mut self) {
        let next = match self.mangas_list_state.selected() {
            Some(index) => {
                if index == self.crates_list.widget.crates.len().saturating_sub(1) {
                    0
                } else {
                    index.saturating_add(1)
                }
            }
            None => self.crates_list.state.selected().unwrap_or(0),
        };
        self.crates_list.state.select(Some(next));
    }

    pub fn scroll_up(&mut self) {
        let next_index = match self.mangas_list_state.selected()  {
            Some(index) => {
                if index == 0 {
                    self.crates_list.widget.crates.len().saturating_sub(1)
                } else {
                    index.saturating_sub(1)
                }
            }
            None => 1,
        };
        self.crates_list.state.select(Some(next_index));
    }
}
