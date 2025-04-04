use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::text::{Line, ToSpan};
use ratatui::widgets::{Block, List, Paragraph, StatefulWidget, Widget, Wrap};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};
use throbber_widgets_tui::{Throbber, ThrobberState};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;

use crate::backend::database::{Bookmark, ChapterToBookmark, ChapterToSaveHistory, Database, MangaReadingHistorySave};
use crate::backend::error_log::{ErrorType, write_to_error_log};
use crate::backend::manga_provider::{ChapterReader, ChapterToRead, ListOfChapters, MangaPanel, ReaderPageProvider};
use crate::backend::tracker::{MangaTracker, track_manga};
use crate::backend::tui::Events;
use crate::common::format_error_message_tracking_reading_history;
use crate::config::MangaTuiConfig;
use crate::global::{ERROR_STYLE, INSTRUCTIONS_STYLE};
use crate::view::widgets::Component;
use crate::view::widgets::reader::{PageItemState, PagesItem, PagesList, PagesListState};

#[derive(Debug, PartialEq, Clone, Default)]
pub enum PageSize {
    #[default]
    Normal,
    Wide,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MangaReaderActions {
    BookMarkCurrentChapter,
    SearchNextChapter,
    SearchPreviousChapter,
    NextPage,
    PreviousPage,
    ReloadPage,
    ExitReaderPage,
}

#[derive(Debug, PartialEq, Eq, Default)]
pub enum State {
    ManualBookmark,
    ErrorSearchingChapter,
    DisplayingChapterNotFound,
    SearchingChapter,
    #[default]
    SearchingPages,
}

#[derive(Debug, PartialEq, Clone)]
pub struct PageData {
    pub panel: MangaPanel,
    pub index: usize,
}

#[derive(Debug, PartialEq)]
pub enum MangaReaderEvents {
    SaveReadingToDatabase,
    ErrorSearchingChapter,
    ChapterNotFound,
    LoadChapter(ChapterToRead),
    SearchNextChapter(String),
    SearchPreviousChapter(String),
    FetchPages,
    LoadPage(PageData),
    FailedPage(usize),
    ErrorTrackingReadingProgress(String),
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

pub struct MangaReader<T, S>
where
    T: ReaderPageProvider,
    S: MangaTracker,
{
    manga_title: String,
    manga_id: String,
    pub list_of_chapters: ListOfChapters,
    current_chapter: ChapterToRead,
    pages: Vec<Page>,
    pages_list: PagesList,
    current_page_size: PageSize,
    page_list_state: PagesListState,
    state: State,
    image_tasks: JoinSet<()>,
    picker: Picker,
    search_next_chapter_loader: ThrobberState,
    api_client: Arc<T>,
    pub manga_tracker: Option<S>,
    pub auto_bookmark: bool,
    pub global_event_tx: Option<UnboundedSender<Events>>,
    pub local_action_tx: UnboundedSender<MangaReaderActions>,
    pub local_action_rx: UnboundedReceiver<MangaReaderActions>,
    pub local_event_tx: UnboundedSender<MangaReaderEvents>,
    pub local_event_rx: UnboundedReceiver<MangaReaderEvents>,
}

impl<T, S> Component for MangaReader<T, S>
where
    T: ReaderPageProvider,
    S: MangaTracker,
{
    type Actions = MangaReaderActions;

    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) {
        let buf = frame.buffer_mut();

        let layout = match self.current_page_size {
            PageSize::Normal => [Constraint::Percentage(30), Constraint::Percentage(40), Constraint::Percentage(30)],
            PageSize::Wide => [Constraint::Percentage(20), Constraint::Percentage(60), Constraint::Percentage(20)],
        };

        let [left, center, right] = Layout::horizontal(layout).areas(area);

        Block::bordered().render(left, buf);

        let index = self.current_page_index();
        let show_reload = if let Some(page) = self.pages.get_mut(index).filter(|page| page.image_state.is_some()) {
            let image = StatefulImage::new(None).resize(Resize::Fit(None));
            StatefulWidget::render(image, center, buf, page.image_state.as_mut().unwrap());
            let (width, height) = page.dimensions.unwrap();
            self.resize_based_on_image_size(width, height);

            false
        } else {
            let show_failed = self
                .pages_list
                .pages
                .get(index)
                .map(|page| page.state == PageItemState::FailedLoad)
                .unwrap_or(false);

            if show_failed {
                Block::bordered().title("Failed to load page").render(center, buf);
            } else {
                Block::bordered().title("Loading page").render(center, buf);
            }

            show_failed
        };

        self.render_page_list(left, buf);
        self.render_right_panel(buf, right, show_reload);
    }

