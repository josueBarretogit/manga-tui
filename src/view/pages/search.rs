use crate::backend::fetch::MangadexClient;
use crate::backend::tui::Events;
use crate::backend::SearchMangaResponse;
use crate::view::widgets::search::*;
use crate::view::widgets::Component;
use crossterm::event::KeyEvent;
use crossterm::event::{self, KeyCode};
use image::io::Reader;
use image::DynamicImage;
use ratatui::{prelude::*, widgets::*};
use ratatui_image::picker::Picker;
use std::io::Cursor;
use std::sync::Arc;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;
use tui_widget_list::ListState;

/// Determine wheter or not mangas are being searched
/// if so then this should not make a request until the most recent one finishes
#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
enum PageState {
    SearchingMangas,
    DisplayingMangasFound,
    NotFound,
    #[default]
    Normal,
}

/// These happens in the background
pub enum SearchPageEvents {
    LoadCover(Option<DynamicImage>, String),
    LoadMangasFound(Option<SearchMangaResponse>),
}

/// These are actions that the user actively does
pub enum SearchPageActions {
    StartTyping,
    StopTyping,
    Search,
    ScrollUp,
    ScrollDown,
    NextPage,
    PreviousPage,
    GoToMangaPage,
}

#[derive(Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum InputMode {
    Typing,
    #[default]
    Idle,
}

///This is the "page" where the user can search for a manga
pub struct SearchPage {
    /// This tx "talks" to the app
    global_event_tx: UnboundedSender<Events>,
    picker: Picker,
    local_action_tx: UnboundedSender<SearchPageActions>,
    pub local_action_rx: UnboundedReceiver<SearchPageActions>,
    local_event_tx: UnboundedSender<SearchPageEvents>,
    pub local_event_rx: UnboundedReceiver<SearchPageEvents>,
    pub input_mode: InputMode,
    search_bar: Input,
    fetch_client: Arc<MangadexClient>,
    state: PageState,
    mangas_found_list: MangasFoundList,
    search_cover_handles: Vec<Option<JoinHandle<()>>>,
}

/// This contains the data the application gets when doing a search
#[derive(Default)]
struct MangasFoundList {
    widget: ListMangasFoundWidget,
    state: tui_widget_list::ListState,
    total_result: i32,
    page: i32,
}

