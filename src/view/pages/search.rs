use std::io::Cursor;
use std::ops::Deref;
use std::thread;

use crate::backend::fetch::MangadexClient;
use crate::backend::tui::Events;
use crate::backend::SearchMangaResponse;
use crate::view::widgets::search::*;
use crate::view::widgets::Component;
use bytes::Bytes;
use crossterm::event::KeyEvent;
use crossterm::event::{self, KeyCode, KeyEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::{self, Constraint, Direction, Layout, Offset, Rect};
use ratatui::widgets::ListState;
use ratatui::widgets::StatefulWidget;
use ratatui::widgets::StatefulWidgetRef;
use ratatui::widgets::{Block, Paragraph, Widget, WidgetRef};
use ratatui::Frame;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::Protocol;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::Resize;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

/// Determine wheter or not mangas are being searched
/// if so then this should not make a request until the most recent one finishes
#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
enum PageState {
    SearchingMangas,
    DisplayingSearchResponse,
    #[default]
    Normal,
}

/// This should happend "in the background"
enum SearchPageEvents {
    LoadCoverBytes(Option<Bytes>, usize),
    LoadMangasFound(Option<SearchMangaResponse>),
    StartSearchingCovers,
}

/// These are what the user can actively does
pub enum SearchPageActions {
    StartTyping,
    StopTyping,
    Search,
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
    picker: Picker,
    action_tx: UnboundedSender<SearchPageActions>,
    pub action_rx: UnboundedReceiver<SearchPageActions>,
    event_tx: UnboundedSender<SearchPageEvents>,
    event_rx: UnboundedReceiver<SearchPageEvents>,
    pub input_mode: InputMode,
    search_bar: Input,
    fetch_client: MangadexClient,
    state: PageState,
    mangas_found_list: MangasFoundList,
}

#[derive(Default)]
struct MangasFoundList {
    widget: ListMangasFoundWidget,
    cover_protocols: Vec<Box<dyn Protocol>>,
    state: tui_widget_list::ListState,
    cover_list_state: tui_widget_list::ListState,
    page: u16,
}

impl Component<SearchPageActions> for SearchPage {
    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) {
        let search_page_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Max(4), Constraint::Min(20)]);

        let [input_area, manga_area] = search_page_layout.areas(area);

        self.render_input_area(input_area, frame);

        self.render_search_results(manga_area, frame.buffer_mut());
    }

    fn update(&mut self, action: SearchPageActions) {
        match action {
            SearchPageActions::StartTyping => self.focus_search_bar(),
            SearchPageActions::StopTyping => self.input_mode = InputMode::Idle,
            SearchPageActions::Search => {
                self.state = PageState::SearchingMangas;
                let tx = self.event_tx.clone();
                let client = self.fetch_client.clone();
                let manga_to_search = self.search_bar.value().to_string();
                tokio::spawn(async move {
                    let search_response = client.search_mangas(&manga_to_search).await;

                    match search_response {
                        Ok(mangas_found) => {
                            if mangas_found.data.is_empty() {
                                tx.send(SearchPageEvents::LoadMangasFound(None)).unwrap();
                            } else {
                                tx.send(SearchPageEvents::LoadMangasFound(Some(mangas_found)))
                                    .unwrap();
                            }
                        }
                        Err(_) => {
                            tx.send(SearchPageEvents::LoadMangasFound(None)).unwrap();
                        }
                    }
                });
            }
            SearchPageActions::ScrollUp => self.scroll_up(),
            SearchPageActions::ScrollDown => self.scroll_down(),

            _ => {}
        }
    }
    fn handle_events(&mut self, events: Events) {
        match events {
            Events::Key(key_event) => self.handle_key_event(key_event),
            Events::Tick => {
                self.tick();
            }
            _ => {}
        }
    }
}