    fn update(&mut self, action: Self::Actions) {
        match action {
            MangaReaderActions::ExitReaderPage => self.exit(),
            MangaReaderActions::BookMarkCurrentChapter => self.bookmark_current_chapter(),
            MangaReaderActions::SearchPreviousChapter => self.initiate_search_previous_chapter(),
            MangaReaderActions::SearchNextChapter => self.initiate_search_next_chapter(),
            MangaReaderActions::NextPage => self.next_page(),
            MangaReaderActions::PreviousPage => self.previous_page(),
            MangaReaderActions::ReloadPage => self.reload_page(),
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
        self.page_list_state = PagesListState::default();
    }
}

impl<T, S> MangaReader<T, S>
where
    T: ReaderPageProvider,
    S: MangaTracker,
{
    pub fn new(chapter: ChapterToRead, manga_id: String, picker: Picker, api_client: Arc<T>) -> Self {
        let set: JoinSet<()> = JoinSet::new();
        let (local_action_tx, local_action_rx) = mpsc::unbounded_channel::<MangaReaderActions>();
        let (local_event_tx, local_event_rx) = mpsc::unbounded_channel::<MangaReaderEvents>();

        let num_page_bookmarked = chapter.num_page_bookmarked;

        Self {
            global_event_tx: None,
            auto_bookmark: false,
            current_chapter: chapter,
            manga_title: String::default(),
            pages: vec![],
            manga_id,
            list_of_chapters: ListOfChapters::default(),
            page_list_state: PagesListState::new(num_page_bookmarked.map(|num| num as usize)),
            image_tasks: set,
            local_action_tx,
            local_action_rx,
            local_event_tx,
            local_event_rx,
            state: State::default(),
            manga_tracker: None,
            current_page_size: PageSize::default(),
            pages_list: PagesList::default(),
            search_next_chapter_loader: ThrobberState::default(),
            picker,
            api_client,
        }
    }

    pub fn with_global_sender(mut self, sender: UnboundedSender<Events>) -> Self {
        self.global_event_tx = Some(sender);
        self
    }

    pub fn set_auto_bookmark(&mut self) {
        self.auto_bookmark = true;
    }

    pub fn with_list_of_chapters(mut self, list: ListOfChapters) -> Self {
        self.list_of_chapters = list;
        self
    }

    pub fn with_manga_title(mut self, title: String) -> Self {
        self.manga_title = title;
        self
    }

    pub fn with_manga_tracker(mut self, manga_tracker: Option<S>) -> Self {
        self.manga_tracker = manga_tracker;
        self
    }

    fn next_page(&mut self) {
        self.page_list_state.list_state.next();
        self.fetch_pages();
    }

    fn previous_page(&mut self) {
        self.page_list_state.list_state.previous();
        self.fetch_pages();
    }

    fn reload_page(&mut self) {
        self.fetch_page(self.current_page_index());
    }

    fn render_page_list(&mut self, area: Rect, buf: &mut Buffer) {
        let inner_area = area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });
        StatefulWidget::render(self.pages_list.clone(), inner_area, buf, &mut self.page_list_state);
    }

