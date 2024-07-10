use crate::backend::tui::{Action, Events};
use crate::view::pages::*;
use ::crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::{self, Constraint, Layout, Rect};
use ratatui::style::Color;
use ratatui::widgets::{Block, Borders, Tabs, Widget};
use ratatui::Frame;
use ratatui_image::picker::Picker;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use self::home::Home;
use self::manga::MangaPage;
use self::reader::MangaReader;
use self::search::{InputMode, SearchPage};

use super::widgets::Component;

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum AppState {
    Runnning,
    Done,
}

pub struct App {
    picker: Picker,
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
}

impl Component for App {
    type Actions = Action;
    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) {
        if self.manga_reader_page.is_some() && self.current_tab == SelectedTabs::ReaderTab {
            self.manga_reader_page.as_mut().unwrap().render(area, frame);
        } else {
            let main_layout = Layout::default()
                .direction(layout::Direction::Vertical)
                .constraints([Constraint::Percentage(6), Constraint::Percentage(94)]);

            let [top_tabs_area, page_area] = main_layout.areas(area);

            self.render_top_tabs(top_tabs_area, frame.buffer_mut());

            self.render_pages(page_area, frame);
        }
    }

    fn handle_events(&mut self, events: Events) {
        match events {
            Events::Key(key_event) => {
                if self.search_page.input_mode != InputMode::Typing {
                    match key_event.code {
                        KeyCode::Char('c') => {
                            if let KeyModifiers::CONTROL = key_event.modifiers {
                                self.global_action_tx.send(Action::Quit).unwrap()
                            }
                        }
                        KeyCode::Char('2') => {
                            if self.manga_reader_page.is_none() {
                                self.global_action_tx.send(Action::GoToSearchPage).ok();
                            }
                        }

                        _ => {}
                    }
                }
            }
            Events::GoToMangaPage(manga) => {
                self.current_tab = SelectedTabs::MangaTab;
                self.manga_page = Some(MangaPage::new(
                    manga.id,
                    manga.title,
                    manga.description,
                    manga.tags,
                    manga.img_url,
                    manga.image_state,
                    manga.status,
                    manga.content_rating,
                    manga.author.unwrap_or_default(),
                    manga.artist.unwrap_or_default(),
                    self.global_event_tx.clone(),
                ));
            }

            //At this point the search must be cleared
            Events::ReadChapter(chapter_response) => {
                self.current_tab = SelectedTabs::ReaderTab;
                self.manga_reader_page = Some(MangaReader::new(
                    self.global_event_tx.clone(),
                    chapter_response.chapter.hash,
                    chapter_response.base_url,
                    self.picker,
                    chapter_response
                        .chapter
                        .data_saver
                        .iter()
                        .take(5)
                        .cloned()
                        .collect(),
                    chapter_response
                        .chapter
                        .data
                        .iter()
                        .skip(5)
                        .cloned()
                        .collect(),
                ));
            }
            Events::GoBackMangaPage => {
                self.current_tab = SelectedTabs::MangaTab;
            }
            Events::GoSearchPage => {
                self.current_tab = SelectedTabs::Search;
            }
            Events::GoToHome => {
                self.current_tab = SelectedTabs::Home;
            }

            _ => {}
        }
    }

    fn update(&mut self, action: Action) {
        match action {
            Action::Quit => {
                self.state = AppState::Done;
            }
            Action::PreviousTab => self.previous_tab(),
            Action::NextTab => self.next_tab(),
            Action::GoToSearchPage => {
                self.current_tab = SelectedTabs::Search;
                self.manga_reader_page = None;
                self.manga_page = None;
            }
            _ => {}
        }
    }
    fn clean_up(&mut self) {}
}

impl App {
    pub fn new() -> Self {
        let mut picker = Picker::from_termios().unwrap();

        picker.guess_protocol();

        let (global_action_tx, global_action_rx) = unbounded_channel::<Action>();
        let (global_event_tx, global_event_rx) = unbounded_channel::<Events>();

        App {
            picker,
            current_tab: SelectedTabs::default(),
            search_page: SearchPage::init(picker, global_event_tx.clone()),
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
        let titles: Vec<&str> = if self.current_tab == SelectedTabs::MangaTab {
            match self.manga_page.as_ref() {
                Some(page) => vec!["Search", &page.title],
                None => vec!["Search"],
            }
        } else {
            vec!["Search"]
        };

        let tabs_block = Block::default().borders(Borders::BOTTOM);

        let current_page = self.current_tab as usize;

        Tabs::new(titles)
            .block(tabs_block)
            .highlight_style(Color::Yellow)
            .select(current_page)
            .padding("", "")
            .divider(" | ")
            .render(area, buf);
    }

    pub fn render_pages(&mut self, area: Rect, frame: &mut Frame<'_>) {
        match self.current_tab {
            SelectedTabs::Search => self.render_search_page(area, frame),
            SelectedTabs::MangaTab => self.render_manga_page(area, frame),
            SelectedTabs::Home => self.render_home_page(area, frame),
            // Reader tab should be on full screen
            SelectedTabs::ReaderTab => {}
        }
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

    pub fn next_tab(&mut self) {
        self.current_tab = self.current_tab.next();
    }

    pub fn previous_tab(&mut self) {
        self.current_tab = self.current_tab.previous();
    }
}