impl SearchPage {
    pub fn init(client: MangadexClient, picker: Picker) -> Self {
        let (action_tx, action_rx) = mpsc::unbounded_channel::<SearchPageActions>();
        let (state_tx, state_rx) = mpsc::unbounded_channel::<SearchPageEvents>();

        Self {
            picker,
            action_tx,
            action_rx,
            event_tx: state_tx,
            event_rx: state_rx,
            input_mode: InputMode::default(),
            search_bar: Input::default(),
            fetch_client: client,
            state: PageState::default(),
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

    fn render_search_results(&mut self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Max(30), Constraint::Fill(1)]);

        let [cover_area, mangas_area] = layout.areas(area);

        if self.state == PageState::Normal {
            Block::bordered().render(area, buf);
        } else {
            let mut covers: Vec<MangaCover> = vec![];

            for manga in &self.mangas_found_list.widget.mangas {
                let mut cover = MangaCover::new();
                covers.push(cover);
            }

            let covers_list = tui_widget_list::List::new(covers);

            StatefulWidget::render(
                covers_list,
                cover_area,
                buf,
                &mut self.mangas_found_list.cover_list_state,
            );

            StatefulWidgetRef::render_ref(
                &self.mangas_found_list.widget,
                mangas_area,
                buf,
                &mut self.mangas_found_list.state,
            );
        }
    }

    fn focus_search_bar(&mut self) {
        self.input_mode = InputMode::Typing;
    }

    pub fn scroll_down(&mut self) {
        self.mangas_found_list.state.next();
        self.mangas_found_list.cover_list_state.next();
    }

    pub fn scroll_up(&mut self) {
        self.mangas_found_list.state.previous();
        self.mangas_found_list.cover_list_state.previous();
    }

    fn get_current_manga_selected(&mut self) -> Option<&mut MangaItem> {
        if let Some(index) = self.mangas_found_list.state.selected {
            return self.mangas_found_list.widget.mangas.get_mut(index);
        }
        None
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match self.input_mode {
            InputMode::Idle => match key_event.code {
                KeyCode::Char('s') => {
                    self.action_tx.send(SearchPageActions::StartTyping).unwrap();
                }
                KeyCode::Char('j') => self.action_tx.send(SearchPageActions::ScrollDown).unwrap(),
                KeyCode::Char('k') => self.action_tx.send(SearchPageActions::ScrollUp).unwrap(),
                _ => {}
            },
            InputMode::Typing => match key_event.code {
                KeyCode::Enter => {
                    if self.state != PageState::SearchingMangas {
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
        };
    }

    /// Listen for events that happen "in the background"
    fn tick(&mut self) {
        if let Ok(event) = self.event_rx.try_recv() {
            match event {
                SearchPageEvents::LoadMangasFound(response) => {
                    self.state = PageState::DisplayingSearchResponse;
                    match response {
                        Some(mangas_found) => {
                            self.mangas_found_list.widget =
                                ListMangasFoundWidget::from_response(mangas_found.data);

                            self.event_tx
                                .send(SearchPageEvents::StartSearchingCovers)
                                .unwrap();
                        }
                        None => self.mangas_found_list.widget = ListMangasFoundWidget::not_found(),
                    }
                }
                SearchPageEvents::LoadCoverBytes(maybe_bytes, index_cover) => {
                    match maybe_bytes {
                        Some(bytes_found) => {
                            // self.mangas_found_list.cover_protocols.push(protocol_found);
                        }
                        None => {}
                    }
                }
                SearchPageEvents::StartSearchingCovers => {
                    for (index, manga) in self.mangas_found_list.widget.mangas.iter().enumerate() {
                        let manga_id = manga.id.clone();
                        let manga_file_name = manga.img_url.as_ref().unwrap().clone();
                        let tx = self.event_tx.clone();
                        let client = self.fetch_client.clone();
                        tokio::spawn(async move {
                            let response = client
                                .get_cover_for_manga(&manga_id, &manga_file_name)
                                .await;
                            match response {
                                Ok(bytes) => {
                                    tx.send(SearchPageEvents::LoadCoverBytes(Some(bytes), index))
                                        .unwrap();
                                }
                                Err(_e) => {
                                    tx.send(SearchPageEvents::LoadCoverBytes(None, index))
                                        .unwrap();
                                }
                            }
                        });
                    }
                }
            }
        }
    }
}
