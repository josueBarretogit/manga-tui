use crate::backend::database::save_plan_to_read;
use crate::backend::database::MangaPlanToReadSave;
use crate::backend::error_log::write_to_error_log;
use crate::backend::error_log::ErrorType;
use crate::backend::fetch::MangadexClient;
use crate::backend::tags::TagsResponse;
use crate::backend::tui::Events;
use crate::backend::SearchMangaResponse;
use crate::utils::search_manga_cover;
use crate::view::widgets::filter_widget::FilterState;
use crate::view::widgets::filter_widget::FilterWidget;
use crate::view::widgets::search::*;
use crate::view::widgets::Component;
use crate::view::widgets::ImageHandler;
use crate::PICKER;
use crossterm::event::KeyEvent;
use crossterm::event::{self, KeyCode};
use image::DynamicImage;
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;
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
    SearchCovers,
    SearchTags,
    LoadTags(TagsResponse),
    LoadCover(Option<DynamicImage>, String),
    LoadMangasFound(Option<SearchMangaResponse>),
}

impl ImageHandler for SearchPageEvents {
    fn load(image: DynamicImage, id: String) -> Self {
        Self::LoadCover(Some(image), id)
    }
    fn not_found(id: String) -> Self {
        Self::LoadCover(None, id)
    }
}

/// These are actions that the user actively does
pub enum SearchPageActions {
    StartTyping,
    StopTyping,
    Search,
    ScrollUp,
    ScrollDown,
    ToggleFilters,
    NextPage,
    PreviousPage,
    GoToMangaPage,
    PlanToRead,
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
    local_action_tx: UnboundedSender<SearchPageActions>,
    pub local_action_rx: UnboundedReceiver<SearchPageActions>,
    local_event_tx: UnboundedSender<SearchPageEvents>,
    pub local_event_rx: UnboundedReceiver<SearchPageEvents>,
    pub input_mode: InputMode,
    search_bar: Input,
    state: PageState,
    mangas_found_list: MangasFoundList,
    filter_state: FilterState,
    tasks: JoinSet<()>,
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
            SearchPageActions::ToggleFilters => self.open_advanced_filters(),
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
                        .send(Events::GoToMangaPage(manga.clone()))
                        .ok();
                }
            }
            SearchPageActions::PlanToRead => self.plan_to_read(),
        }
    }
    fn handle_events(&mut self, events: Events) {
        match events {
            Events::Key(key_event) => {
                if self.filter_state.is_open {
                    self.filter_state.handle_key_events(key_event);
                } else {
                    self.handle_key_events(key_event);
                }
            }
            Events::Tick => self.tick(),
            _ => {}
        }
    }
    fn clean_up(&mut self) {
        self.state = PageState::default();
        self.input_mode = InputMode::Idle;
        self.abort_tasks();
        self.mangas_found_list.state = ListState::default();
        if !self.mangas_found_list.widget.mangas.is_empty() {
            self.mangas_found_list.widget.mangas.clear();
        }
    }
}

impl SearchPage {
    pub fn init(event_tx: UnboundedSender<Events>) -> Self {
        let (action_tx, action_rx) = mpsc::unbounded_channel::<SearchPageActions>();
        let (local_event_tx, local_event) = mpsc::unbounded_channel::<SearchPageEvents>();

        local_event_tx.send(SearchPageEvents::SearchTags).ok();

        Self {
            global_event_tx: event_tx,
            local_action_tx: action_tx,
            local_action_rx: action_rx,
            local_event_tx,
            local_event_rx: local_event,
            input_mode: InputMode::default(),
            search_bar: Input::default(),
            state: PageState::default(),
            mangas_found_list: MangasFoundList::default(),
            tasks: JoinSet::new(),
            filter_state: FilterState::default(),
        }
    }

    fn search_tags(&mut self) {

        self.tasks.spawn(async move {
            
        });
    }

