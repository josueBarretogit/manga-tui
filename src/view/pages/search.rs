use std::io::Cursor;
use std::sync::mpsc::TryRecvError;
use std::sync::Arc;
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
use ratatui::widgets::StatefulWidget;
use ratatui::widgets::StatefulWidgetRef;
use ratatui::widgets::{Block, Paragraph, Widget, WidgetRef};
use ratatui::Frame;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::Resize;
use tokio::sync::broadcast::error::RecvError;
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

/// These happens in the background
enum SearchPageEvents {
    LoadCover(Option<Bytes>, usize),
    LoadMangasFound(Option<SearchMangaResponse>),
    SearchCovers,
}

/// These are actions that the user actively does
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
    global_event_tx: UnboundedSender<Events>,
    picker: Picker,
    action_tx: UnboundedSender<SearchPageActions>,
    pub action_rx: UnboundedReceiver<SearchPageActions>,
    event_tx: UnboundedSender<SearchPageEvents>,
    event_rx: UnboundedReceiver<SearchPageEvents>,
    pub input_mode: InputMode,
    search_bar: Input,
    fetch_client: Arc<MangadexClient>,
    state: State,
    mangas_found_list: MangasFoundList,
}

#[derive(Default)]
struct MangasFoundList {
    widget: ListMangasFoundWidget,
    state: tui_widget_list::ListState,
}

impl Component<SearchPageActions> for SearchPage {
    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) {
        let search_page_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Max(4), Constraint::Fill(1)]);

        let [input_area, manga_area] = search_page_layout.areas(area);

        self.render_input_area(input_area, frame);

        self.render_manga_area(manga_area, frame.buffer_mut());
    }

    fn update(&mut self, action: SearchPageActions) {
        match action {
            SearchPageActions::StartTyping => self.focus_search_bar(),
            SearchPageActions::StopTyping => self.input_mode = InputMode::Idle,
            SearchPageActions::Search => {
                self.state = State::SearchingMangas;
                self.mangas_found_list.widget = ListMangasFoundWidget::default();
                let tx = self.event_tx.clone();
                let client = Arc::clone(&self.fetch_client);
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
            Events::Key(key_event) => self.handle_key_events(key_event),
            Events::Redraw(protocol, index) => {
                if let Some(pro) = self.mangas_found_list.widget.mangas[index]
                    .image_state
                    .as_mut()
                {
                    pro.inner = Some(protocol)
                }
            }
            Events::Tick => self.tick(),
            _ => {}
        }
    }
}

impl SearchPage {
    pub fn init(
        client: Arc<MangadexClient>,
        picker: Picker,
        event_tx: UnboundedSender<Events>,
    ) -> Self {
        let (action_tx, action_rx) = mpsc::unbounded_channel::<SearchPageActions>();
        let (local_event_tx, local_event) = mpsc::unbounded_channel::<SearchPageEvents>();

        Self {
            global_event_tx: event_tx,
            picker,
            action_tx,
            action_rx,
            event_tx: local_event_tx,
            event_rx: local_event,
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
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(50)]);

        let [manga_list_area, preview_area] = layout.areas(area);

        if self.state == State::Normal || self.state == State::Loading {
            Block::bordered().render(area, buf);
        } else {
            StatefulWidgetRef::render_ref(
                &self.mangas_found_list.widget,
                manga_list_area,
                buf,
                &mut self.mangas_found_list.state,
            );

            if let Some(manga_select) = self.get_current_manga_selected() {
                StatefulWidget::render(
                    match manga_select.image_state.as_ref() {
                        Some(state) => MangaPreview::with_image_protocol(
                            manga_select.title.clone(),
                            manga_select.description.clone(),
                            vec![],
                            state.clone(),
                        ),
                        None => MangaPreview::new(
                            manga_select.title.clone(),
                            manga_select.description.clone(),
                            vec![],
                        ),
                    },
                    preview_area,
                    buf,
                    &mut None,
                );
            }
        }
    }

    fn focus_search_bar(&mut self) {
        self.input_mode = InputMode::Typing;
    }

    pub fn scroll_down(&mut self) {
        self.mangas_found_list.state.next();
    }

    pub fn scroll_up(&mut self) {
        self.mangas_found_list.state.previous();
    }

    fn get_current_manga_selected(&mut self) -> Option<&mut MangaItem> {
        if let Some(index) = self.mangas_found_list.state.selected {
            return self.mangas_found_list.widget.mangas.get_mut(index);
        }
        None
    }
    fn handle_key_events(&mut self, key_event: KeyEvent) {
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

    fn tick(&mut self) {
        if let Ok(event) = self.event_rx.try_recv() {
            match event {
                SearchPageEvents::LoadMangasFound(response) => {
                    self.state = State::DisplayingSearchResponse;
                    match response {
                        Some(mangas_found) => {
                            let mut mangas: Vec<MangaItem> = vec![];


                            for manga in mangas_found.data {
                                let manga_id = manga.id.clone();
                                let cover_file_name =
                                mangas.push(MangaItem::from(manga));
                            }

                            self.mangas_found_list.widget = ListMangasFoundWidget::new(mangas);
                        }
                        None => self.mangas_found_list.widget = ListMangasFoundWidget::default(),
                    }
                }
                SearchPageEvents::LoadCover(maybe_bytes, index) => match maybe_bytes {
                    Some(bytes) => {
                        let tx = self.global_event_tx.clone();

                        let (tx_worker, rec_worker) =
                            std::sync::mpsc::channel::<(Box<dyn StatefulProtocol>, Resize, Rect)>();

                        let dyn_img = image::io::Reader::new(Cursor::new(bytes))
                            .with_guessed_format()
                            .unwrap();

                        let image = self.picker.new_resize_protocol(dyn_img.decode().unwrap());

                        self.mangas_found_list.widget.mangas[index].image_state =
                            Some(ThreadProtocol::new(tx_worker.clone(), image));

                        thread::spawn(move || loop {
                            match rec_worker.recv() {
                                Ok((mut protocol, resize, area)) => {
                                    protocol.resize_encode(&resize, None, area);
                                    tx.send(Events::Redraw(protocol, index)).unwrap();
                                }
                                Err(_e) => break,
                            }
                        });
                    }
                    None => {}
                },

                SearchPageEvents::SearchCovers => {
                    for (index, manga) in self.mangas_found_list.widget.mangas.iter().enumerate() {
                        let event_tx = self.event_tx.clone();
                        let client = Arc::clone(&self.fetch_client);
                        let manga_id = manga.id.clone();

                        let file_name = manga.img_url.clone();

                        match file_name {
                            Some(name) => {
                                tokio::spawn(async move {
                                    let img_bytes =
                                        client.get_cover_for_manga(&manga_id, &name).await;
                                    match img_bytes {
                                        Ok(bytes) => {
                                            event_tx
                                                .send(SearchPageEvents::LoadCover(
                                                    Some(bytes),
                                                    index,
                                                ))
                                                .unwrap();
                                        }
                                        Err(e) => event_tx
                                            .send(SearchPageEvents::LoadCover(None, index))
                                            .unwrap(),
                                    }
                                });
                            }
                            None => {}
                        }
                    }
                }
            }
        }
    }
}
