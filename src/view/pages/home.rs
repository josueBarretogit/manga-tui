use core::panic;
use crossterm::event::{KeyCode, KeyEvent};
use image::io::Reader;
use image::DynamicImage;
use ratatui::{prelude::*, widgets::*};
use std::io::Cursor;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;

use crate::backend::fetch::MangadexClient;
use crate::backend::tui::Events;
use crate::backend::SearchMangaResponse;
use crate::utils::search_manga_cover;
use crate::view::widgets::home::{CarrouselItem, PopularMangaCarrousel, RecentlyAddedCarrousel};
use crate::view::widgets::search::MangaItem;
use crate::view::widgets::{Component, ImageHandler};
use crate::PICKER;

#[derive(PartialEq, Eq)]
pub enum HomeState {
    Unused,
    SearchingPopularMangas,
    DisplayingPopularMangas,
    NotFound,
}

pub enum HomeEvents {
    SearchPopularNewMangas,
    SearchPopularMangasCover,
    SearchRecentlyAddedMangas,
    SearchRecentlyCover,
    LoadPopularMangas(Option<SearchMangaResponse>),
    LoadRecentlyAddedMangas(Option<SearchMangaResponse>),
    LoadCover(Option<DynamicImage>, String),
    LoadRecentlyAddedMangasCover(Option<DynamicImage>, String),
}

impl ImageHandler for HomeEvents {
    fn load(image: DynamicImage, id: String) -> Self {
        Self::LoadCover(Some(image), id)
    }
    fn not_found(id: String) -> Self {
        Self::LoadCover(None, id)
    }
}

pub enum HomeActions {
    SelectNextPopularManga,
    SelectPreviousPopularManga,
    GoToPopularMangaPage,
    SelectNextRecentlyAddedManga,
    SelectPreviousRecentlyAddedManga,
}