    fn render_input_area(&self, area: Rect, frame: &mut Frame<'_>) {
        let layout = Layout::vertical([Constraint::Max(1), Constraint::Max(5)]).split(area);

        let (input_help, input_style) = match self.input_mode {
            InputMode::Idle => (
                "Press <s> to type, open advanced filters: <f> ",
                Style::default(),
            ),
            InputMode::Typing => (
                "Press <enter> to search, <esc> to stop typing",
                Style::default().fg(Color::Yellow),
            ),
        };

        let input_bar = Paragraph::new(self.search_bar.value()).block(
            Block::bordered()
                .title(input_help)
                .border_style(input_style),
        );

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
                let total_pages = self.mangas_found_list.total_result as f64 / 10_f64;

                let list_instructions = Line::from(vec![
                    "Go down ".into(),
                    "<j> ".bold().blue(),
                    "Go up ".into(),
                    "<k> ".bold().blue(),
                    "Plan to read ".into(),
                    "<p> ".bold().fg(Color::Yellow),
                    "Read ".into(),
                    "<r> ".bold().fg(Color::Yellow),
                ]);

                let pagination_instructions = Line::from(vec![
                    format!(
                        "Page : {} of {}, total : {} ",
                        self.mangas_found_list.page,
                        total_pages.ceil(),
                        self.mangas_found_list.total_result
                    )
                    .into(),
                    "Next ".into(),
                    "<w> ".bold().fg(Color::Yellow),
                    "Previous ".into(),
                    "<b> ".bold().fg(Color::Yellow),
                ]);

                Block::bordered()
                    .title_top(list_instructions)
                    .title_bottom(pagination_instructions)
                    .render(manga_list_area, buf);

                let inner_list_area = manga_list_area.inner(Margin {
                    horizontal: 1,
                    vertical: 1,
                });

                StatefulWidgetRef::render_ref(
                    &self.mangas_found_list.widget,
                    inner_list_area,
                    buf,
                    &mut self.mangas_found_list.state,
                );

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
        if self.filter_state.is_open {
            StatefulWidget::render(
                FilterWidget::new().block(
                    Block::bordered()
                        .title(Line::from(vec!["Close ".into(), "<f>".bold().yellow()])),
                ),
                area,
                buf,
                &mut self.filter_state,
            );
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

    fn open_advanced_filters(&mut self) {
        self.filter_state.toggle();
    }

    fn get_current_manga_selected_mut(&mut self) -> Option<&mut MangaItem> {
        if let Some(index) = self.mangas_found_list.state.selected {
            return self.mangas_found_list.widget.mangas.get_mut(index);
        }
        None
    }

    fn get_current_manga_selected(&self) -> Option<&MangaItem> {
        if let Some(index) = self.mangas_found_list.state.selected {
            return self.mangas_found_list.widget.mangas.get(index);
        }
        None
    }

    fn plan_to_read(&mut self) {
        if let Some(manga) = self.get_current_manga_selected() {
            let plan_to_read_op = save_plan_to_read(MangaPlanToReadSave {
                id: &manga.id,
                title: &manga.title,
                img_url: manga.img_url.as_deref(),
            });

            if let Err(e) = plan_to_read_op {
                write_to_error_log(ErrorType::FromError(Box::new(e)));
            }
        }
    }

    fn abort_tasks(&mut self) {
        self.tasks.abort_all();
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
                KeyCode::Char('p') => {
                    self.local_action_tx
                        .send(SearchPageActions::PlanToRead)
                        .ok();
                }
                KeyCode::Char('b') => self
                    .local_action_tx
                    .send(SearchPageActions::PreviousPage)
                    .unwrap(),
                KeyCode::Char('f') => {
                    self.local_action_tx
                        .send(SearchPageActions::ToggleFilters)
                        .ok();
                }
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
        self.clean_up();

        self.state = PageState::SearchingMangas;

        let tx = self.local_event_tx.clone();

        let manga_to_search = self.search_bar.value().to_string();

        let filters = self.filter_state.filters.clone();
        self.tasks.spawn(async move {
            let search_response = MangadexClient::global()
                .search_mangas(&manga_to_search, page, filters)
                .await;

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
                    write_to_error_log(ErrorType::FromError(Box::new(e)));
                    tx.send(SearchPageEvents::LoadMangasFound(None)).ok();
                }
            }
        });
    }

    pub fn tick(&mut self) {
        if let Ok(event) = self.local_event_rx.try_recv() {
            match event {
                SearchPageEvents::LoadTags(response) => todo!(),
                SearchPageEvents::SearchTags => todo!(),
                SearchPageEvents::LoadMangasFound(response) => {
                    match response {
                        Some(response) => {
                            self.mangas_found_list.widget =
                                ListMangasFoundWidget::from_response(response.data);
                            self.mangas_found_list.total_result = response.total;
                            self.state = PageState::DisplayingMangasFound;
                            if PICKER.is_some() {
                                self.local_event_tx
                                    .send(SearchPageEvents::SearchCovers)
                                    .ok();
                            }
                        }
                        // Todo indicate that mangas where not found
                        None => {
                            self.state = PageState::NotFound;
                            self.mangas_found_list.total_result = 0;
                        }
                    }
                }
                SearchPageEvents::SearchCovers => {
                    for manga in self.mangas_found_list.widget.mangas.iter() {
                        let manga_id = manga.id.clone();
                        let tx = self.local_event_tx.clone();

                        match manga.img_url.as_ref() {
                            Some(file_name) => {
                                let file_name = file_name.clone();
                                search_manga_cover(file_name, manga_id, &mut self.tasks, tx);
                            }
                            None => {
                                tx.send(SearchPageEvents::LoadCover(None, manga_id))
                                    .unwrap();
                            }
                        };
                    }
                }

                SearchPageEvents::LoadCover(maybe_image, manga_id) => match maybe_image {
                    Some(image) => {
                        let image = PICKER.unwrap().new_resize_protocol(image);

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
