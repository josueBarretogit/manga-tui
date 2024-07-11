use core::panic;
use crossterm::event::{KeyCode, KeyEvent};
use image::io::Reader;
use image::DynamicImage;
use ratatui::{prelude::*, widgets::*};
use ratatui_image::picker::Picker;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;

use crate::backend::fetch::MangadexClient;
use crate::backend::tui::Events;
use crate::backend::SearchMangaResponse;
use crate::view::widgets::home::{Carrousel, CarrouselItem};
use crate::view::widgets::search::MangaItem;
use crate::view::widgets::Component;

#[derive(PartialEq, Eq)]
pub enum HomeState {
    Unused,
    Searching,
    DisplayingPopularMangas,
    NotFound,
}

pub enum HomeEvents {
    SearchPopularNewMangas,
    SearchPopularMangasCover,
    LoadPopularMangas(Option<SearchMangaResponse>),
    LoadCover(Option<DynamicImage>, usize),
}

pub enum HomeActions {
    SelectNextPopularManga,
    SelectPreviousPopularManga,
    GoToPopularMangaPage,
}

pub struct Home {
    pub global_event_tx: UnboundedSender<Events>,
    carrousel: Carrousel,
    state: HomeState,
    pub local_action_tx: UnboundedSender<HomeActions>,
    pub local_action_rx: UnboundedReceiver<HomeActions>,
    pub local_event_tx: UnboundedSender<HomeEvents>,
    pub local_event_rx: UnboundedReceiver<HomeEvents>,
    tasks: JoinSet<()>,
}

impl Component for Home {
    type Actions = HomeActions;
    fn render(&mut self, area: ratatui::prelude::Rect, frame: &mut Frame<'_>) {
        let layout = Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]);
        let buf = frame.buffer_mut();

        let [carrousel_popular_mangas_area, latest_updates_area] = layout.areas(area);
        match self.state {
            HomeState::Searching => {
                Block::bordered()
                    .title("loading")
                    .render(carrousel_popular_mangas_area, buf);
            }
            HomeState::NotFound => {
                Block::bordered()
                    .title("error fetching")
                    .render(carrousel_popular_mangas_area, buf);
            }
            HomeState::DisplayingPopularMangas => {
                self.render_carrousel(carrousel_popular_mangas_area, buf);
            }
            HomeState::Unused => {}
        }
    }

    fn update(&mut self, action: Self::Actions) {
        match action {
            HomeActions::SelectNextPopularManga => {
                self.carrousel.next();
            }
            HomeActions::SelectPreviousPopularManga => self.carrousel.previous(),
            HomeActions::GoToPopularMangaPage => self.go_to_manga_page(),
        }
    }

    fn clean_up(&mut self) {
        self.tasks.abort_all();
        self.carrousel.items.clear();
        self.state = HomeState::Unused;
    }

    fn handle_events(&mut self, events: Events) {
        match events {
            Events::Key(key_event) => self.handle_key_events(key_event),
            Events::Tick => self.tick(),
            _ => {}
        }
    }
}

impl Home {
    pub fn new(tx: UnboundedSender<Events>) -> Self {
        let (local_action_tx, local_action_rx) = mpsc::unbounded_channel::<HomeActions>();
        let (local_event_tx, local_event_rx) = mpsc::unbounded_channel::<HomeEvents>();

        Self {
            carrousel: Carrousel::default(),
            state: HomeState::Unused,
            global_event_tx: tx,
            local_event_tx,
            local_event_rx,
            local_action_tx,
            local_action_rx,
            tasks: JoinSet::new(),
        }
    }
    pub fn render_carrousel(&mut self, area: Rect, buf: &mut Buffer) {
        StatefulWidget::render(
            self.carrousel.clone(),
            area,
            buf,
            &mut self.carrousel.current_item_visible_index,
        );
    }

    pub fn go_to_manga_page(&self) {
        if let Some(manga) = self.get_current_popular_manga() {
            self.global_event_tx
                .send(Events::GoToMangaPage(MangaItem::new(
                    manga.id.clone(),
                    manga.title.clone(),
                    manga.description.clone(),
                    manga.tags.clone(),
                    manga.content_rating.clone(),
                    manga.status.clone(),
                    manga.img_url.clone(),
                    manga.author.clone(),
                    manga.artist.clone(),
                    manga.cover_state.clone(),
                )))
                .ok();
        }
    }

