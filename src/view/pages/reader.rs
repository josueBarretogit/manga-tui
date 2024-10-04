use std::error::Error;
use std::future::Future;

use crossterm::event::{KeyCode, KeyEvent};
use image::DynamicImage;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, StatefulWidget, Widget};
use ratatui::Frame;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};
use reqwest::Url;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;

use crate::backend::filter::Languages;
use crate::backend::tui::Events;
use crate::global::INSTRUCTIONS_STYLE;
use crate::view::tasks::reader::get_manga_panel;
use crate::view::widgets::reader::{PageItemState, PagesItem, PagesList};
use crate::view::widgets::Component;

pub trait SearchChapter: Send + Clone + 'static {
    fn search_chapter(
        &self,
        manga_id: &str,
        volume_number: &str,
        chapter_number: &str,
        language: Languages,
    ) -> impl Future<Output = Result<Option<Chapter>, Box<dyn Error>>> + Send;
}

pub trait SearchMangaPanel: Send + Clone + 'static {
    fn search_manga_panel(&self, endpoint: Url) -> impl Future<Output = Result<MangaPanel, Box<dyn Error>>> + Send;
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct MangaPanel {
    pub image_decoded: DynamicImage,
    pub dimensions: (u32, u32),
}

#[derive(Debug, PartialEq, Eq)]
pub enum MangaReaderActions {
    SearchNextChapter,
    NextPage,
    PreviousPage,
}

#[derive(Debug, PartialEq, Eq)]
pub enum State {
    ErrorSearchingChapter,
    DisplayingChapterNotFound,
    SearchingChapter,
    SearchingPages,
}

#[derive(Debug, PartialEq, Clone)]
pub struct PageData {
    pub panel: MangaPanel,
    pub index: usize,
}

#[derive(Debug, PartialEq)]
pub enum MangaReaderEvents {
    ErrorSearchingChapter,
    ChapterNotFound,
    LoadChapter(Chapter),
    SearchNextChapter,
    FetchPages,
    LoadPage(Option<PageData>),
}

pub struct Page {
    pub image_state: Option<Box<dyn StatefulProtocol>>,
    pub dimensions: Option<(u32, u32)>,
}

