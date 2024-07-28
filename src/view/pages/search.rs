use crate::backend::database::save_plan_to_read;
use crate::backend::database::MangaPlanToReadSave;
use crate::backend::error_log::write_to_error_log;
use crate::backend::error_log::ErrorType;
use crate::backend::fetch::MangadexClient;
use crate::backend::tui::Events;
use crate::backend::SearchMangaResponse;
use crate::common::Artist;
use crate::common::Author;
use crate::global::INSTRUCTIONS_STYLE;
use crate::utils::render_search_bar;
use crate::utils::search_manga_cover;
use crate::view::widgets::filter_widget::state::FilterState;
use crate::view::widgets::filter_widget::FilterWidget;
use crate::view::widgets::search::*;
use crate::view::widgets::Component;
use crate::view::widgets::ImageHandler;
use crate::view::widgets::StatefulWidgetFrame;
use crate::PICKER;
use crossterm::event::KeyEvent;
use crossterm::event::{self, KeyCode};
use ratatui::{prelude::*, widgets::*};
use ratatui_image::protocol::StatefulProtocol;
use throbber_widgets_tui::Throbber;
use throbber_widgets_tui::ThrobberState;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;
use tui_widget_list::ListState;

use self::text::ToSpan;

// Todo! display cover area loading
// Todo! display loading search
// Todo! indicate a manga has been added to plan to read

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
    LoadCover(Option<Box<dyn StatefulProtocol>>, String),
    LoadMangasFound(Option<SearchMangaResponse>),
}

impl ImageHandler for SearchPageEvents {
    fn load(image: Box<dyn StatefulProtocol>, id: String) -> Self {
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
    loader_state: ThrobberState,
    mangas_found_list: MangasFoundList,
    filter_state: FilterState,
    manga_added_to_plan_to_read: Option<String>,
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

        self.render_manga_found_area(manga_area, frame);
    }

    fn update(&mut self, action: SearchPageActions) {
        match action {
            SearchPageActions::ToggleFilters => self.open_advanced_filters(),
            SearchPageActions::StartTyping => self.focus_search_bar(),
            SearchPageActions::StopTyping => self.input_mode = InputMode::Idle,
            SearchPageActions::Search => {
                self.mangas_found_list.page = 1;
                self.search_mangas();
            }
            SearchPageActions::ScrollUp => self.scroll_up(),
            SearchPageActions::ScrollDown => self.scroll_down(),
            SearchPageActions::NextPage => self.search_next_page(),
            SearchPageActions::PreviousPage => self.search_previous_page(),
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
        if self.filter_state.is_open {
            self.filter_state.handle_events(events);
        } else {
            match events {
                Events::Key(key_event) => {
                    self.handle_key_events(key_event);
                }
                Events::Tick => self.tick(),
                _ => {}
            }
        }
    }
    fn clean_up(&mut self) {
        self.state = PageState::default();
        self.manga_added_to_plan_to_read = None;
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
            filter_state: FilterState::new(),
            loader_state: ThrobberState::default(),
            manga_added_to_plan_to_read: None,
        }
    }