    fn get_current_popular_manga(&self) -> Option<&CarrouselItem> {
        self.carrousel.get_current_item()
    }

    pub fn init_search(&mut self) {
        if self.state != HomeState::Searching {
            self.local_event_tx
                .send(HomeEvents::SearchPopularNewMangas)
                .ok();
        }
    }

    pub fn tick(&mut self) {
        if let Ok(local_event) = self.local_event_rx.try_recv() {
            match local_event {
                HomeEvents::SearchPopularMangasCover => self.search_popular_mangas_cover(),
                HomeEvents::SearchPopularNewMangas => self.search_popular_mangas(),
                HomeEvents::LoadPopularMangas(maybe_response) => {
                    self.load_popular_mangas(maybe_response);
                }
                HomeEvents::LoadCover(maybe_cover, index) => {
                    self.load_popular_manga_cover(maybe_cover, index)
                }
            }
        }
    }

    fn load_popular_mangas(&mut self, maybe_response: Option<SearchMangaResponse>) {
        match maybe_response {
            Some(response) => {
                self.state = HomeState::DisplayingPopularMangas;
                self.carrousel = Carrousel::from_response(response);
                self.local_event_tx
                    .send(HomeEvents::SearchPopularMangasCover)
                    .ok();
            }
            None => {
                self.state = HomeState::NotFound;
            }
        }
    }

    fn load_popular_manga_cover(&mut self, maybe_cover: Option<DynamicImage>, index: usize) {
        match maybe_cover {
            Some(cover) => {
                if let Some(popular_manga) = self.carrousel.items.get_mut(index) {
                    let mut picker = Picker::from_termios().unwrap();
                    picker.guess_protocol();

                    let image = picker.new_resize_protocol(cover);
                    popular_manga.cover_state = Some(image);
                }
            }
            None => {
                // Todo! image could not be rendered
            }
        }
    }

    fn search_popular_mangas(&mut self) {
        self.state = HomeState::Searching;
        let tx = self.local_event_tx.clone();
        self.tasks.spawn(async move {
            let response = MangadexClient::global().get_popular_mangas().await;
            match response {
                Ok(mangas) => {
                    tx.send(HomeEvents::LoadPopularMangas(Some(mangas))).ok();
                }
                Err(e) => {
                    panic!("error fetching mangas {e}");
                    tx.send(HomeEvents::LoadPopularMangas(None)).ok();
                }
            }
        });
    }

    fn search_popular_mangas_cover(&mut self) {
        for (index, manga) in self.carrousel.items.iter().enumerate() {
            let manga_id = manga.id.clone();
            let tx = self.local_event_tx.clone();

            match manga.img_url.as_ref() {
                Some(file_name) => {
                    let file_name = file_name.clone();
                    self.tasks.spawn(async move {
                        let response = MangadexClient::global()
                            .get_cover_for_manga(&manga_id, &file_name)
                            .await;

                        match response {
                            Ok(bytes) => {
                                let dyn_img = Reader::new(std::io::Cursor::new(bytes))
                                    .with_guessed_format()
                                    .unwrap();

                                let maybe_decoded = dyn_img.decode();
                                match maybe_decoded {
                                    Ok(image) => {
                                        tx.send(HomeEvents::LoadCover(Some(image), index)).unwrap();
                                    }
                                    Err(_) => {
                                        tx.send(HomeEvents::LoadCover(None, index)).unwrap();
                                    }
                                };
                            }
                            Err(e) => {
                                panic!("error fetching cover: {e}");
                            }
                        }
                    });
                }
                None => {
                    tx.send(HomeEvents::LoadCover(None, index)).unwrap();
                }
            };
        }
    }

    pub fn handle_key_events(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('w') => {
                if self.state == HomeState::DisplayingPopularMangas {
                    self.local_action_tx
                        .send(HomeActions::SelectNextPopularManga)
                        .ok();
                }
            }

            KeyCode::Char('b') => {
                if self.state == HomeState::DisplayingPopularMangas {
                    self.local_action_tx
                        .send(HomeActions::SelectPreviousPopularManga)
                        .ok();
                }
            }
            KeyCode::Char('r') => {
                if self.state == HomeState::DisplayingPopularMangas {
                    self.local_action_tx
                        .send(HomeActions::GoToPopularMangaPage)
                        .ok();
                }
            }
            _ => {}
        }
    }
}