impl Page {
    pub fn new() -> Self {
        Self {
            image_state: None,
            dimensions: None,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Chapter {
    pub id: String,
    pub number: u32,
    pub volume_number: Option<u32>,
    pub pages_url: Vec<Url>,
    pub language: Languages,
}

impl Default for Chapter {
    fn default() -> Self {
        Self {
            id: String::default(),
            number: 1,
            volume_number: Some(1),
            pages_url: vec![],
            language: Languages::default(),
        }
    }
}

pub struct MangaReader<T: SearchChapter + SearchMangaPanel> {
    manga_id: String,
    chapter: Chapter,
    pages: Vec<Page>,
    pages_list: PagesList,
    current_page_size: u16,
    page_list_state: tui_widget_list::ListState,
    state: State,
    image_tasks: JoinSet<()>,
    picker: Picker,
    api_client: T,
    pub _global_event_tx: Option<UnboundedSender<Events>>,
    pub local_action_tx: UnboundedSender<MangaReaderActions>,
    pub local_action_rx: UnboundedReceiver<MangaReaderActions>,
    pub local_event_tx: UnboundedSender<MangaReaderEvents>,
    pub local_event_rx: UnboundedReceiver<MangaReaderEvents>,
}

impl<T: SearchChapter + SearchMangaPanel> Component for MangaReader<T> {
    type Actions = MangaReaderActions;

    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) {
        let buf = frame.buffer_mut();

        let layout =
            Layout::horizontal([Constraint::Fill(1), Constraint::Fill(self.current_page_size), Constraint::Fill(1)]).spacing(1);

        let [left, center, right] = layout.areas(area);

        Block::bordered().render(left, buf);
        self.render_page_list(left, buf);

        Paragraph::new(Line::from(vec!["Go back: ".into(), Span::raw("<Backspace>").style(*INSTRUCTIONS_STYLE)]))
            .render(right, buf);

        self.render_instructions(
            buf,
            right.inner(Margin {
                horizontal: 2,
                vertical: 2,
            }),
        );

        match self.pages.get_mut(self.page_list_state.selected.unwrap_or(0)) {
            Some(page) => match page.image_state.as_mut() {
                Some(img_state) => {
                    let (width, height) = page.dimensions.unwrap();
                    if width > height {
                        if width - height > 250 {
                            self.current_page_size = 5;
                        }
                    } else {
                        self.current_page_size = 2;
                    }
                    let image = StatefulImage::new(None).resize(Resize::Fit(None));
                    StatefulWidget::render(image, center, buf, img_state);
                },
                None => {
                    Block::bordered().title("Loading page").render(center, frame.buffer_mut());
                },
            },
            None => Block::bordered().title("Loading page").render(center, frame.buffer_mut()),
        };
    }

    fn update(&mut self, action: Self::Actions) {
        match action {
            MangaReaderActions::SearchNextChapter => self.initiate_search_next_chapter(),
            MangaReaderActions::NextPage => self.next_page(),
            MangaReaderActions::PreviousPage => self.previous_page(),
        }
    }

    fn handle_events(&mut self, events: crate::backend::tui::Events) {
        match events {
            Events::Key(key_event) => self.handle_key_events(key_event),
            Events::Mouse(mouse_event) => match mouse_event.kind {
                crossterm::event::MouseEventKind::ScrollUp => {
                    self.local_action_tx.send(MangaReaderActions::PreviousPage).ok();
                },
                crossterm::event::MouseEventKind::ScrollDown => {
                    self.local_action_tx.send(MangaReaderActions::NextPage).ok();
                },
                _ => {},
            },
            Events::Tick => self.tick(),
            _ => {},
        }
    }

    fn clean_up(&mut self) {
        self.image_tasks.abort_all();
        self.pages = vec![];
        self.pages_list.pages = vec![];
        self.page_list_state = tui_widget_list::ListState::default();
    }
}

impl<T: SearchChapter + SearchMangaPanel> MangaReader<T> {
    pub fn new(chapter: Chapter, manga_id: String, picker: Picker, api_client: T) -> Self {
        let set: JoinSet<()> = JoinSet::new();
        let (local_action_tx, local_action_rx) = mpsc::unbounded_channel::<MangaReaderActions>();
        let (local_event_tx, local_event_rx) = mpsc::unbounded_channel::<MangaReaderEvents>();

        Self {
            _global_event_tx: None,
            chapter,
            pages: vec![],
            manga_id,
            page_list_state: tui_widget_list::ListState::default(),
            image_tasks: set,
            local_action_tx,
            local_action_rx,
            local_event_tx,
            local_event_rx,
            state: State::SearchingPages,
            current_page_size: 2,
            pages_list: PagesList::default(),
            picker,
            api_client,
        }
    }

    pub fn with_global_sender(mut self, sender: UnboundedSender<Events>) -> Self {
        self._global_event_tx = Some(sender);
        self
    }

    fn next_page(&mut self) {
        self.page_list_state.next()
    }

    fn previous_page(&mut self) {
        self.page_list_state.previous();
    }

    fn render_page_list(&mut self, area: Rect, buf: &mut Buffer) {
        let inner_area = area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });
        StatefulWidget::render(self.pages_list.clone(), inner_area, buf, &mut self.page_list_state);
    }

    fn load_page(&mut self, maybe_data: Option<PageData>) {
        if let Some(data) = maybe_data {
            match self.pages.get_mut(data.index) {
                Some(page) => {
                    let protocol = self.picker.new_resize_protocol(data.panel.image_decoded);
                    page.image_state = Some(protocol);
                    page.dimensions = Some(data.panel.dimensions);
                },
                None => {
                    // Todo! indicate that the page couldnot be loaded
                },
            };
            match self.pages_list.pages.get_mut(data.index) {
                Some(page_item) => page_item.state = PageItemState::FinishedLoad,
                None => {
                    // Todo! indicate with an x that some page didnt load
                },
            }
        }
    }

    fn load_chapter(&mut self, chapter: Chapter) {
        self.clean_up();
        self.chapter = chapter;
        self.state = State::SearchingPages;
        self.init_fetching_pages();
    }

    fn fecht_pages(&mut self) {
        for (index, url) in self.chapter.pages_url.iter_mut().enumerate() {
            self.pages.push(Page::new());
            self.pages_list.pages.push(PagesItem::new(index));
            let tx = self.local_event_tx.clone();
            let api_client = self.api_client.clone();
            self.image_tasks.spawn(get_manga_panel(api_client, url.clone(), tx, index));
        }
    }

    fn set_chapter_not_found(&mut self) {
        self.state = State::DisplayingChapterNotFound;
    }

    fn set_error_searching_chapter(&mut self) {
        self.state = State::ErrorSearchingChapter;
    }