    fn load_page(&mut self, data: PageData) {
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

    fn resize_based_on_image_size(&mut self, width: u32, height: u32) {
        if width > height && width > 300 {
            self.current_page_size = PageSize::Wide;
        } else {
            self.current_page_size = PageSize::Normal;
        }
    }

    fn load_chapter(&mut self, chapter: ChapterToRead) {
        self.clean_up();

        self.current_chapter = chapter;
        self.state = State::SearchingPages;

        self.init_fetching_pages();
        self.init_save_reading_history();
        self.track_manga_reading_history(self.manga_tracker.clone());
    }

    fn init_save_reading_history(&self) {
        self.local_event_tx.send(MangaReaderEvents::SaveReadingToDatabase).ok();
    }

    fn current_page_index(&self) -> usize {
        self.page_list_state.list_state.selected.unwrap_or(0)
    }

    fn failed_page(&mut self, index: usize) {
        match self.pages_list.pages.get_mut(index) {
            Some(page_item) => page_item.state = PageItemState::FailedLoad,
            None => {
                // Todo! indicate that the page does not exist?
            },
        }
    }

    fn get_pages_to_fetch(&self) -> Vec<usize> {
        let pages = MangaTuiConfig::get().amount_pages as usize;

        if self.pages.len() == 1 {
            return vec![0];
        }

        // Collect `pages` pages before and after index that are not yet loaded
        let curr = self.current_page_index();
        let start_index = curr.saturating_sub(pages);
        let end_index = curr.saturating_add(pages).min(self.pages.len().saturating_sub(1));

        if end_index > 0 {
            self.pages[start_index..=end_index]
                .iter()
                .enumerate()
                .filter_map(|(base_index, page)| match page.image_state {
                    Some(_) => None,
                    None => Some(base_index + start_index),
                })
                .collect()
        } else {
            vec![]
        }
    }

    fn fetch_page(&mut self, index: usize) {
        if let Some((url, item)) = self
            .current_chapter
            .pages_url
            .get(index)
            .and_then(|page| self.pages_list.pages.get_mut(index).map(|item| (page, item)))
        {
            //NOTE:  This will need to become async atomic if this becomes an async function
            if item.state != PageItemState::Loading && item.state != PageItemState::FinishedLoad {
                let tx = self.local_event_tx.clone();
                let api_client = Arc::clone(&self.api_client);
                let url = url.clone();

                self.image_tasks.spawn(async move {
                    let response = api_client.search_manga_panel(url).await;

                    match response {
                        Ok(panel) => {
                            let page = PageData { panel, index };
                            tx.send(MangaReaderEvents::LoadPage(page)).ok();
                        },
                        Err(e) => {
                            tx.send(MangaReaderEvents::FailedPage(index)).ok();
                            write_to_error_log(ErrorType::Error(e));
                        },
                    }
                });

                item.state = PageItemState::Loading;
            }
        }
    }

    fn fetch_pages(&mut self) {
        for index in self.get_pages_to_fetch() {
            self.fetch_page(index);
        }
    }

    fn set_chapter_not_found(&mut self) {
        self.state = State::DisplayingChapterNotFound;
    }

    fn set_error_searching_chapter(&mut self) {
        self.state = State::ErrorSearchingChapter;
    }

    fn set_current_chapter_bookmarked(&mut self, num_page: Option<u32>, database: &mut dyn Bookmark) {
        let chapter_to_bookmark = ChapterToBookmark {
            chapter_id: &self.current_chapter.id,
            manga_id: &self.manga_id,
            chapter_title: &self.current_chapter.title,
            manga_title: &self.manga_title,
            manga_cover_url: None,
            translated_language: self.current_chapter.language,
            page_number: num_page,
            provider: self.api_client.name(),
        };

        match database.bookmark(chapter_to_bookmark) {
            Ok(()) => {
                let page_index = num_page.unwrap_or(0) as usize;
                self.state = State::ManualBookmark;
                self.pages_list.highlight_page_as_bookmarked(page_index);
                self.page_list_state.set_page_bookmarked(page_index);
            },
            Err(e) => {
                write_to_error_log(ErrorType::String(format!("Could not mark chapter as bookmarked: more details : {e}").as_str()))
            },
        }
    }

    pub fn bookmark_current_chapter(&mut self) {
        let connection = Database::get_connection();
        if let Ok(conn) = connection {
            let mut database = Database::new(&conn);
            self.set_current_chapter_bookmarked(self.page_list_state.list_state.selected.map(|index| index as u32), &mut database);
        }
    }

    fn track_manga_reading_history(&self, manga_tracker: Option<S>) {
        let chapter_to_track = self.current_chapter.clone();
        let tx = self.local_event_tx.clone();

        track_manga(
            manga_tracker,
            self.manga_title.clone(),
            chapter_to_track.number as u32,
            chapter_to_track.volume_number.clone().unwrap_or("0".to_string()).parse().ok(),
            move |error| {
                tx.send(MangaReaderEvents::ErrorTrackingReadingProgress(error)).ok();
            },
        );
    }

    pub fn exit(&mut self) {
        if self.auto_bookmark {
            self.bookmark_current_chapter()
        }
        self.global_event_tx.as_ref().unwrap().send(Events::GoBackMangaPage).ok();
    }

    fn render_right_panel(&mut self, buf: &mut Buffer, area: Rect, show_reload: bool) {
        let [instructions_area, information_era, status_area] =
            Layout::vertical([Constraint::Percentage(20), Constraint::Percentage(20), Constraint::Percentage(20)])
                .margin(2)
                .areas(area);

        let mut instructions = vec![
            Line::from(vec!["Go back: ".into(), "<Backspace>".to_span().style(*INSTRUCTIONS_STYLE)]),
            Line::from(vec!["Next chapter: ".into(), "<w>".to_span().style(*INSTRUCTIONS_STYLE)]),
            Line::from(vec!["Previous chapter: ".into(), "<b>".to_span().style(*INSTRUCTIONS_STYLE)]),
        ];

        if show_reload {
            instructions.push(Line::from(vec!["Reload: ".into(), "<r>".to_span().style(*INSTRUCTIONS_STYLE)]));
        }

        if !self.auto_bookmark {
            instructions.push(Line::from(vec!["Bookmark: ".into(), "<m>".to_span().style(*INSTRUCTIONS_STYLE)]));
        }

        Widget::render(List::new(instructions).block(Block::bordered()), instructions_area, buf);

        let current_chapter_title = format!(
            "Reading : Vol {} Ch. {} {}",
            self.current_chapter.volume_number.as_ref().cloned().unwrap_or("none".to_string()),
            self.current_chapter.number,
            self.current_chapter.title
        );

        Paragraph::new(current_chapter_title)
            .wrap(Wrap { trim: true })
            .render(information_era, buf);

        match self.state {
            State::DisplayingChapterNotFound => Paragraph::new("There is no more chapters")
                .wrap(Wrap { trim: true })
                .render(status_area, buf),
            State::ErrorSearchingChapter => {
                Paragraph::new("error searching chapter, please try again".to_span().style(*ERROR_STYLE))
                    .wrap(Wrap { trim: true })
                    .render(status_area, buf)
            },
            State::SearchingChapter => {
                let loader = Throbber::default()
                    .label("Searching chapter".to_span().style(*INSTRUCTIONS_STYLE))
                    .throbber_set(throbber_widgets_tui::BRAILLE_SIX)
                    .use_type(throbber_widgets_tui::WhichUse::Spin);

                StatefulWidget::render(loader, status_area, buf, &mut self.search_next_chapter_loader)
            },
            State::ManualBookmark => {
                let message = format!("Bookmarked at page: {}", self.page_list_state.page_bookmarked.unwrap_or(0));

                Paragraph::new(message.to_span().style(*INSTRUCTIONS_STYLE))
                    .wrap(Wrap { trim: true })
                    .render(status_area, buf)
            },
            _ => {},
        };
    }

    fn tick(&mut self) {
        self.pages_list.on_tick();
        if self.state == State::SearchingChapter {
            self.search_next_chapter_loader.calc_next();
        }

        while let Ok(background_event) = self.local_event_rx.try_recv() {
            match background_event {
                MangaReaderEvents::SaveReadingToDatabase => {
                    self.save_reading_history();
                },
                MangaReaderEvents::SearchPreviousChapter(id_chapter) => self.search_chapter(id_chapter),
                MangaReaderEvents::ErrorSearchingChapter => self.set_error_searching_chapter(),
                MangaReaderEvents::ChapterNotFound => self.set_chapter_not_found(),
                MangaReaderEvents::LoadChapter(chapter_found) => self.load_chapter(chapter_found),
                MangaReaderEvents::SearchNextChapter(id_chapter) => self.search_chapter(id_chapter),
                MangaReaderEvents::FetchPages => self.fetch_pages(),
                MangaReaderEvents::LoadPage(maybe_data) => self.load_page(maybe_data),
                MangaReaderEvents::FailedPage(index) => self.failed_page(index),
                MangaReaderEvents::ErrorTrackingReadingProgress(error_message) => self.log_manga_tracking_error(error_message),
            }
        }
    }

    fn log_manga_tracking_error(&self, error_message: String) {
        write_to_error_log(
            format_error_message_tracking_reading_history(self.current_chapter.clone(), self.manga_title.clone(), error_message)
                .into(),
        );
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
            KeyCode::Char('b') => {
                self.local_action_tx.send(MangaReaderActions::SearchPreviousChapter).ok();
            },
            KeyCode::Char('r') => {
                self.local_action_tx.send(MangaReaderActions::ReloadPage).ok();
            },
            KeyCode::Char('m') => {
                if !self.auto_bookmark {
                    self.local_action_tx.send(MangaReaderActions::BookMarkCurrentChapter).ok();
                }
            },
            KeyCode::Backspace => {
                self.local_action_tx.send(MangaReaderActions::ExitReaderPage).ok();
            },
            _ => {},
        }
    }

