use ::crossterm::event::KeyCode;
use crossterm::event::{KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::{Block, Borders, Tabs, Widget};
use ratatui::Frame;
use ratatui_image::picker::{Picker, ProtocolType};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use self::feed::Feed;
use self::home::Home;
use self::manga::MangaPage;
use self::reader::MangaReader;
use self::search::{InputMode, SearchPage};
use super::widgets::search::MangaItem;
use super::widgets::Component;
use crate::backend::tui::{Action, Events};
use crate::backend::ChapterPagesResponse;
use crate::global::INSTRUCTIONS_STYLE;
use crate::view::pages::*;

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum AppState {
    Runnning,
    Done,
}

pub struct App {
    pub global_action_tx: UnboundedSender<Action>,
    pub global_action_rx: UnboundedReceiver<Action>,
    pub global_event_tx: UnboundedSender<Events>,
    pub global_event_rx: UnboundedReceiver<Events>,
    pub state: AppState,
    pub current_tab: SelectedPage,
    pub manga_page: Option<MangaPage>,
    pub manga_reader_page: Option<MangaReader>,
    pub search_page: SearchPage,
    pub home_page: Home,
    pub feed_page: Feed,
    // The picker is what decides how big a image needs to be rendered depending on the user's
    // terminal font size and the graphics it supports
    // if the terminal doesn't support any graphics protocol the picker is `None`
    picker: Option<Picker>,
}

impl Component for App {
    type Actions = Action;

    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) {
        if self.manga_reader_page.is_some() && self.current_tab == SelectedPage::ReaderTab {
            self.manga_reader_page.as_mut().unwrap().render(area, frame);
        } else {
            let main_layout = Layout::vertical([Constraint::Percentage(6), Constraint::Percentage(94)]);

            let [top_tabs_area, page_area] = main_layout.areas(area);

            self.render_top_tabs(top_tabs_area, frame.buffer_mut());

            self.render_pages(page_area, frame);
        }
    }

    fn handle_events(&mut self, events: Events) {
        match events {
            Events::Key(key_event) => self.handle_key_events(key_event),
            Events::GoToMangaPage(manga) => self.go_to_manga_page(manga),
            Events::ReadChapter(chapter_response) => self.go_to_read_chapter(chapter_response),
            Events::GoSearchPage => {
                self.go_search_page();
            },
            Events::GoToHome => self.go_to_home(),
            Events::GoFeedPage => self.go_feed_page(),

            Events::GoSearchMangasAuthor(author) => {
                self.go_search_page();
                self.search_page.search_mangas_of_author(author);
            },
            Events::GoSearchMangasArtist(artist) => {
                self.go_search_page();
                self.search_page.search_mangas_of_artist(artist);
            },
            _ => {},
        }
    }

    fn update(&mut self, action: Action) {
        match action {
            Action::Quit => {
                self.state = AppState::Done;
            },
        }
    }

    fn clean_up(&mut self) {}
}

impl App {
    pub fn new() -> Self {
        let (global_action_tx, global_action_rx) = unbounded_channel::<Action>();
        let (global_event_tx, global_event_rx) = unbounded_channel::<Events>();

        global_event_tx.send(Events::GoToHome).ok();

        let picker = get_picker();

        App {
            picker,
            current_tab: SelectedPage::default(),
            search_page: SearchPage::init(global_event_tx.clone(), picker),
            feed_page: Feed::new(global_event_tx.clone()),
            home_page: Home::new(global_event_tx.clone(), picker),
            manga_page: None,
            manga_reader_page: None,
            global_action_tx,
            global_action_rx,
            global_event_tx,
            global_event_rx,
            state: AppState::Runnning,
        }
    }

    pub fn render_top_tabs(&self, area: Rect, buf: &mut Buffer) {
        let mut titles: Vec<&str> = vec!["Home <F1>/<u>", "Search <F2>/<i>", "Feed <F3>/<o>"];

        let tabs_block = Block::default().borders(Borders::BOTTOM);

        let index_current_tab = match self.current_tab {
            SelectedPage::Home => 0,
            SelectedPage::Search => 1,
            SelectedPage::Feed => 2,
            SelectedPage::MangaTab => {
                titles.push(" 📖 Manga page");
                3
            },
            _ => 0,
        };

        Tabs::new(titles)
            .block(tabs_block)
            .highlight_style(*INSTRUCTIONS_STYLE)
            .select(index_current_tab)
            .padding("", "")
            .divider(" | ")
            .render(area, buf);
    }

    pub fn render_pages(&mut self, area: Rect, frame: &mut Frame<'_>) {
        match self.current_tab {
            SelectedPage::Search => self.render_search_page(area, frame),
            SelectedPage::MangaTab => self.render_manga_page(area, frame),
            SelectedPage::Home => self.render_home_page(area, frame),
            SelectedPage::Feed => self.render_feed_page(area, frame),
            // Reader tab should be on full screen
            SelectedPage::ReaderTab => {},
        }
    }

