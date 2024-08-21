use crossterm::event::{self, KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::prelude::*;
use ratatui::widgets::*;
use ratatui_image::protocol::StatefulProtocol;
use throbber_widgets_tui::{Throbber, ThrobberState};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;
use tui_widget_list::ListState;

use self::text::ToSpan;
use crate::backend::database::{save_plan_to_read, MangaPlanToReadSave};
use crate::backend::error_log::{write_to_error_log, ErrorType};
use crate::backend::fetch::MangadexClient;
use crate::backend::tui::Events;
use crate::backend::SearchMangaResponse;
use crate::common::{Artist, Author};
use crate::global::{ERROR_STYLE, INSTRUCTIONS_STYLE};
use crate::utils::{render_search_bar, search_manga_cover};
use crate::view::widgets::filter_widget::state::FilterState;
use crate::view::widgets::filter_widget::FilterWidget;
use crate::view::widgets::search::*;
use crate::view::widgets::{Component, ImageHandler, StatefulWidgetFrame};
use crate::PICKER;

/// Determine wheter or not mangas are being searched
/// if so then this should not make a request until the most recent one finishes
#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
enum PageState {
    SearchingMangas,
    DisplayingMangasFound,
    NotFound,
    ErrorOcurred,
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
#[derive(Debug, PartialEq, Eq)]
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
    total_result: u32,
    page: u32,
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
            },
            SearchPageActions::ScrollUp => self.scroll_up(),
            SearchPageActions::ScrollDown => self.scroll_down(),
            SearchPageActions::NextPage => self.search_next_page(),
            SearchPageActions::PreviousPage => self.search_previous_page(),
            SearchPageActions::GoToMangaPage => {
                let manga_selected = self.get_current_manga_selected();
                if let Some(manga) = manga_selected {
                    self.global_event_tx.send(Events::GoToMangaPage(manga.clone())).ok();
                }
            },
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
                },
                Events::Mouse(mouse_event) => self.handle_mouse_events(mouse_event),
                Events::Tick => self.tick(),
                _ => {},
            }
        }
    }

    fn clean_up(&mut self) {
        self.abort_tasks();
        self.state = PageState::default();
        self.manga_added_to_plan_to_read = None;
        self.input_mode = InputMode::Idle;
        self.mangas_found_list.state = ListState::default();
        if !self.mangas_found_list.widget.mangas.is_empty() {
            self.mangas_found_list.widget.mangas = vec![];
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
        let [input_area, information_area] = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(area);

        let input_help = match self.input_mode {
            InputMode::Idle => Line::from(vec![
                "Press ".into(),
                "<s>".to_span().style(*INSTRUCTIONS_STYLE),
                " to search mangas ".into(),
                "<f>".to_span().style(*INSTRUCTIONS_STYLE),
                " to open advanced filters".into(),
            ]),
            InputMode::Typing => Line::from(vec![
                "Press ".into(),
                "<Enter>".to_span().style(*INSTRUCTIONS_STYLE),
                " to search ".into(),
                "<Esc>".to_span().style(*INSTRUCTIONS_STYLE),
                " to stop typing".into(),
            ]),
        };

        render_search_bar(self.input_mode == InputMode::Typing, input_help, &self.search_bar, frame, input_area);

        if let Some(name) = self.manga_added_to_plan_to_read.as_ref() {
            Paragraph::new(format!("Added: {} to plan to read 📖", name).to_span().underlined())
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
        let [manga_list_area, preview_area] =
            Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]).areas(area);

        match self.state {
            PageState::Normal => {
                Block::bordered().render(area, buf);
            },
            PageState::SearchingMangas => {
                let loader = Throbber::default()
                    .label("Searching mangas")
                    .style(Style::default().fg(Color::Yellow))
                    .throbber_set(throbber_widgets_tui::BRAILLE_SIX)
                    .use_type(throbber_widgets_tui::WhichUse::Spin);

                StatefulWidget::render(loader, area, buf, &mut self.loader_state);
            },
            PageState::NotFound => {
                Block::bordered().title("No mangas were found").render(area, buf);
            },
            PageState::ErrorOcurred => {
                Block::bordered()
                    .title("An error ocurred when searching mangas, please try again".to_span().style(*ERROR_STYLE))
                    .render(area, buf);
            },
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
            },
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
            let plan_to_read_operation = save_plan_to_read(MangaPlanToReadSave {
                id: &item.manga.id,
                title: &item.manga.title,
                img_url: item.manga.img_url.as_deref(),
            });

            match plan_to_read_operation {
                Ok(()) => {
                    self.manga_added_to_plan_to_read = Some(item.manga.title.clone());
                },
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
                    self.local_action_tx.send(SearchPageActions::StartTyping).ok();
                },
                KeyCode::Char('j') | KeyCode::Down => {
                    self.local_action_tx.send(SearchPageActions::ScrollDown).ok();
                },

                KeyCode::Char('k') | KeyCode::Up => {
                    self.local_action_tx.send(SearchPageActions::ScrollUp).ok();
                },
                KeyCode::Char('w') => {
                    self.local_action_tx.send(SearchPageActions::NextPage).ok();
                },
                KeyCode::Char('p') => {
                    self.local_action_tx.send(SearchPageActions::PlanToRead).ok();
                },
                KeyCode::Char('b') => {
                    self.local_action_tx.send(SearchPageActions::PreviousPage).ok();
                },
                KeyCode::Char('f') => {
                    self.local_action_tx.send(SearchPageActions::ToggleFilters).ok();
                },
                KeyCode::Char('r') | KeyCode::Enter => {
                    self.local_action_tx.send(SearchPageActions::GoToMangaPage).ok();
                },

                _ => {},
            },
            InputMode::Typing => match key_event.code {
                KeyCode::Enter => {
                    if self.state != PageState::SearchingMangas {
                        self.local_action_tx.send(SearchPageActions::Search).ok();
                    }
                },
                KeyCode::Esc => {
                    self.local_action_tx.send(SearchPageActions::StopTyping).ok();
                },
                _ => {
                    self.search_bar.handle_event(&event::Event::Key(key_event));
                },
            },
        }
    }

    fn handle_mouse_events(&mut self, mouse_event: MouseEvent) {
        match mouse_event.kind {
            MouseEventKind::ScrollDown => {
                self.local_action_tx.send(SearchPageActions::ScrollDown).ok();
            },
            MouseEventKind::ScrollUp => {
                self.local_action_tx.send(SearchPageActions::ScrollUp).ok();
            },
            MouseEventKind::Down(button) => {
                if button == MouseButton::Left {
                    self.local_action_tx.send(SearchPageActions::GoToMangaPage).ok();
                }
            },
            _ => {},
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
            let search_response = MangadexClient::global().search_mangas(&manga_to_search, page, filters).await;

            match search_response {
                Ok(mangas_found) => {
                    tx.send(SearchPageEvents::LoadMangasFound(Some(mangas_found))).ok();
                },
                Err(e) => {
                    write_to_error_log(ErrorType::FromError(Box::new(e)));
                    tx.send(SearchPageEvents::LoadMangasFound(None)).ok();
                },
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
                if response.data.is_empty() {
                    self.state = PageState::NotFound;
                    self.mangas_found_list.total_result = 0;
                    return;
                }
                self.mangas_found_list.widget = ListMangasFoundWidget::from_response(response.data);
                self.mangas_found_list.total_result = response.total;
                self.state = PageState::DisplayingMangasFound;
                if PICKER.is_some() {
                    self.local_event_tx.send(SearchPageEvents::SearchCovers).ok();
                }
            },
            None => {
                self.state = PageState::ErrorOcurred;
                self.mangas_found_list.total_result = 0;
            },
        }
    }

    fn search_covers(&mut self) {
        for item in self.mangas_found_list.widget.mangas.iter() {
            let manga_id = item.manga.id.clone();
            let tx = self.local_event_tx.clone();

            match item.manga.img_url.as_ref() {
                Some(file_name) => {
                    let file_name = file_name.clone();
                    search_manga_cover(file_name, manga_id, &mut self.tasks, tx);
                },
                None => {
                    tx.send(SearchPageEvents::LoadCover(None, manga_id)).ok();
                },
            };
        }
    }

    fn load_cover(&mut self, maybe_cover: Option<Box<dyn StatefulProtocol>>, manga_id: String) {
        if let Some(image) = maybe_cover {
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
    }

    pub fn tick(&mut self) {
        self.loader_state.calc_next();
        if let Ok(event) = self.local_event_rx.try_recv() {
            match event {
                SearchPageEvents::LoadMangasFound(response) => self.load_mangas_found(response),
                SearchPageEvents::SearchCovers => {
                    self.search_covers();
                },
                SearchPageEvents::LoadCover(maybe_image, manga_id) => self.load_cover(maybe_image, manga_id),
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::view::widgets::press_key;

    #[tokio::test]
    async fn search_page_key_events() {
        let (tx, _) = mpsc::unbounded_channel::<Events>();
        let mut search_page = SearchPage::init(tx);

        assert!(search_page.state == PageState::Normal);
        assert!(!search_page.filter_state.is_open);

        // focus search_bar
        press_key(&mut search_page, KeyCode::Char('s'));

        if let Some(action) = search_page.local_action_rx.recv().await {
            search_page.update(action)
        }

        assert!(search_page.input_mode == InputMode::Typing);

        // user is typing in the search_bar
        press_key(&mut search_page, KeyCode::Char('t'));
        press_key(&mut search_page, KeyCode::Char('e'));

        assert_eq!("te", search_page.search_bar.value());

        // unfocus search_bar
        press_key(&mut search_page, KeyCode::Esc);

        if let Some(action) = search_page.local_action_rx.recv().await {
            search_page.update(action)
        }

        assert!(search_page.input_mode == InputMode::Idle);

        // Assuming a search was made and some mangas were found
        search_page.state = PageState::DisplayingMangasFound;
        search_page.mangas_found_list.widget.mangas = vec![MangaItem::default(), MangaItem::default()];
        search_page.mangas_found_list.total_result = 20;
        search_page.mangas_found_list.page = 1;

        let area = Rect::new(0, 0, 50, 50);
        let mut buf = Buffer::empty(area);

        // Render the list of mangas found
        StatefulWidgetRef::render_ref(
            &search_page.mangas_found_list.widget,
            area,
            &mut buf,
            &mut search_page.mangas_found_list.state,
        );

        // scroll down the list
        press_key(&mut search_page, KeyCode::Char('j'));

        if let Some(action) = search_page.local_action_rx.recv().await {
            search_page.update(action)
        }

        assert!(search_page.mangas_found_list.state.selected.is_some());

        // open filters
        press_key(&mut search_page, KeyCode::Char('f'));

        if let Some(action) = search_page.local_action_rx.recv().await {
            search_page.update(action)
        }

        assert!(search_page.filter_state.is_open);

        search_page.filter_state.is_open = false;

        // // Add a manga to plan to read
        // To test the actual funcionality it's necessary the database, so let's assert the right
        // event is called in the meantime
        // press_key(&mut search_page, KeyCode::Char('p'));
        //
        // if let Some(action) = search_page.local_action_rx.recv().await {
        //     search_page.update(action)
        // }
        //
        // assert!(search_page.manga_added_to_plan_to_read.is_some());

        // Add a manga to plan to read
        press_key(&mut search_page, KeyCode::Char('p'));

        if let Some(action) = search_page.local_action_rx.recv().await {
            assert_eq!(SearchPageActions::PlanToRead, action);
        } else {
            panic!("Add plan to read functionality is not being called");
        }

        // Go next page
        press_key(&mut search_page, KeyCode::Char('w'));

        if let Some(action) = search_page.local_action_rx.recv().await {
            search_page.update(action)
        }

        assert_eq!(2, search_page.mangas_found_list.page);

        search_page.state = PageState::DisplayingMangasFound;

        // Go previous page
        press_key(&mut search_page, KeyCode::Char('b'));

        if let Some(action) = search_page.local_action_rx.recv().await {
            search_page.update(action)
        }
        assert_eq!(1, search_page.mangas_found_list.page);

        // Go to manga page
        press_key(&mut search_page, KeyCode::Char('r'));

        if let Some(action) = search_page.local_action_rx.recv().await {
            assert_eq!(SearchPageActions::GoToMangaPage, action);
        } else {
            panic!("The action `go to manga page` is not working");
        }
    }
}