    fn render_instructions(&mut self, buf: &mut Buffer, area: Rect) {
        match self.state {
            State::DisplayingChapterNotFound => "There is no more chapter".render(area, buf),
            State::ErrorSearchingChapter => "error searching chapter".render(area, buf),
            State::SearchingChapter => "searching chapter".render(area, buf),
            _ => {},
        };
    }

    fn tick(&mut self) {
        self.pages_list.on_tick();
        if let Ok(background_event) = self.local_event_rx.try_recv() {
            match background_event {
                MangaReaderEvents::ErrorSearchingChapter => self.set_error_searching_chapter(),
                MangaReaderEvents::ChapterNotFound => self.set_chapter_not_found(),
                MangaReaderEvents::LoadChapter(chapter_found) => self.load_chapter(chapter_found),
                MangaReaderEvents::SearchNextChapter => self.search_chapter(self.api_client.clone()),
                MangaReaderEvents::FetchPages => self.fecht_pages(),
                MangaReaderEvents::LoadPage(maybe_data) => self.load_page(maybe_data),
            }
        }
    }

    fn handle_key_events(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.local_action_tx.send(MangaReaderActions::NextPage).ok();
            },
            KeyCode::Up | KeyCode::Char('k') => {
                self.local_action_tx.send(MangaReaderActions::PreviousPage).ok();
            },
            KeyCode::Char('w') => {
                self.local_action_tx.send(MangaReaderActions::SearchNextChapter).ok();
            },
            _ => {},
        }
    }

    pub fn init_fetching_pages(&self) {
        self.local_event_tx.send(MangaReaderEvents::FetchPages).ok();
    }

    fn set_searching_chapter(&mut self) {
        self.state = State::SearchingChapter;
    }

    fn initiate_search_next_chapter(&mut self) {
        self.set_searching_chapter();
        self.chapter.number += 1;
        self.local_event_tx.send(MangaReaderEvents::SearchNextChapter).ok();
    }

    fn search_chapter<C: SearchChapter>(&mut self, api_client: C) {
        let manga_id = self.manga_id.clone();
        let chapter_number = self.chapter.number.to_string();
        let language = self.chapter.language;
        let volume_number = self.chapter.volume_number.unwrap_or_default();
        let sender = self.local_event_tx.clone();
        self.image_tasks.spawn(async move {
            let response = api_client
                .search_chapter(&manga_id, &volume_number.to_string(), &chapter_number, language)
                .await;
            match response {
                Ok(res) => match res {
                    Some(chapter_found) => {
                        sender.send(MangaReaderEvents::LoadChapter(chapter_found)).ok();
                    },
                    None => {
                        sender.send(MangaReaderEvents::ChapterNotFound).ok();
                    },
                },
                Err(_e) => {
                    sender.send(MangaReaderEvents::ErrorSearchingChapter).ok();
                },
            };
        });
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use pretty_assertions::assert_eq;
    use tokio::time::timeout;

    use super::*;
    use crate::view::widgets::press_key;

    #[derive(Clone)]
    struct TestApiClient {
        should_fail: bool,
        response: Option<Chapter>,
        panel_response: MangaPanel,
    }

    impl TestApiClient {
        pub fn new() -> Self {
            Self {
                should_fail: false,
                response: None,
                panel_response: MangaPanel::default(),
            }
        }

        pub fn with_response(response: Chapter) -> Self {
            Self {
                should_fail: false,
                response: Some(response),
                panel_response: MangaPanel::default(),
            }
        }

        pub fn with_not_found() -> Self {
            Self {
                should_fail: false,
                response: None,
                panel_response: MangaPanel::default(),
            }
        }

        pub fn with_failing_request() -> Self {
            Self {
                should_fail: true,
                response: None,
                panel_response: MangaPanel::default(),
            }
        }

        pub fn with_page_response(response: MangaPanel) -> Self {
            Self {
                should_fail: true,
                response: None,
                panel_response: response,
            }
        }
    }

    impl SearchChapter for TestApiClient {
        async fn search_chapter(
            &self,
            _volume_number: &str,
            _chapter_id: &str,
            _chapter_number: &str,
            _language: Languages,
        ) -> Result<Option<Chapter>, Box<dyn Error>> {
            if self.should_fail { Err("should_fail".into()) } else { Ok(self.response.clone()) }
        }
    }

    impl SearchMangaPanel for TestApiClient {
        async fn search_manga_panel(&self, _endpoint: Url) -> Result<MangaPanel, Box<dyn Error>> {
            if self.should_fail { Err("must_failt".into()) } else { Ok(self.panel_response.clone()) }
        }
    }

    fn initialize_reader_page<T: SearchChapter + SearchMangaPanel>(api_client: T) -> MangaReader<T> {
        let picker = Picker::new((8, 19));
        let chapter_id = "some_id".to_string();
        let base_url = "some_base_url".to_string();
        let url_imgs = vec!["http://localhost".parse().unwrap(), "http://localhost".parse().unwrap()];
        MangaReader::new(
            Chapter {
                id: chapter_id,
                number: 1,
                pages_url: url_imgs,
                volume_number: Some(2),
                language: Languages::default(),
            },
            "some_manga_id".to_string(),
            picker,
            api_client,
        )
    }

    #[tokio::test]
    async fn trigget_key_events() {
        let mut reader_page = initialize_reader_page(TestApiClient::new());

        press_key(&mut reader_page, KeyCode::Char('j'));
        let action = reader_page.local_action_rx.recv().await.unwrap();

        assert_eq!(MangaReaderActions::NextPage, action);

        press_key(&mut reader_page, KeyCode::Char('k'));
        let action = reader_page.local_action_rx.recv().await.unwrap();

        assert_eq!(MangaReaderActions::PreviousPage, action);
    }

    #[test]
    fn handle_key_events() {
        let mut reader_page = initialize_reader_page(TestApiClient::new());

        reader_page.pages_list = PagesList::new(vec![PagesItem::new(0), PagesItem::new(1), PagesItem::new(2)]);

        let area = Rect::new(0, 0, 20, 20);
        let mut buf = Buffer::empty(area);

        reader_page.render_page_list(area, &mut buf);

        let action = MangaReaderActions::NextPage;
        reader_page.update(action);

        assert_eq!(0, reader_page.page_list_state.selected.expect("no page is selected"));

        let action = MangaReaderActions::NextPage;
        reader_page.update(action);

        assert_eq!(1, reader_page.page_list_state.selected.expect("no page is selected"));

        let action = MangaReaderActions::PreviousPage;
        reader_page.update(action);

        assert_eq!(0, reader_page.page_list_state.selected.expect("no page is selected"));
    }

    #[tokio::test]
    async fn it_collects_pages() {
        let chapter: Chapter = Chapter {
            pages_url: vec!["http://localhost".parse().unwrap(), "http://localhost".parse().unwrap()],
            ..Default::default()
        };

        let mut manga_reader = MangaReader::new(chapter, "some_id".to_string(), Picker::new((8, 8)), TestApiClient::new());

        manga_reader.fecht_pages();

        assert!(!manga_reader.pages.is_empty());
        assert!(!manga_reader.pages_list.pages.is_empty());
    }

    #[tokio::test]
    async fn it_initiates_search_next_chapter_event() {
        let mut manga_reader =
            MangaReader::new(Chapter::default(), "some_id".to_string(), Picker::new((8, 8)), TestApiClient::new());

        manga_reader.initiate_search_next_chapter();

        let expected_event = tokio::time::timeout(Duration::from_millis(250), manga_reader.local_event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(expected_event, MangaReaderEvents::SearchNextChapter);
        assert_eq!(manga_reader.state, State::SearchingChapter);
        assert_eq!(manga_reader.chapter.number, 2);
    }

    #[tokio::test]
    async fn it_initiates_search_next_chapter_on_key_event() {
        let mut manga_reader =
            MangaReader::new(Chapter::default(), "some_id".to_string(), Picker::new((8, 8)), TestApiClient::new());

        manga_reader.update(MangaReaderActions::SearchNextChapter);

        assert_eq!(manga_reader.state, State::SearchingChapter);
    }

    #[tokio::test]
    async fn it_sends_search_next_chapter_action_on_w_key_press() {
        let mut manga_reader =
            MangaReader::new(Chapter::default(), "some_id".to_string(), Picker::new((8, 8)), TestApiClient::new());

        press_key(&mut manga_reader, KeyCode::Char('w'));

        let expected_event = timeout(Duration::from_millis(250), manga_reader.local_action_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(expected_event, MangaReaderActions::SearchNextChapter);
    }
    #[tokio::test]
    async fn it_searches_chapter_and_sends_successful_result() {
        let expected = Chapter {
            id: "next_chapter_id".to_string(),
            number: 2,
            volume_number: Some(1),
            pages_url: vec!["http://localhost".parse().unwrap()],
            language: Languages::default(),
        };

        let api_client = TestApiClient::with_response(expected.clone());

        let mut manga_reader = MangaReader::new(Chapter::default(), "some_id".to_string(), Picker::new((8, 8)), api_client);

        manga_reader.search_chapter(manga_reader.api_client.clone());

        let result = timeout(Duration::from_millis(250), manga_reader.local_event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(result, MangaReaderEvents::LoadChapter(expected));
    }

    #[tokio::test]
    async fn it_searches_chapter_and_sends_chapter_not_found_event() {
        let api_client = TestApiClient::with_not_found();
        let mut manga_reader = MangaReader::new(Chapter::default(), "some_id".to_string(), Picker::new((8, 8)), api_client);

        manga_reader.search_chapter(manga_reader.api_client.clone());

        let result = timeout(Duration::from_millis(250), manga_reader.local_event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(result, MangaReaderEvents::ChapterNotFound);
    }

    #[tokio::test]
    async fn it_searches_chapter_and_sends_error_event() {
        let api_client = TestApiClient::with_failing_request();
        let mut manga_reader = MangaReader::new(Chapter::default(), "some_id".to_string(), Picker::new((8, 8)), api_client);

        manga_reader.search_chapter(manga_reader.api_client.clone());

        let result = timeout(Duration::from_millis(250), manga_reader.local_event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(result, MangaReaderEvents::ErrorSearchingChapter);
    }

    #[test]
    fn it_loads_chapter_found_and_sets_state_as_searching_pages() {
        let expected = Chapter {
            id: "id_before".to_string(),
            number: 1,
            volume_number: Some(1),
            pages_url: vec![],
            language: Languages::default(),
        };

        let api_client = TestApiClient::with_response(expected.clone());

        let mut manga_reader = MangaReader::new(Chapter::default(), "some_id".to_string(), Picker::new((8, 8)), api_client);

        manga_reader.state = State::SearchingChapter;

        manga_reader.load_chapter(expected.clone());

        assert_eq!(expected, manga_reader.chapter);
        assert_eq!(manga_reader.state, State::SearchingPages);
    }

    #[test]
    fn it_resets_pages_after_chapter_was_found() {
        let api_client = TestApiClient::new();

        let mut manga_reader = MangaReader::new(Chapter::default(), "some_id".to_string(), Picker::new((8, 8)), api_client);

        manga_reader.pages = vec![Page::new(), Page::new()];
        manga_reader.pages_list.pages = vec![PagesItem::new(1), PagesItem::new(1)];
        manga_reader.page_list_state.select(Some(1));

        manga_reader.load_chapter(Chapter::default());
        assert!(manga_reader.pages.is_empty());
        assert!(manga_reader.pages_list.pages.is_empty());
        assert!(manga_reader.page_list_state.selected.is_none());
    }

    #[tokio::test]
    async fn it_send_event_to_search_pages_after_chapter_was_loaded() {
        let api_client = TestApiClient::new();
        let mut manga_reader = MangaReader::new(Chapter::default(), "some_id".to_string(), Picker::new((8, 8)), api_client);

        manga_reader.load_chapter(Chapter::default());

        let expected = MangaReaderEvents::FetchPages;

        let result = timeout(Duration::from_millis(250), manga_reader.local_event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(expected, result)
    }

    #[test]
    fn it_loads_chapter_on_event() {
        let chapter_to_load = Chapter {
            id: "new_chapter_id".to_string(),
            ..Default::default()
        };

        let api_client = TestApiClient::new();

        let mut manga_reader = MangaReader::new(Chapter::default(), "some_id".to_string(), Picker::new((8, 8)), api_client);

        manga_reader
            .local_event_tx
            .send(MangaReaderEvents::LoadChapter(chapter_to_load.clone()))
            .ok();

        manga_reader.tick();

        assert_eq!(manga_reader.chapter.id, chapter_to_load.id);
    }

    #[test]
    fn it_is_set_as_error_searching_chapter_on_event() {
        let api_client = TestApiClient::new();

        let mut manga_reader = MangaReader::new(Chapter::default(), "some_id".to_string(), Picker::new((8, 8)), api_client);

        manga_reader.local_event_tx.send(MangaReaderEvents::ErrorSearchingChapter).ok();

        manga_reader.tick();

        assert_eq!(manga_reader.state, State::ErrorSearchingChapter);
    }
}
