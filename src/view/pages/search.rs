use crate::backend::fetch::MangadexClient;
use crate::backend::tui::Events;
use crate::backend::SearchMangaResponse;
use crate::view::widgets::search::*;
use crate::view::widgets::Component;
use crossterm::event::{self, KeyCode, KeyEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::{self, Constraint, Direction, Layout, Offset, Rect};
use ratatui::widgets::ListState;
use ratatui::widgets::StatefulWidgetRef;
use ratatui::widgets::{Block, Paragraph, Widget, WidgetRef};
use ratatui::Frame;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

/// Determine wheter or not mangas are being searched
/// if so then this should not make a request until the most recent one finishes
#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
enum State {
    Loading,
    SearchingMangas,
    DisplayingSearchResponse,
    #[default]
    Normal,
}

pub enum SearchPageActions {
    StartTyping,
    StopTyping,
    Search,
    LoadMangasFound(Option<SearchMangaResponse>),
    ScrollUp,
    ScrollDown,
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
    mangas_found_list: MangasFoundList,
}

#[derive(Default)]
struct MangasFoundList {
    widget: ListMangasFoundWidget,
    state: tui_widget_list::ListState,
    page: u16,
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
                self.state = State::DisplayingSearchResponse;
                match response {
                    Some(mangas_found) => {
                        self.mangas_found_list.widget =
                            ListMangasFoundWidget::from_response(mangas_found.data)
                    }
                    None => self.mangas_found_list.widget = ListMangasFoundWidget::default(),
                }
            }
            SearchPageActions::ScrollUp => self.scroll_up(),
            SearchPageActions::ScrollDown => self.scroll_down(),
        }
    }
    fn handle_events(&mut self, events: Events) {
        if let Events::Key(key_event) = events {
            match self.input_mode {
                InputMode::Idle => match key_event.code {
                    KeyCode::Char('s') => {
                        self.action_tx.send(SearchPageActions::StartTyping).unwrap();
                    }
                    KeyCode::Char('j') => {
                        self.action_tx.send(SearchPageActions::ScrollDown).unwrap()
                    }
                    KeyCode::Char('k') => self.action_tx.send(SearchPageActions::ScrollUp).unwrap(),
                    _ => {}
                },
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
            mangas_found_list: MangasFoundList::default(),
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

    fn render_manga_area(&mut self, area: Rect, buf: &mut Buffer) {

        if self.state == State::Normal || self.state == State::Loading {
            Block::bordered().render(area, buf);
        } else {
            StatefulWidgetRef::render_ref(
                &self.mangas_found_list.widget,
                area,
                buf,
                &mut self.mangas_found_list.state,
            );

        }
    }

    fn focus_search_bar(&mut self) {
        self.input_mode = InputMode::Typing;
    }

    pub fn scroll_down(&mut self) {
        let next = match self.mangas_found_list.state.selected {
            Some(index) => {
                if index == self.mangas_found_list.widget.mangas.len().saturating_sub(1) {
                    0
                } else {
                    index.saturating_add(1)
                }
            }
            None => self.mangas_found_list.state.selected().unwrap_or(0),
        };
        self.mangas_found_list.state.select(Some(next));
    }

    pub fn scroll_up(&mut self) {
        let next_index = match self.mangas_found_list.state.selected() {
            Some(index) => {
                if index == 0 {
                    self.mangas_found_list.widget.mangas.len().saturating_sub(1)
                } else {
                    index.saturating_sub(1)
                }
            }
            None => 1,
        };
        self.mangas_found_list.state.select(Some(next_index));
    }

    fn get_current_manga_selected(&mut self) -> Option<&mut MangaItem> {
        if let Some(index) = self.mangas_found_list.state.selected() {
            return self.mangas_found_list.widget.mangas.get_mut(index);
        }
        None
    }

    fn render_manga_preview(&mut self, area: Rect, buf: &mut Buffer) {
        let current_manga_selected = self.get_current_manga_selected();
        if let Some(manga) = current_manga_selected {
            let preview = MangaPreview::new(
                manga.title.clone(),
                manga.description.clone(),
                Some(&[2, 3, 4, 5]),
            );
            preview.render(area, buf);
        }
    }

}