    pub fn init_fetching_pages(&mut self) {
        let page_count = self.current_chapter.pages_url.len();
        for index in 0..page_count {
            self.pages.push(Page::new());
            self.pages_list.pages.push(PagesItem::new(index));
        }

        self.local_event_tx.send(MangaReaderEvents::FetchPages).ok();
    }

    fn set_searching_chapter(&mut self) {
        self.state = State::SearchingChapter;
    }

    fn initiate_search_next_chapter(&mut self) {
        match self.get_next_chapter_in_the_list() {
            Some(next_chapter) => {
                self.set_searching_chapter();
                self.local_event_tx.send(MangaReaderEvents::SearchNextChapter(next_chapter.id)).ok();
            },
            None => {
                self.set_chapter_not_found();
            },
        }
    }

    fn initiate_search_previous_chapter(&mut self) {
        match self.get_previous_chapter_in_the_list() {
            Some(chapter) => {
                self.set_searching_chapter();
                self.local_event_tx.send(MangaReaderEvents::SearchPreviousChapter(chapter.id)).ok();
            },
            None => self.set_chapter_not_found(),
        }
    }

    fn get_next_chapter_in_the_list(&self) -> Option<ChapterReader> {
        self.list_of_chapters
            .get_next_chapter(self.current_chapter.volume_number.as_deref(), self.current_chapter.number)
    }

    fn get_previous_chapter_in_the_list(&self) -> Option<ChapterReader> {
        self.list_of_chapters
            .get_previous_chapter(self.current_chapter.volume_number.as_deref(), self.current_chapter.number)
    }

    fn search_chapter(&mut self, chapter_id: String) {
        let api_client = self.api_client.clone();
        let sender = self.local_event_tx.clone();
        let manga_id = self.manga_id.clone();
        self.image_tasks.spawn(async move {
            let response = api_client.search_chapter(&chapter_id, &manga_id).await;
            match response {
                Ok(res) => {
                    sender.send(MangaReaderEvents::LoadChapter(res)).ok();
                },
                Err(e) => {
                    write_to_error_log(ErrorType::Error(e));
                    sender.send(MangaReaderEvents::ErrorSearchingChapter).ok();
                },
            };
        });
    }

    fn save_reading_history(&self) -> String {
        let connection = Database::get_connection();
        if let Ok(conn) = connection {
            let database = Database::new(&conn);
            database
                .save_history(MangaReadingHistorySave {
                    id: &self.manga_id,
                    title: &self.manga_title,
                    img_url: None,
                    chapter: ChapterToSaveHistory {
                        id: &self.current_chapter.id,
                        title: &self.current_chapter.title,
                        translated_language: "en",
                    },
                    provider: self.api_client.name(),
                })
                .unwrap();
            return self.current_chapter.id.clone();
        }
        "".to_string()
    }
}

#[cfg(test)]
mod test {
    use std::error::Error;
    use std::time::Duration;

    use pretty_assertions::assert_eq;
    use tokio::time::timeout;