pub struct Home {
    pub global_event_tx: UnboundedSender<Events>,
    carrousel_popular_mangas: PopularMangaCarrousel,
    carrousel_recently_added: RecentlyAddedCarrousel,
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
        let layout =
            Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).margin(1);
        let buf = frame.buffer_mut();

        let [carrousel_popular_mangas_area, latest_updates_area] = layout.areas(area);
        match self.state {
            HomeState::SearchingPopularMangas => {
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
                self.render_popular_mangas_carrousel(carrousel_popular_mangas_area, buf);
            }
            HomeState::Unused => {}
        }
        self.render_recently_added_mangas(latest_updates_area, buf);
    }

    fn update(&mut self, action: Self::Actions) {
        match action {
            HomeActions::SelectNextPopularManga => {
                self.carrousel_popular_mangas.next();
            }
            HomeActions::SelectPreviousPopularManga => self.carrousel_popular_mangas.previous(),
            HomeActions::GoToPopularMangaPage => self.go_to_manga_page(),
            HomeActions::SelectNextRecentlyAddedManga => {
                self.carrousel_recently_added.select_next()
            }
            HomeActions::SelectPreviousRecentlyAddedManga => {
                self.carrousel_recently_added.select_previous()
            }
        }
    }

    fn clean_up(&mut self) {
        self.tasks.abort_all();
        self.carrousel_popular_mangas.items.clear();
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

        local_event_tx.send(HomeEvents::SearchPopularNewMangas).ok();

        local_event_tx
            .send(HomeEvents::SearchRecentlyAddedMangas)
            .ok();
        Self {
            carrousel_popular_mangas: PopularMangaCarrousel::default(),
            carrousel_recently_added: RecentlyAddedCarrousel::default(),
            state: HomeState::Unused,
            global_event_tx: tx,
            local_event_tx,
            local_event_rx,
            local_action_tx,
            local_action_rx,
            tasks: JoinSet::new(),
        }
    }
    pub fn render_popular_mangas_carrousel(&mut self, area: Rect, buf: &mut Buffer) {
        StatefulWidget::render(
            self.carrousel_popular_mangas.clone(),
            area,
            buf,
            &mut self.carrousel_popular_mangas.current_item_visible_index,
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
        self.carrousel_popular_mangas.get_current_item()
    }

    pub fn require_search(&mut self) -> bool {
        self.carrousel_popular_mangas.items.is_empty()
    }

    pub fn init_search(&mut self) {
        if self.state != HomeState::SearchingPopularMangas {
            self.local_event_tx
                .send(HomeEvents::SearchPopularNewMangas)
                .ok();

            self.local_event_tx
                .send(HomeEvents::SearchRecentlyAddedMangas)
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
                HomeEvents::SearchRecentlyAddedMangas => {
                    self.search_recently_added_mangas();
                }
                HomeEvents::LoadRecentlyAddedMangas(maybe_response) => {
                    self.load_recently_added_mangas(maybe_response);
                }
                HomeEvents::SearchRecentlyCover => {
                    self.search_recently_added_mangas_cover();
                }
                HomeEvents::LoadRecentlyAddedMangasCover(maybe_image, id) => {
                    self.load_recently_added_mangas_cover(maybe_image, id);
                }
            }
        }
    }

    fn load_popular_mangas(&mut self, maybe_response: Option<SearchMangaResponse>) {
        match maybe_response {
            Some(response) => {
                self.state = HomeState::DisplayingPopularMangas;
                self.carrousel_popular_mangas = PopularMangaCarrousel::from_response(response);
                if PICKER.is_some() {
                    self.local_event_tx
                        .send(HomeEvents::SearchPopularMangasCover)
                        .ok();
                }
            }
            None => {
                self.state = HomeState::NotFound;
            }
        }
    }

    fn load_popular_manga_cover(&mut self, maybe_cover: Option<DynamicImage>, id: String) {
        match maybe_cover {
            Some(cover) => {
                if let Some(popular_manga) = self
                    .carrousel_popular_mangas
                    .items
                    .iter_mut()
                    .find(|manga| manga.id == id)
                {
                    let image = PICKER.unwrap().new_resize_protocol(cover);
                    popular_manga.cover_state = Some(image);
                }
            }
            None => {
                // Todo! image could not be rendered
            }
        }
    }

    fn search_popular_mangas(&mut self) {
        self.state = HomeState::SearchingPopularMangas;
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
        for manga in self.carrousel_popular_mangas.items.iter() {
            let manga_id = manga.id.clone();
            let tx = self.local_event_tx.clone();
            match manga.img_url.as_ref() {
                Some(file_name) => {
                    let file_name = file_name.clone();
                    search_manga_cover(file_name, manga_id, &mut self.tasks, tx);
                }
                None => {
                    tx.send(HomeEvents::LoadCover(None, manga.id.clone())).ok();
                }
            };
        }
    }

    fn search_recently_added_mangas(&mut self) {
        let tx = self.local_event_tx.clone();
        self.tasks.spawn(async move {
            let response = MangadexClient::global().get_recently_added().await;
            match response {
                Ok(mangas) => {
                    tx.send(HomeEvents::LoadRecentlyAddedMangas(Some(mangas)))
                        .ok();
                }
                Err(e) => {
                    panic!("error fetching mangas {e}");
                    tx.send(HomeEvents::LoadRecentlyAddedMangas(None)).ok();
                }
            }
        });
    }

    fn load_recently_added_mangas(&mut self, maybe_response: Option<SearchMangaResponse>) {
        match maybe_response {
            Some(response) => {
                self.carrousel_recently_added = RecentlyAddedCarrousel::from_response(response);
                if PICKER.is_some() {
                    self.local_event_tx
                        .send(HomeEvents::SearchRecentlyCover)
                        .ok();
                }
            }
            None => {
                // self.state = HomeState::NotFound;
            }
        }
    }

    fn search_recently_added_mangas_cover(&mut self) {
        for manga in self.carrousel_recently_added.items.iter() {
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
                                let dyn_img = Reader::new(Cursor::new(bytes))
                                    .with_guessed_format()
                                    .unwrap();

                                let maybe_decoded = dyn_img.decode();
                                match maybe_decoded {
                                    Ok(image) => {
                                        tx.send(HomeEvents::LoadRecentlyAddedMangasCover(
                                            Some(image),
                                            manga_id,
                                        ))
                                        .unwrap();
                                    }
                                    Err(_) => {
                                        tx.send(HomeEvents::LoadRecentlyAddedMangasCover(
                                            None, manga_id,
                                        ))
                                        .unwrap();
                                    }
                                };
                            }
                            Err(_) => tx
                                .send(HomeEvents::LoadRecentlyAddedMangasCover(None, manga_id))
                                .unwrap(),
                        }
                    });
                }
                None => {
                    tx.send(HomeEvents::LoadRecentlyAddedMangasCover(
                        None,
                        manga.id.clone(),
                    ))
                    .ok();
                }
            };
        }
    }

    fn load_recently_added_mangas_cover(&mut self, maybe_cover: Option<DynamicImage>, id: String) {
        match maybe_cover {
            Some(cover) => {
                if let Some(recently_added_manga) = self
                    .carrousel_recently_added
                    .items
                    .iter_mut()
                    .find(|manga| manga.id == id)
                {
                    let image = PICKER.unwrap().new_resize_protocol(cover);
                    recently_added_manga.cover_state = Some(image);
                }
            }
            None => {
                // Todo! image could not be rendered
            }
        }
    }

    fn render_recently_added_mangas(&mut self, area: Rect, buf: &mut Buffer) {
        StatefulWidget::render(
            self.carrousel_recently_added.clone(),
            area,
            buf,
            &mut self.carrousel_recently_added.selected_item_index,
        );
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
            KeyCode::Char('l') | KeyCode::Right => {
                if self.state == HomeState::DisplayingPopularMangas {
                    self.local_action_tx
                        .send(HomeActions::SelectNextRecentlyAddedManga)
                        .ok();
                }
            }
            KeyCode::Char('h') | KeyCode::Left => {
                if self.state == HomeState::DisplayingPopularMangas {
                    self.local_action_tx
                        .send(HomeActions::SelectPreviousRecentlyAddedManga)
                        .ok();
                }
            }
            _ => {}
        }
    }
}