    fn render_input_area(&self, area: Rect, frame: &mut Frame<'_>) {
        let [input_area, information_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(area);

        let input_help = match self.input_mode {
            InputMode::Idle => "Press <s> to type, open advanced filters: <f> ",
            InputMode::Typing => "Press <enter> to search, <esc> to stop typing",
        };

        render_search_bar(
            self.input_mode == InputMode::Typing,
            input_help.into(),
            &self.search_bar,
            frame,
            input_area,
        );

        if let Some(name) = self.manga_added_to_plan_to_read.as_ref() {
            Paragraph::new(
                format!("Added: {} to plan to read 📖", name)
                    .to_span()
                    .underlined(),
            )
            .wrap(Wrap { trim: true })
            .render(
                information_area.inner(Margin {
                    horizontal: 1,
                    vertical: 1,
                }),
                frame.buffer_mut(),
            );
        }
    }

    fn render_manga_found_area(&mut self, area: Rect, frame: &mut Frame<'_>) {
        let buf = frame.buffer_mut();
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)]);

        let [manga_list_area, preview_area] = layout.areas(area);

        match self.state {
            PageState::Normal => {
                Block::bordered().render(area, buf);
            }
            PageState::SearchingMangas => {
                let loader = Throbber::default()
                    .label("Searching mangas")
                    .style(Style::default().fg(Color::Yellow))
                    .throbber_set(throbber_widgets_tui::BRAILLE_SIX)
                    .use_type(throbber_widgets_tui::WhichUse::Spin);

                StatefulWidget::render(loader, area, buf, &mut self.loader_state);
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
                    Span::raw("<j>").style(*INSTRUCTIONS_STYLE),
                    " Go up ".into(),
                    Span::raw("<k>").style(*INSTRUCTIONS_STYLE),
                    " Plan to read ".into(),
                    Span::raw("<p>").style(*INSTRUCTIONS_STYLE),
                    " Read ".into(),
                    Span::raw("<r>").style(*INSTRUCTIONS_STYLE),
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
                    Span::raw("<w>").style(*INSTRUCTIONS_STYLE),
                    " Previous ".into(),
                    Span::raw("<b>").style(*INSTRUCTIONS_STYLE),
                ]);

                Block::bordered()
                    .title_top(list_instructions)
                    .title_bottom(pagination_instructions)
                    .render(manga_list_area, buf);

                let inner_list_area = manga_list_area.inner(Margin {
                    horizontal: 1,
                    vertical: 1,
                });

                if !self.filter_state.is_open {
                    StatefulWidgetRef::render_ref(
                        &self.mangas_found_list.widget,
                        inner_list_area,
                        buf,
                        &mut self.mangas_found_list.state,
                    );
                    let loader_state = self.loader_state.clone();
                    if let Some(manga_selected) = self.get_current_manga_selected_mut() {
                        StatefulWidget::render(
                            MangaPreview::new(
                                &manga_selected.manga.title,
                                &manga_selected.manga.description,
                                &manga_selected.manga.tags,
                                &manga_selected.manga.content_rating,
                                &manga_selected.manga.status,
                                loader_state,
                            ),
                            preview_area,
                            buf,
                            &mut manga_selected.image_state,
                        )
                    }
                }
            }
        }
        if self.filter_state.is_open {
            self.render_filters(area, frame);
        }
    }

    fn render_filters(&mut self, area: Rect, frame: &mut Frame<'_>) {
        let filter_instructions = Line::from(vec![
            "Close ".into(),
            Span::raw("<f>").style(*INSTRUCTIONS_STYLE),
            " Reset filters ".into(),
            Span::raw("<r>").style(*INSTRUCTIONS_STYLE),
        ]);

        FilterWidget::new()
            .block(Block::bordered().title(filter_instructions))
            .render(area, frame, &mut self.filter_state);
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
        if let Some(item) = self.get_current_manga_selected() {
            let plan_to_read_op = save_plan_to_read(MangaPlanToReadSave {
                id: &item.manga.id,
                title: &item.manga.title,
                img_url: item.manga.img_url.as_deref(),
            });

            match plan_to_read_op {
                Ok(()) => {
                    self.manga_added_to_plan_to_read = Some(item.manga.title.clone());
                }
                Err(e) => write_to_error_log(ErrorType::FromError(Box::new(e))),
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
    pub fn is_typing_filter(&mut self) -> bool {
        self.filter_state.is_typing
    }

    fn search_mangas(&mut self) {
        self.clean_up();

        self.state = PageState::SearchingMangas;

        let page = self.mangas_found_list.page;

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

    fn search_next_page(&mut self) {
        if self.state == PageState::DisplayingMangasFound
            && self.state != PageState::SearchingMangas
            && self.mangas_found_list.page * 10 < self.mangas_found_list.total_result
        {
            self.mangas_found_list.page += 1;
            self.search_mangas();
        }
    }

    fn search_previous_page(&mut self) {
        if self.state == PageState::DisplayingMangasFound
            && self.state != PageState::SearchingMangas
            && self.mangas_found_list.page != 1
        {
            self.mangas_found_list.page -= 1;
            self.search_mangas();
        }
    }

    pub fn search_mangas_of_author(&mut self, author: Author) {
        self.filter_state.set_author(author);
        self.search_bar.reset();
        self.mangas_found_list.page = 1;
        self.search_mangas();
    }

    pub fn search_mangas_of_artist(&mut self, artist: Artist) {
        self.filter_state.set_artist(artist);
        self.search_bar.reset();
        self.mangas_found_list.page = 1;
        self.search_mangas();
    }

    fn load_mangas_found(&mut self, response: Option<SearchMangaResponse>) {
        match response {
            Some(response) => {
                self.mangas_found_list.widget = ListMangasFoundWidget::from_response(response.data);
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

    pub fn tick(&mut self) {
        self.loader_state.calc_next();
        if let Ok(event) = self.local_event_rx.try_recv() {
            match event {
                SearchPageEvents::LoadMangasFound(response) => self.load_mangas_found(response),
                SearchPageEvents::SearchCovers => {
                    for item in self.mangas_found_list.widget.mangas.iter() {
                        let manga_id = item.manga.id.clone();
                        let tx = self.local_event_tx.clone();

                        match item.manga.img_url.as_ref() {
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
                        if let Some(manga) = self
                            .mangas_found_list
                            .widget
                            .mangas
                            .iter_mut()
                            .find(|manga_item| manga_item.manga.id == manga_id)
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