    use self::mpsc::unbounded_channel;
    use super::*;
    use crate::backend::database::ChapterToBookmark;
    use crate::backend::manga_provider::mock::ReaderPageProvierMock;
    use crate::backend::manga_provider::{Languages, SortedChapters, SortedVolumes, Volumes};
    use crate::global::test_utils::TrackerTest;
    use crate::view::widgets::press_key;

    fn initialize_reader_page<T, S>(api_client: Arc<T>) -> MangaReader<T, S>
    where
        T: ReaderPageProvider,
        S: MangaTracker,
    {
        let picker = Picker::new((8, 19));
        let chapter_id = "some_id".to_string();
        let url_imgs = vec!["http://localhost".parse().unwrap(), "http://localhost".parse().unwrap()];
        MangaReader::new(
            ChapterToRead {
                id: chapter_id,
                title: String::default(),
                number: 1.0,
                pages_url: url_imgs,
                language: Languages::default(),
                num_page_bookmarked: None,
                volume_number: Some("2".to_string()),
            },
            "some_manga_id".to_string(),
            picker,
            api_client,
        )
    }

    #[tokio::test]
    async fn trigget_key_events() {
        let mut reader_page: MangaReader<ReaderPageProvierMock, TrackerTest> =
            initialize_reader_page(ReaderPageProvierMock::new().into());

        press_key(&mut reader_page, KeyCode::Char('j'));
        let action = reader_page.local_action_rx.recv().await.unwrap();

        assert_eq!(MangaReaderActions::NextPage, action);

        press_key(&mut reader_page, KeyCode::Char('k'));
        let action = reader_page.local_action_rx.recv().await.unwrap();

        assert_eq!(MangaReaderActions::PreviousPage, action);
    }

    #[tokio::test]
    async fn handle_key_events() {
        let mut reader_page: MangaReader<ReaderPageProvierMock, TrackerTest> =
            initialize_reader_page(ReaderPageProvierMock::new().into());

        reader_page.pages_list = PagesList::new(vec![PagesItem::new(0), PagesItem::new(1), PagesItem::new(2)]);

        let area = Rect::new(0, 0, 20, 20);
        let mut buf = Buffer::empty(area);

        reader_page.render_page_list(area, &mut buf);

        let action = MangaReaderActions::NextPage;
        reader_page.update(action);

        assert_eq!(0, reader_page.page_list_state.list_state.selected.expect("no page is selected"));

        let action = MangaReaderActions::NextPage;
        reader_page.update(action);

        assert_eq!(1, reader_page.page_list_state.list_state.selected.expect("no page is selected"));

        let action = MangaReaderActions::PreviousPage;
        reader_page.update(action);

        assert_eq!(0, reader_page.page_list_state.list_state.selected.expect("no page is selected"));
    }

    #[tokio::test]
    async fn init_fetching_and_fetch_pages_should_set_correct_page_count_and_first_page_state_to_loading() {
        let chapter: ChapterToRead = ChapterToRead {
            pages_url: vec!["http://localhost".parse().unwrap(), "http://localhost".parse().unwrap()],
            ..Default::default()
        };

        let mut manga_reader: MangaReader<ReaderPageProvierMock, TrackerTest> =
            MangaReader::new(chapter, "some_id".to_string(), Picker::new((8, 8)), ReaderPageProvierMock::new().into());

        manga_reader.init_fetching_pages();
        manga_reader.fetch_pages();

        assert_eq!(2, manga_reader.pages.len());
        assert_eq!(2, manga_reader.pages_list.pages.len());

        assert_eq!(PageItemState::Loading, manga_reader.pages_list.pages[0].state);
    }

    #[test]
    fn it_increases_page_size_based_on_manga_panel_dimesions() {
        let mut manga_reader: MangaReader<ReaderPageProvierMock, TrackerTest> = MangaReader::new(
            ChapterToRead::default(),
            "some_id".to_string(),
            Picker::new((8, 8)),
            ReaderPageProvierMock::new().into(),
        );

        manga_reader.resize_based_on_image_size(500, 200);

        assert_eq!(PageSize::Wide, manga_reader.current_page_size);

        manga_reader.resize_based_on_image_size(100, 200);

        assert_eq!(PageSize::Normal, manga_reader.current_page_size);

        // only resize if the image is big enough
        manga_reader.resize_based_on_image_size(300, 200);

        assert_eq!(PageSize::Normal, manga_reader.current_page_size);
    }

    #[test]
    fn it_does_not_initiate_search_next_chapter_if_there_is_no_next_chapter() {
        let list_of_chapters = ListOfChapters::default();
        let mut manga_reader: MangaReader<ReaderPageProvierMock, TrackerTest> =
            MangaReader::new(ChapterToRead::default(), "".to_string(), Picker::new((8, 8)), ReaderPageProvierMock::new().into())
                .with_list_of_chapters(list_of_chapters);

        manga_reader.initiate_search_next_chapter();

        assert_eq!(manga_reader.state, State::DisplayingChapterNotFound);
    }