impl Component for SearchPage {
    type Actions = SearchPageActions;
    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) {
        let search_page_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Max(4), Constraint::Fill(1)]);

        let [input_area, manga_area] = search_page_layout.areas(area);

        self.render_input_area(input_area, frame);

        self.render_manga_found_area(manga_area, frame.buffer_mut());
    }

    fn update(&mut self, action: SearchPageActions) {
        match action {
            SearchPageActions::StartTyping => self.focus_search_bar(),
            SearchPageActions::StopTyping => self.input_mode = InputMode::Idle,
            SearchPageActions::Search => {
                self.mangas_found_list.page = 1;
                self.search_mangas(1);
            }
            SearchPageActions::ScrollUp => self.scroll_up(),
            SearchPageActions::ScrollDown => self.scroll_down(),
            SearchPageActions::NextPage => {
                if self.state == PageState::DisplayingMangasFound
                    && self.state != PageState::SearchingMangas
                    && self.mangas_found_list.page * 10 < self.mangas_found_list.total_result
                {
                    self.mangas_found_list.page += 1;
                    self.search_mangas(self.mangas_found_list.page);
                }
            }
            SearchPageActions::PreviousPage => {
                if self.state == PageState::DisplayingMangasFound
                    && self.state != PageState::SearchingMangas
                    && self.mangas_found_list.page != 1
                {
                    self.mangas_found_list.page -= 1;
                    self.search_mangas(self.mangas_found_list.page);
                }
            }
            SearchPageActions::GoToMangaPage => {
                let manga_selected = self.get_current_manga_selected();
                if let Some(manga) = manga_selected {
                    self.global_event_tx
                        .send(Events::GoToMangaPage(manga))
                        .unwrap();
                }
            }
        }
    }
    fn handle_events(&mut self, events: Events) {
        match events {
            Events::Key(key_event) => self.handle_key_events(key_event),
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
            local_action_tx: action_tx,
            local_action_rx: action_rx,
            local_event_tx,
            local_event_rx: local_event,
            input_mode: InputMode::default(),
            search_bar: Input::default(),
            fetch_client: client,
            state: PageState::default(),
            mangas_found_list: MangasFoundList::default(),
            search_cover_handles: vec![],
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
                InputMode::Typing => "Press <enter> to search, <esc> to stop typing",
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

    fn render_manga_found_area(&mut self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)]);

        let [manga_list_area, preview_area] = layout.areas(area);

        match self.state {
            PageState::Normal => {
                Block::bordered().render(area, buf);
            }
            PageState::SearchingMangas => {
                Block::bordered().render(area, buf);
            }
            PageState::NotFound => {
                Block::bordered()
                    .title("No mangas were found")
                    .render(area, buf);
            }
            PageState::DisplayingMangasFound => {
                StatefulWidgetRef::render_ref(
                    &self.mangas_found_list.widget,
                    manga_list_area,
                    buf,
                    &mut self.mangas_found_list.state,
                );

                let total_pages = self.mangas_found_list.total_result as f64 / 10_f64;
                Block::default()
                    .title_bottom(format!(
                        "Page: {} of {}, total : {}",
                        self.mangas_found_list.page,
                        total_pages.ceil(),
                        self.mangas_found_list.total_result
                    ))
                    .render(manga_list_area, buf);

                if let Some(manga_selected) = self.get_current_manga_selected_mut() {
                    StatefulWidget::render(
                        MangaPreview::new(
                            &manga_selected.title,
                            &manga_selected.description,
                            &manga_selected.tags,
                            &manga_selected.content_rating,
                            &manga_selected.status,
                        ),
                        preview_area,
                        buf,
                        &mut manga_selected.image_state,
                    )
                }
            }
        }
    }

    fn focus_search_bar(&mut self) {
        self.input_mode = InputMode::Typing;
    }

    fn scroll_down(&mut self) {
        self.mangas_found_list.state.next();
    }

    fn scroll_up(&mut self) {
        self.mangas_found_list.state.previous();
    }

    fn get_current_manga_selected_mut(&mut self) -> Option<&mut MangaItem> {
        if let Some(index) = self.mangas_found_list.state.selected {
            return self.mangas_found_list.widget.mangas.get_mut(index);
        }
        None
    }

    fn get_current_manga_selected(&mut self) -> Option<MangaItem> {
        if let Some(index) = self.mangas_found_list.state.selected {
            return Some(self.mangas_found_list.widget.mangas[index].clone());
        }
        None
    }

    fn abort_search_cover_handles(&mut self) {
        if !self.search_cover_handles.is_empty() {
            for handle in self.search_cover_handles.iter() {
                match handle {
                    Some(running_task) => {
                        running_task.abort();
                    }
                    None => {}
                }
            }
            self.search_cover_handles.clear();
        }
    }

    /// This method is used to "forget" the data stored in the search page
    pub fn clean(&mut self) {
        self.state = PageState::default();
        self.input_mode = InputMode::Idle;
        self.search_bar.reset();
        self.abort_search_cover_handles();
        self.mangas_found_list.state = ListState::default();
        self.mangas_found_list.widget.mangas.clear();
    }

    fn handle_key_events(&mut self, key_event: KeyEvent) {
        match self.input_mode {
            InputMode::Idle => match key_event.code {
                KeyCode::Char('s') => {
                    self.local_action_tx
                        .send(SearchPageActions::StartTyping)
                        .unwrap();
                }
                KeyCode::Char('j') => self
                    .local_action_tx
                    .send(SearchPageActions::ScrollDown)
                    .unwrap(),

                KeyCode::Char('k') => self
                    .local_action_tx
                    .send(SearchPageActions::ScrollUp)
                    .unwrap(),
                KeyCode::Char('w') => self
                    .local_action_tx
                    .send(SearchPageActions::NextPage)
                    .unwrap(),
                KeyCode::Char('b') => self
                    .local_action_tx
                    .send(SearchPageActions::PreviousPage)
                    .unwrap(),
                KeyCode::Char('r') => self
                    .local_action_tx
                    .send(SearchPageActions::GoToMangaPage)
                    .unwrap(),
                _ => {}
            },
            InputMode::Typing => match key_event.code {
                KeyCode::Enter => {
                    if self.state != PageState::SearchingMangas {
                        self.local_action_tx
                            .send(SearchPageActions::Search)
                            .unwrap();
                    }
                }
                KeyCode::Esc => {
                    self.local_action_tx
                        .send(SearchPageActions::StopTyping)
                        .unwrap();
                }
                _ => {
                    self.search_bar.handle_event(&event::Event::Key(key_event));
                }
            },
        }
    }

    fn search_mangas(&mut self, page: i32) {
        self.abort_search_cover_handles();
        if !self.mangas_found_list.widget.mangas.is_empty() {
            self.mangas_found_list.widget.mangas.clear();
        }
        self.state = PageState::SearchingMangas;
        self.mangas_found_list.state = tui_widget_list::ListState::default();

        let tx = self.local_event_tx.clone();
        let client = Arc::clone(&self.fetch_client);
        let manga_to_search = self.search_bar.value().to_string();

        tokio::spawn(async move {
            let search_response = client.search_mangas(&manga_to_search, page).await;

            match search_response {
                Ok(mangas_found) => {
                    if mangas_found.data.is_empty() {
                        tx.send(SearchPageEvents::LoadMangasFound(None)).unwrap();
                    } else {
                        tx.send(SearchPageEvents::LoadMangasFound(Some(mangas_found)))
                            .unwrap();
                    }
                }
                Err(e) => {
                    panic!("could not fetch mangas : {e}");
                    tx.send(SearchPageEvents::LoadMangasFound(None)).unwrap();
                }
            }
        });
    }

    pub fn tick(&mut self) {
        if let Ok(event) = self.local_event_rx.try_recv() {
            match event {
                SearchPageEvents::LoadMangasFound(response) => {
                    match response {
                        Some(response) => {
                            let mut mangas: Vec<MangaItem> = vec![];

                            for manga in response.data.iter() {
                                let manga_id = manga.id.clone();
                                let client = Arc::clone(&self.fetch_client);
                                let tx = self.local_event_tx.clone();

                                let img_metadata = manga
                                    .relationships
                                    .iter()
                                    .find(|relation| relation.attributes.is_some());

                                let img_url = match img_metadata {
                                    Some(data) => {
                                        data.attributes.as_ref().map(|cover_img_attributes| {
                                            cover_img_attributes.file_name.clone()
                                        })
                                    }
                                    None => None,
                                };

                                let search_cover_task = match img_url {
                                    Some(file_name) => {
                                        let handle = tokio::spawn(async move {
                                            let response = client
                                                .get_cover_for_manga(&manga_id, &file_name)
                                                .await;

                                            match response {
                                                Ok(bytes) => {
                                                    let dyn_img = Reader::new(Cursor::new(bytes))
                                                        .with_guessed_format()
                                                        .unwrap();

                                                    let maybe_decoded = dyn_img.decode();
                                                    match maybe_decoded {
                                                        Ok(image) => {
                                                            tx.send(SearchPageEvents::LoadCover(
                                                                Some(image),
                                                                manga_id,
                                                            ))
                                                            .unwrap();
                                                        }
                                                        Err(_) => {
                                                            tx.send(SearchPageEvents::LoadCover(
                                                                None, manga_id,
                                                            ))
                                                            .unwrap();
                                                        }
                                                    };
                                                }
                                                Err(_) => tx
                                                    .send(SearchPageEvents::LoadCover(
                                                        None, manga_id,
                                                    ))
                                                    .unwrap(),
                                            }
                                        });
                                        Some(handle)
                                    }
                                    None => {
                                        tx.send(SearchPageEvents::LoadCover(None, manga_id))
                                            .unwrap();
                                        None
                                    }
                                };
                                self.search_cover_handles.push(search_cover_task);
                                mangas.push(MangaItem::from(manga.clone()));
                            }
                            self.mangas_found_list.widget = ListMangasFoundWidget::new(mangas);
                            self.mangas_found_list.total_result = response.total;
                            self.state = PageState::DisplayingMangasFound;
                        }
                        // Todo indicate that mangas where not found
                        None => {
                            self.state = PageState::NotFound;
                            self.mangas_found_list.total_result = 0;
                        }
                    }
                }

                SearchPageEvents::LoadCover(maybe_image, manga_id) => match maybe_image {
                    Some(image) => {
                        let image = self.picker.new_resize_protocol(image);

                        if let Some(manga) = self
                            .mangas_found_list
                            .widget
                            .mangas
                            .iter_mut()
                            .find(|manga| manga.id == manga_id)
                        {
                            manga.image_state = Some(image);
                        }
                    }
                    None => {
                        // todo! indicate that for the manga the cover could not be loaded
                    }
                },
            }
        }
    }
}
