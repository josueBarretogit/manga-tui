use self::feed::Feed;
use self::home::Home;
use self::manga::MangaPage;
use self::reader::MangaReader;
use self::search::{InputMode, SearchPage};
use crate::backend::tui::{Action, Events};
use crate::global::INSTRUCTIONS_STYLE;
use crate::view::pages::*;
use ::crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::{Block, Borders, Tabs, Widget};
use ratatui::Frame;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use super::widgets::Component;

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
    pub current_tab: SelectedTabs,
    pub manga_page: Option<MangaPage>,
    pub manga_reader_page: Option<MangaReader>,
    pub search_page: SearchPage,
    pub home_page: Home,
    pub feed_page: Feed,
}

impl Component for App {
    type Actions = Action;
    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) {
        if self.manga_reader_page.is_some() && self.current_tab == SelectedTabs::ReaderTab {
            self.manga_reader_page.as_mut().unwrap().render(area, frame);
        } else {
            let main_layout =
                Layout::vertical([Constraint::Percentage(6), Constraint::Percentage(94)]);

            let [top_tabs_area, page_area] = main_layout.areas(area);

            self.render_top_tabs(top_tabs_area, frame.buffer_mut());

            self.render_pages(page_area, frame);
        }
    }

    fn handle_events(&mut self, events: Events) {
        match events {
            Events::Key(key_event) => {
                if self.search_page.input_mode != InputMode::Typing
                    && !self.search_page.is_typing_filter()
                    && !self.feed_page.is_typing()
                {
                    match key_event.code {
                        KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => {
                            self.global_action_tx.send(Action::Quit).ok();
                        }
                        KeyCode::Char('u') | KeyCode::F(1) => {
                            if self.current_tab != SelectedTabs::ReaderTab {
                                self.global_event_tx.send(Events::GoToHome).ok();
                            }
                        }
                        KeyCode::Char('i') | KeyCode::F(2) => {
                            if self.current_tab != SelectedTabs::ReaderTab {
                                self.global_event_tx.send(Events::GoSearchPage).ok();
                            }
                        }
                        KeyCode::Char('o') | KeyCode::F(3) => {
                            if self.current_tab != SelectedTabs::ReaderTab {
                                self.global_event_tx.send(Events::GoFeedPage).ok();
                            }
                        }
                        KeyCode::Backspace => {
                            if self.current_tab == SelectedTabs::ReaderTab
                                && self.manga_reader_page.is_some()
                            {
                                self.manga_reader_page.as_mut().unwrap().clean_up();
                                self.current_tab = SelectedTabs::MangaTab;
                            }
                        }

                        _ => {}
                    }
                }
            }
            Events::GoToMangaPage(manga) => {
                if self.manga_reader_page.is_some() {
                    self.manga_reader_page.as_mut().unwrap().clean_up();
                    self.manga_reader_page = None;
                }

                self.feed_page.clean_up();

                self.current_tab = SelectedTabs::MangaTab;
                self.manga_page = Some(MangaPage::new(
                    manga.manga,
                    manga.image_state,
                    self.global_event_tx.clone(),
                ));
            }

            Events::ReadChapter(chapter_response) => {
                self.home_page.clean_up();
                self.feed_page.clean_up();
                self.current_tab = SelectedTabs::ReaderTab;
                self.manga_reader_page = Some(MangaReader::new(
                    self.global_event_tx.clone(),
                    chapter_response.chapter.hash,
                    chapter_response.base_url,
                    chapter_response.chapter.data_saver,
                    chapter_response.chapter.data,
                ));
            }

            Events::GoSearchPage => {
                self.go_search_page();
            }

            Events::GoToHome => {
                if self.manga_page.is_some() {
                    self.manga_page.as_mut().unwrap().clean_up();
                    self.manga_page = None;
                }

                self.feed_page.clean_up();

                if self.home_page.require_search() {
                    self.home_page.init_search();
                }

                self.current_tab = SelectedTabs::Home;
            }
            Events::GoFeedPage => {
                if self.manga_page.is_some() {
                    self.manga_page.as_mut().unwrap().clean_up();
                    self.manga_page = None;
                }
                self.feed_page.init_search();
                self.current_tab = SelectedTabs::Feed;
            }

            Events::GoSearchMangasAuthor(author) => {
                self.go_search_page();
                self.search_page.search_mangas_of_author(author);
            }

            Events::GoSearchMangasArtist(artist) => {
                self.go_search_page();
                self.search_page.search_mangas_of_artist(artist);
            }

            _ => {}
        }
    }

    fn update(&mut self, action: Action) {
        match action {
            Action::Quit => {
                self.state = AppState::Done;
            }
        }
    }
    fn clean_up(&mut self) {}
}

impl App {
    pub fn new() -> Self {
        let (global_action_tx, global_action_rx) = unbounded_channel::<Action>();
        let (global_event_tx, global_event_rx) = unbounded_channel::<Events>();

        global_event_tx.send(Events::GoSearchPage).ok();

        App {
            current_tab: SelectedTabs::default(),
            search_page: SearchPage::init(global_event_tx.clone()),
            feed_page: Feed::new(global_event_tx.clone()),
            home_page: Home::new(global_event_tx.clone()),
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
            SelectedTabs::Home => 0,
            SelectedTabs::Search => 1,
            SelectedTabs::Feed => 2,
            SelectedTabs::MangaTab => {
                titles.push(" 📖 Manga page");
                3
            }
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
            SelectedTabs::Search => self.render_search_page(area, frame),
            SelectedTabs::MangaTab => self.render_manga_page(area, frame),
            SelectedTabs::Home => self.render_home_page(area, frame),
            SelectedTabs::Feed => self.render_feed_page(area, frame),
            // Reader tab should be on full screen
            SelectedTabs::ReaderTab => {}
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

    fn go_search_page(&mut self) {
        if self.manga_page.is_some() {
            self.manga_page.as_mut().unwrap().clean_up();
            self.manga_page = None;
        }

        self.feed_page.clean_up();
        self.current_tab = SelectedTabs::Search;
    }
}