    #[test]
    fn it_does_not_initiate_search_previous_chapter_if_there_is_no_previous_chapter() {
        let list_of_chapters = ListOfChapters::default();
        let mut manga_reader: MangaReader<ReaderPageProvierMock, TrackerTest> =
            MangaReader::new(ChapterToRead::default(), "".to_string(), Picker::new((8, 8)), ReaderPageProvierMock::new().into())
                .with_list_of_chapters(list_of_chapters);

        manga_reader.initiate_search_previous_chapter();

        assert_eq!(manga_reader.state, State::DisplayingChapterNotFound);
    }

    #[tokio::test]
    async fn it_initiates_search_next_chapter() {
        let list_of_chapters: ListOfChapters = ListOfChapters {
            volumes: SortedVolumes::new(vec![Volumes {
                volume: "1".to_string(),
                chapters: SortedChapters::new(vec![
                    ChapterReader {
                        number: "1".to_string(),
                        ..Default::default()
                    },
                    ChapterReader {
                        id: "id_next_chapter".to_string(),
                        number: "2".to_string(),
                        ..Default::default()
                    },
                ]),
            }]),
        };

        let current_chapter: ChapterToRead = ChapterToRead {
            number: 1.0,
            volume_number: Some("1".to_string()),
            ..Default::default()
        };

        let mut manga_reader: MangaReader<ReaderPageProvierMock, TrackerTest> =
            MangaReader::new(current_chapter, "".to_string(), Picker::new((8, 8)), ReaderPageProvierMock::new().into())
                .with_list_of_chapters(list_of_chapters);

        manga_reader.initiate_search_next_chapter();

        let expected = MangaReaderEvents::SearchNextChapter("id_next_chapter".to_string());

        let result = timeout(Duration::from_millis(250), manga_reader.local_event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(expected, result);
        assert_eq!(State::SearchingChapter, manga_reader.state);
    }
    #[tokio::test]
    async fn it_initiates_search_previous_chapter() {
        let list_of_chapters: ListOfChapters = ListOfChapters {
            volumes: SortedVolumes::new(vec![Volumes {
                volume: "1".to_string(),
                chapters: SortedChapters::new(vec![
                    ChapterReader {
                        id: "id_previous_chapter".to_string(),
                        number: "1".to_string(),
                        ..Default::default()
                    },
                    ChapterReader {
                        number: "2".to_string(),
                        ..Default::default()
                    },
                ]),
            }]),
        };

        let current_chapter: ChapterToRead = ChapterToRead {
            number: 2.0,
            volume_number: Some("1".to_string()),
            ..Default::default()
        };

        let mut manga_reader: MangaReader<ReaderPageProvierMock, TrackerTest> =
            MangaReader::new(current_chapter, "".to_string(), Picker::new((8, 8)), ReaderPageProvierMock::new().into())
                .with_list_of_chapters(list_of_chapters);

        manga_reader.initiate_search_previous_chapter();

        let expected_event = tokio::time::timeout(Duration::from_millis(250), manga_reader.local_event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(expected_event, MangaReaderEvents::SearchPreviousChapter("id_previous_chapter".to_string()));
        assert_eq!(manga_reader.state, State::SearchingChapter);
    }

    #[tokio::test]
    async fn it_sends_search_next_chapter_action_on_w_key_press() {
        let mut manga_reader: MangaReader<ReaderPageProvierMock, TrackerTest> =
            MangaReader::new(ChapterToRead::default(), "".to_string(), Picker::new((8, 8)), ReaderPageProvierMock::new().into());

        press_key(&mut manga_reader, KeyCode::Char('w'));

        let expected_event = timeout(Duration::from_millis(250), manga_reader.local_action_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(expected_event, MangaReaderActions::SearchNextChapter);
    }

    #[tokio::test]
    async fn it_sends_search_previous_chapter_event_on_b_key_press() {
        let mut manga_reader: MangaReader<ReaderPageProvierMock, TrackerTest> =
            MangaReader::new(ChapterToRead::default(), "".to_string(), Picker::new((8, 8)), ReaderPageProvierMock::new().into());

        press_key(&mut manga_reader, KeyCode::Char('b'));

        let expected_event = timeout(Duration::from_millis(250), manga_reader.local_action_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(MangaReaderActions::SearchPreviousChapter, expected_event);
    }

    #[tokio::test]
    async fn it_searches_chapter_and_sends_successful_result() {
        let expected = ChapterToRead {
            id: "next_chapter_id".to_string(),
            title: "some_title".to_string(),
            number: 2.0,
            language: Languages::default(),
            volume_number: Some("1".to_string()),
            num_page_bookmarked: None,
            pages_url: vec!["http://localhost".parse().unwrap()],
        };

        let api_client = ReaderPageProvierMock::with_response(expected.clone());

        let mut manga_reader: MangaReader<ReaderPageProvierMock, TrackerTest> =
            MangaReader::new(ChapterToRead::default(), "some_id".to_string(), Picker::new((8, 8)), api_client.into());

        manga_reader.search_chapter("some_id".to_string());

        let result = timeout(Duration::from_millis(250), manga_reader.local_event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(result, MangaReaderEvents::LoadChapter(expected));
    }

    #[tokio::test]
    async fn it_searches_chapter_and_sends_error_event() {
        let api_client = ReaderPageProvierMock::with_failing_request();

        let mut manga_reader: MangaReader<ReaderPageProvierMock, TrackerTest> =
            MangaReader::new(ChapterToRead::default(), "some_id".to_string(), Picker::new((8, 8)), api_client.into());

        manga_reader.search_chapter("some_id".to_string());

        let result = timeout(Duration::from_millis(250), manga_reader.local_event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(result, MangaReaderEvents::ErrorSearchingChapter);
    }

    #[test]
    fn it_loads_chapter_found_and_sets_state_as_default() {
        let expected = ChapterToRead {
            id: "id_before".to_string(),
            title: "some_title".to_string(),
            language: Languages::default(),
            number: 1.0,
            num_page_bookmarked: None,
            volume_number: Some("1".to_string()),
            pages_url: vec![],
        };

        let api_client = ReaderPageProvierMock::with_response(expected.clone());

        let mut manga_reader: MangaReader<ReaderPageProvierMock, TrackerTest> =
            MangaReader::new(ChapterToRead::default(), "some_id".to_string(), Picker::new((8, 8)), api_client.into());

        manga_reader.state = State::SearchingChapter;

        manga_reader.load_chapter(expected.clone());

        assert_eq!(expected, manga_reader.current_chapter);
        assert_eq!(manga_reader.state, State::default());
    }

    #[test]
    fn it_resets_pages_after_chapter_was_found() {
        let mut manga_reader: MangaReader<ReaderPageProvierMock, TrackerTest> = MangaReader::new(
            ChapterToRead::default(),
            "some_id".to_string(),
            Picker::new((8, 8)),
            ReaderPageProvierMock::new().into(),
        );

        manga_reader.pages = vec![Page::new(), Page::new()];
        manga_reader.pages_list.pages = vec![PagesItem::new(1), PagesItem::new(1)];
        manga_reader.page_list_state.list_state.select(Some(1));

        manga_reader.load_chapter(ChapterToRead::default());

        assert!(manga_reader.pages.is_empty());
        assert!(manga_reader.pages_list.pages.is_empty());
        assert!(manga_reader.page_list_state.list_state.selected.is_none());
    }

    #[tokio::test]
    async fn it_send_event_to_search_pages_after_chapter_was_loaded() {
        let mut manga_reader: MangaReader<ReaderPageProvierMock, TrackerTest> =
            MangaReader::new(ChapterToRead::default(), "".to_string(), Picker::new((8, 8)), ReaderPageProvierMock::new().into());

        manga_reader.load_chapter(ChapterToRead::default());

        let expected = MangaReaderEvents::FetchPages;

        let result = timeout(Duration::from_millis(250), manga_reader.local_event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(expected, result)
    }

    #[tokio::test]
    async fn it_send_event_to_save_reading_status_to_database_after_chapter_was_loaded() {
        let mut manga_reader: MangaReader<ReaderPageProvierMock, TrackerTest> =
            MangaReader::new(ChapterToRead::default(), "".to_string(), Picker::new((8, 8)), ReaderPageProvierMock::new().into());

        let new_chapter: ChapterToRead = ChapterToRead {
            id: "chapter_to_save".to_string(),
            ..Default::default()
        };

        manga_reader.load_chapter(new_chapter);

        let expected = MangaReaderEvents::SaveReadingToDatabase;

        let mut events: Vec<MangaReaderEvents> = vec![];

        manga_reader.local_event_rx.recv_many(&mut events, 3).await;

        events.iter().find(|result| **result == expected).expect("expected event was not sent");
    }

    ///// This test should use a mock database instead
    //#[test]
    //fn it_save_reading_history() -> Result<(), rusqlite::Error> {
    //    let mut conn = Connection::open_in_memory()?;
    //
    //    let test_database = Database::new(&conn);
    //
    //    test_database.setup()?;
    //
    //    let chapter: ChapterToRead = ChapterToRead {
    //        id: "chapter_to_save".to_string(),
    //        ..Default::default()
    //    };
    //
    //    let manga_reader: MangaReader<ReaderPageProvierMock, TrackerTest> =
    //        MangaReader::new(chapter, "".to_string(), Picker::new((8, 8)), ReaderPageProvierMock::new().into());
    //
    //    let id_chapter_saved = manga_reader.save_reading_history(&mut conn)?;
    //
    //    let has_been_saved: bool =
    //        conn.query_row("SELECT is_read FROM chapters WHERE id = ?1", [id_chapter_saved], |row| row.get(0))?;
    //
    //    assert!(has_been_saved);
    //
    //    Ok(())
    //}

    /// Internally database is used which should be mocked
    //#[test]
    //fn it_loads_chapter_on_event() {
    //    let chapter_to_load = ChapterToRead {
    //        id: "new_chapter_id".to_string(),
    //        ..Default::default()
    //    };
    //
    //    let mut manga_reader: MangaReader<ReaderPageProvierMock, TrackerTest> = MangaReader::new(
    //        ChapterToRead::default(),
    //        "some_id".to_string(),
    //        Picker::new((8, 8)),
    //        ReaderPageProvierMock::new().into(),
    //    );
    //
    //    manga_reader
    //        .local_event_tx
    //        .send(MangaReaderEvents::LoadChapter(chapter_to_load.clone()))
    //        .ok();
    //
    //    manga_reader.tick();
    //
    //    assert_eq!(manga_reader.current_chapter.id, chapter_to_load.id);
    //}

    #[test]
    fn it_is_set_as_error_searching_chapter_on_event() {
        let mut manga_reader: MangaReader<ReaderPageProvierMock, TrackerTest> = MangaReader::new(
            ChapterToRead::default(),
            "some_id".to_string(),
            Picker::new((8, 8)),
            ReaderPageProvierMock::new().into(),
        );

        manga_reader.local_event_tx.send(MangaReaderEvents::ErrorSearchingChapter).ok();

        manga_reader.tick();

        assert_eq!(manga_reader.state, State::ErrorSearchingChapter);
    }

    #[derive(Default, Debug)]
    struct TestDatabase {
        should_fail: bool,
        bookmarked: bool,
    }

    impl TestDatabase {
        pub fn new() -> Self {
            Self {
                should_fail: false,
                bookmarked: false,
            }
        }

        pub fn was_bookmarked(self) -> bool {
            self.bookmarked
        }
    }

    impl Bookmark for TestDatabase {
        fn bookmark(&mut self, _chapter_to_bookmark: ChapterToBookmark<'_>) -> Result<(), Box<dyn std::error::Error>> {
            if self.should_fail {
                return Err("cannot bookmark chapter".into());
            }
            self.bookmarked = true;
            Ok(())
        }
    }

    #[test]
    fn it_sets_current_chapter_as_bookmarked_and_sets_state_as_bookmarked() {
        let mut manga_reader: MangaReader<ReaderPageProvierMock, TrackerTest> =
            MangaReader::new(ChapterToRead::default(), "".to_string(), Picker::new((8, 8)), ReaderPageProvierMock::new().into());

        let mut database = TestDatabase::new();

        manga_reader.set_current_chapter_bookmarked(Some(2), &mut database);

        assert!(database.was_bookmarked());
        assert_eq!(Some(2), manga_reader.page_list_state.page_bookmarked);
        assert_eq!(State::ManualBookmark, manga_reader.state)
    }

    #[test]
    fn when_pages_list_is_rendered_it_selects_page_with_num_page_bookmarked_and_highlights_it() {
        let chapter_to_read: ChapterToRead = ChapterToRead {
            num_page_bookmarked: Some(1),
            ..Default::default()
        };

        let mut manga_reader: MangaReader<ReaderPageProvierMock, TrackerTest> =
            MangaReader::new(chapter_to_read, "".to_string(), Picker::new((8, 8)), ReaderPageProvierMock::new().into());

        manga_reader.pages_list = PagesList::new(vec![PagesItem::new(0), PagesItem::new(1)]);

        let area = Rect::new(0, 0, 10, 10);
        let mut buf = Buffer::empty(area);

        StatefulWidget::render(manga_reader.pages_list.clone(), area, &mut buf, &mut manga_reader.page_list_state);

        assert_eq!(1, manga_reader.page_list_state.list_state.selected.expect("should not be none"));
    }

    #[tokio::test]
    async fn it_does_not_send_event_to_bookmark_chapter_on_m_key_press_if_autobookmarking_is_true() {
        let mut manga_reader: MangaReader<ReaderPageProvierMock, TrackerTest> =
            MangaReader::new(ChapterToRead::default(), "".to_string(), Picker::new((8, 8)), ReaderPageProvierMock::new().into());

        manga_reader.set_auto_bookmark();

        press_key(&mut manga_reader, KeyCode::Char('m'));

        assert!(manga_reader.local_action_rx.is_empty());
    }

    #[tokio::test]
    async fn it_sends_event_go_manga_page_on_exit() {
        let (tx, mut rx) = unbounded_channel::<Events>();
        let mut manga_reader: MangaReader<ReaderPageProvierMock, TrackerTest> =
            MangaReader::new(ChapterToRead::default(), "".to_string(), Picker::new((8, 8)), ReaderPageProvierMock::new().into())
                .with_global_sender(tx);

        manga_reader.exit();

        let expected = Events::GoBackMangaPage;

        let result = timeout(Duration::from_millis(250), rx.recv()).await.unwrap().unwrap();

        assert_eq!(expected, result)
    }

    #[tokio::test]
    async fn it_sends_event_to_log_manga_tracking_error() -> Result<(), Box<dyn Error>> {
        let chapter: ChapterToRead = ChapterToRead {
            id: "some_id".to_string(),
            title: "some_title".to_string(),
            number: 1.0,
            volume_number: Some(2.to_string()),
            ..Default::default()
        };

        let mut manga_reader: MangaReader<ReaderPageProvierMock, TrackerTest> =
            MangaReader::new(chapter.clone(), "".to_string(), Picker::new((8, 8)), ReaderPageProvierMock::new().into())
                .with_manga_title("some_title".to_string());

        let expected_error_message = "some_error_message";

        let tracker = TrackerTest::failing_with_error_message(expected_error_message);

        manga_reader.track_manga_reading_history(Some(tracker));

        let expected = MangaReaderEvents::ErrorTrackingReadingProgress(expected_error_message.to_string());

        let result = timeout(Duration::from_millis(500), manga_reader.local_event_rx.recv())
            .await?
            .expect("should send event");

        assert_eq!(expected, result);

        Ok(())
    }
}