    fn render_feed_page(&mut self, area: Rect, frame: &mut Frame<'_>) {
        self.feed_page.render(area, frame);
    }

    pub fn render_search_page(&mut self, area: Rect, frame: &mut Frame<'_>) {
        self.search_page.render(area, frame);
    }

    pub fn render_manga_page(&mut self, area: Rect, frame: &mut Frame<'_>) {
        if let Some(page) = self.manga_page.as_mut() {
            page.render(area, frame);
        }
    }

    pub fn render_home_page(&mut self, area: Rect, frame: &mut Frame<'_>) {
        self.home_page.render(area, frame);
    }

    fn handle_key_events(&mut self, key_event: KeyEvent) {
        if self.manga_page.as_ref().is_some_and(|page| page.is_downloading_all_chapters()) {
            return;
        }

        if self.search_page.input_mode != InputMode::Typing && !self.search_page.is_typing_filter() && !self.feed_page.is_typing() {
            match key_event.code {
                KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => {
                    self.global_action_tx.send(Action::Quit).ok();
                },
                KeyCode::Char('u') | KeyCode::F(1) => {
                    if self.current_tab != SelectedPage::ReaderTab {
                        self.global_event_tx.send(Events::GoToHome).ok();
                    }
                },
                KeyCode::Char('i') | KeyCode::F(2) => {
                    if self.current_tab != SelectedPage::ReaderTab {
                        self.global_event_tx.send(Events::GoSearchPage).ok();
                    }
                },
                KeyCode::Char('o') | KeyCode::F(3) => {
                    if self.current_tab != SelectedPage::ReaderTab {
                        self.global_event_tx.send(Events::GoFeedPage).ok();
                    }
                },
                KeyCode::Backspace => {
                    if self.current_tab == SelectedPage::ReaderTab && self.manga_reader_page.is_some() {
                        self.manga_reader_page.as_mut().unwrap().clean_up();
                        self.current_tab = SelectedPage::MangaTab;
                    }
                },

                _ => {},
            }
        }
    }

    fn go_search_page(&mut self) {
        if self.manga_page.is_some() {
            self.manga_page.as_mut().unwrap().clean_up();
            self.manga_page = None;
        }
        self.feed_page.clean_up();
        self.current_tab = SelectedPage::Search;
    }

    fn go_to_manga_page(&mut self, manga: MangaItem) {
        if self.manga_reader_page.is_some() {
            self.manga_reader_page.as_mut().unwrap().clean_up();
            self.manga_reader_page = None;
        }

        self.feed_page.clean_up();

        self.current_tab = SelectedPage::MangaTab;
        self.manga_page = Some(MangaPage::new(manga.manga, self.global_event_tx.clone(), self.picker));
    }

    fn go_to_read_chapter(&mut self, chapter_response: ChapterPagesResponse) {
        self.home_page.clean_up();
        self.feed_page.clean_up();
        self.current_tab = SelectedPage::ReaderTab;
        self.manga_reader_page = Some(MangaReader::new(
            self.global_event_tx.clone(),
            chapter_response.chapter.hash,
            chapter_response.base_url,
            chapter_response.chapter.data_saver,
            chapter_response.chapter.data,
            self.picker.as_ref().cloned().unwrap(),
        ));
    }

    fn go_to_home(&mut self) {
        if self.manga_page.is_some() {
            self.manga_page.as_mut().unwrap().clean_up();
            self.manga_page = None;
        }

        self.feed_page.clean_up();

        if self.home_page.require_search() {
            self.home_page.init_search();
        }

        self.current_tab = SelectedPage::Home;
    }

    fn go_feed_page(&mut self) {
        if self.manga_page.is_some() {
            self.manga_page.as_mut().unwrap().clean_up();
            self.manga_page = None;
        }
        self.feed_page.init_search();
        self.current_tab = SelectedPage::Feed;
    }
}

#[cfg(unix)]
fn get_picker() -> Option<Picker> {
    Picker::from_termios()
        .ok()
        .map(|mut picker| {
            picker.guess_protocol();
            picker
        })
        .filter(|picker| picker.protocol_type != ProtocolType::Halfblocks)
}
#[cfg(target_os = "windows")]
fn get_picker() -> Option<Picker> {
    use windows_sys::Win32::System::Console::GetConsoleWindow;
    use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;

    struct FontSize {
        pub width: u16,
        pub height: u16,
    }

    let size: FontSize = match unsafe { GetDpiForWindow(GetConsoleWindow()) } {
        96 => FontSize {
            width: 9,
            height: 20,
        },
        120 => FontSize {
            width: 12,
            height: 25,
        },
        144 => FontSize {
            width: 14,
            height: 32,
        },
        _ => unimplemented!("Other DPIs then 96 (100%), 120 (125%) and 144 (150%) are supported as of now"),
    };

    let mut picker = Picker::new((size.width, size.height));

    let protocol = picker.guess_protocol();

    if protocol == ProtocolType::Halfblocks {
        return None;
    }
    Some(picker)
}
