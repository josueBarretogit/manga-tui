use crossterm::event::{KeyCode, KeyEvent};
use image::io::Reader;
use image::DynamicImage;
use ratatui::{prelude::*, widgets::*};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};
use std::env;
use std::io::Cursor;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;

use crate::backend::error_log::{write_to_error_log, ErrorType};
use crate::backend::fetch::MangadexClient;
use crate::backend::tui::Events;
use crate::backend::SearchMangaResponse;
use crate::global::INSTRUCTIONS_STYLE;
use crate::utils::search_manga_cover;
use crate::view::widgets::home::{
    CarrouselItem, CarrouselState, PopularMangaCarrousel, RecentlyAddedCarrousel,
};
use crate::view::widgets::search::MangaItem;
use crate::view::widgets::{Component, ImageHandler};
use crate::PICKER;

use self::text::ToSpan;

#[derive(PartialEq, Eq)]
pub enum HomeState {
    Unused,
}

pub enum HomeEvents {
    SearchPopularNewMangas,
    SearchPopularMangasCover,
    SearchRecentlyAddedMangas,
    SearchRecentlyCover,
    SearchSupportImage,
    LoadSupportImage(Option<DynamicImage>),
    LoadPopularMangas(Option<SearchMangaResponse>),
    LoadRecentlyAddedMangas(Option<SearchMangaResponse>),
    LoadCover(Option<Box<dyn StatefulProtocol>>, String),
    LoadRecentlyAddedMangasCover(Option<Box<dyn StatefulProtocol>>, String),
}

impl ImageHandler for HomeEvents {
    fn load(image: Box<dyn StatefulProtocol>, id: String) -> Self {
        Self::LoadRecentlyAddedMangasCover(Some(image), id)
    }
    fn not_found(id: String) -> Self {
        Self::LoadRecentlyAddedMangasCover(None, id)
    }
}

pub enum HomeActions {
    SelectNextPopularManga,
    SelectPreviousPopularManga,
    GoToPopularMangaPage,
    GoToRecentlyAddedMangaPage,
    SelectNextRecentlyAddedManga,
    SelectPreviousRecentlyAddedManga,
    SupportMangadex,
    SupportProject,
}

pub struct Home {
    carrousel_popular_mangas: PopularMangaCarrousel,
    carrousel_recently_added: RecentlyAddedCarrousel,
    state: HomeState,
    pub global_event_tx: UnboundedSender<Events>,
    pub local_action_tx: UnboundedSender<HomeActions>,
    pub local_action_rx: UnboundedReceiver<HomeActions>,
    pub local_event_tx: UnboundedSender<HomeEvents>,
    pub local_event_rx: UnboundedReceiver<HomeEvents>,
    pub support_image: Option<Box<dyn StatefulProtocol>>,
    tasks: JoinSet<()>,
}

impl Component for Home {
    type Actions = HomeActions;
    fn render(&mut self, area: ratatui::prelude::Rect, frame: &mut Frame<'_>) {
        let layout =
            Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).margin(1);
        let buf = frame.buffer_mut();

        let [carrousel_popular_mangas_area, latest_updates_area] = layout.areas(area);

        self.render_popular_mangas_carrousel(carrousel_popular_mangas_area, buf);

        self.render_recently_added_mangas_area(latest_updates_area, buf);
    }

    fn update(&mut self, action: Self::Actions) {
        match action {
            HomeActions::SelectNextPopularManga => {
                self.carrousel_popular_mangas.next_item();
            }
            HomeActions::SelectPreviousPopularManga => {
                self.carrousel_popular_mangas.previous_item()
            }
            HomeActions::GoToPopularMangaPage => self.go_to_manga_page(),
            HomeActions::SelectNextRecentlyAddedManga => {
                self.carrousel_recently_added.select_next()
            }
            HomeActions::SelectPreviousRecentlyAddedManga => {
                self.carrousel_recently_added.select_previous()
            }
            HomeActions::GoToRecentlyAddedMangaPage => {
                if let Some(item) = self.carrousel_recently_added.get_current_selected_manga() {
                    self.global_event_tx
                        .send(Events::GoToMangaPage(MangaItem::new(
                            item.manga.clone(),
                            item.cover_state.clone(),
                        )))
                        .ok();
                }
            }
            HomeActions::SupportProject => self.support_project(),
            HomeActions::SupportMangadex => self.support_mangadex(),
        }
    }

    fn clean_up(&mut self) {
        self.tasks.abort_all();
        self.carrousel_popular_mangas.items = vec![];
        self.carrousel_recently_added.items = vec![];
        self.support_image = None;
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
            carrousel_popular_mangas: PopularMangaCarrousel::default(),
            carrousel_recently_added: RecentlyAddedCarrousel::default(),
            state: HomeState::Unused,
            global_event_tx: tx,
            local_event_tx,
            local_event_rx,
            local_action_tx,
            local_action_rx,
            support_image: None,
            tasks: JoinSet::new(),
        }
    }
    pub fn render_popular_mangas_carrousel(&mut self, area: Rect, buf: &mut Buffer) {
        let inner = area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });

        let instructions = Line::from(vec![
            "Next ".into(),
            Span::raw("<w>").style(*INSTRUCTIONS_STYLE),
            " previous ".into(),
            Span::raw("<b>").style(*INSTRUCTIONS_STYLE),
            " read ".into(),
            Span::raw("<r>").style(*INSTRUCTIONS_STYLE),
            format!(
                " No.{} Total : {}",
                self.carrousel_popular_mangas.current_item_visible_index,
                self.carrousel_popular_mangas.items.len()
            )
            .into(),
        ]);

        // let instructions = Span::from(format!(
        //     "Next  <w> | previous  <b> | read <r>  No.{}  Total : {}",
        //     self.carrousel_popular_mangas.current_item_visible_index,
        //     self.carrousel_popular_mangas.items.len()
        // ))
        // .into_left_aligned_line();

        Block::bordered()
            .title(Line::from(vec!["Popular new titles".bold()]))
            .title_bottom(instructions)
            .render(area, buf);

        StatefulWidget::render(
            self.carrousel_popular_mangas.clone(),
            inner,
            buf,
            &mut self.carrousel_popular_mangas.current_item_visible_index,
        );
    }

    pub fn go_to_manga_page(&self) {
        if let Some(item) = self.get_current_popular_manga() {
            self.global_event_tx
                .send(Events::GoToMangaPage(MangaItem::new(
                    item.manga.clone(),
                    item.cover_state.clone(),
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
        self.local_event_tx
            .send(HomeEvents::SearchPopularNewMangas)
            .ok();

        self.local_event_tx
            .send(HomeEvents::SearchRecentlyAddedMangas)
            .ok();
        if PICKER.is_some() {
            self.local_event_tx
                .send(HomeEvents::SearchSupportImage)
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
                HomeEvents::SearchSupportImage => {
                    let tx = self.local_event_tx.clone();
                    self.tasks.spawn(async move {
                        let response = MangadexClient::global().get_mangadex_image_support().await;
                        match response {
                            Ok(bytes) => {
                                let dyn_img = Reader::new(Cursor::new(bytes))
                                    .with_guessed_format()
                                    .unwrap();

                                let maybe_decoded = dyn_img.decode();
                                match maybe_decoded {
                                    Ok(image) => {
                                        tx.send(HomeEvents::LoadSupportImage(Some(image))).ok();
                                    }
                                    Err(_) => {
                                        tx.send(HomeEvents::LoadSupportImage(None)).ok();
                                    }
                                };
                            }
                            Err(_) => {
                                tx.send(HomeEvents::LoadSupportImage(None)).ok();
                            }
                        }
                    });
                }
                HomeEvents::LoadSupportImage(maybe_image) => {
                    if let Some(image) = maybe_image {
                        let protocol = PICKER.unwrap().new_resize_protocol(image);
                        self.support_image = Some(protocol);
                    }
                }
            }
        }
    }

    fn load_popular_mangas(&mut self, maybe_response: Option<SearchMangaResponse>) {
        match maybe_response {
            Some(response) => {
                self.carrousel_popular_mangas = PopularMangaCarrousel::from_response(response);

                if PICKER.is_some() {
                    self.local_event_tx
                        .send(HomeEvents::SearchPopularMangasCover)
                        .ok();
                }
            }
            None => {
                self.carrousel_popular_mangas.state = CarrouselState::NotFound;
            }
        }
    }

    fn load_popular_manga_cover(
        &mut self,
        maybe_cover: Option<Box<dyn StatefulProtocol>>,
        id: String,
    ) {
        match maybe_cover {
            Some(cover) => {
                if let Some(popular_manga) = self
                    .carrousel_popular_mangas
                    .items
                    .iter_mut()
                    .find(|manga_item| manga_item.manga.id == id)
                {
                    popular_manga.cover_state = Some(cover);
                }
            }
            None => {
                // Todo! image could not be rendered
            }
        }
    }

    fn search_popular_mangas(&mut self) {
        let tx = self.local_event_tx.clone();
        self.tasks.spawn(async move {
            let response = MangadexClient::global().get_popular_mangas().await;
            match response {
                Ok(mangas) => {
                    if mangas.data.is_empty() {
                        tx.send(HomeEvents::LoadPopularMangas(None)).ok();
                    } else {
                        tx.send(HomeEvents::LoadPopularMangas(Some(mangas))).ok();
                    }
                }
                Err(e) => {
                    write_to_error_log(ErrorType::FromError(Box::new(e)));
                    tx.send(HomeEvents::LoadPopularMangas(None)).ok();
                }
            }
        });
    }

    fn search_popular_mangas_cover(&mut self) {
        for item in self.carrousel_popular_mangas.items.iter() {
            let manga_id = item.manga.id.clone();
            let tx = self.local_event_tx.clone();
            match item.manga.img_url.as_ref() {
                Some(file_name) => {
                    let file_name = file_name.clone();
                    self.tasks.spawn(async move {
                        let response = MangadexClient::global()
                            .get_cover_for_manga(&manga_id, &file_name)
                            .await;

                        if let Ok(bytes) = response {
                            let dyn_img = Reader::new(Cursor::new(bytes))
                                .with_guessed_format()
                                .unwrap();

                            let maybe_decoded = dyn_img.decode();

                            if let Ok(decoded) = maybe_decoded {
                                let protocol = PICKER.unwrap().new_resize_protocol(decoded);
                                tx.send(HomeEvents::LoadCover(Some(protocol), manga_id))
                                    .ok();
                            }
                        }
                    });
                }
                None => {
                    tx.send(HomeEvents::LoadCover(None, manga_id)).ok();
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
                    write_to_error_log(ErrorType::FromError(Box::new(e)));
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
                self.carrousel_recently_added.state = CarrouselState::NotFound;
            }
        }
    }

    fn search_recently_added_mangas_cover(&mut self) {
        for item in self.carrousel_recently_added.items.iter() {
            let manga_id = item.manga.id.clone();
            let tx = self.local_event_tx.clone();
            match item.manga.img_url.as_ref() {
                Some(file_name) => {
                    let file_name = file_name.clone();

                    search_manga_cover(file_name, manga_id, &mut self.tasks, tx);
                }
                None => {
                    tx.send(HomeEvents::LoadRecentlyAddedMangasCover(None, manga_id))
                        .ok();
                }
            };
        }
    }

    fn load_recently_added_mangas_cover(
        &mut self,
        maybe_cover: Option<Box<dyn StatefulProtocol>>,
        id: String,
    ) {
        match maybe_cover {
            Some(cover) => {
                if let Some(recently_added_manga) = self
                    .carrousel_recently_added
                    .items
                    .iter_mut()
                    .find(|manga_item| manga_item.manga.id == id)
                {
                    recently_added_manga.cover_state = Some(cover);
                }
            }
            None => {
                // Todo! image could not be rendered
            }
        }
    }

    fn support_mangadex(&mut self) {
        open::that("https://namicomi.com/en/org/3Hb7HnWG/mangadex/subscriptions").ok();
    }

    fn support_project(&mut self) {
        open::that("https://github.com/josueBarretogit/manga-tui").ok();
    }

    fn render_recently_added_mangas_area(&mut self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)]);
        let [app_info_area, recently_added_mangas_area] = layout.areas(area);

        self.render_app_information(app_info_area, buf);

        let inner_area = recently_added_mangas_area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });

        let instructions = Line::from(vec![
            "Recently added mangas | ".into(),
            "Move right ".into(),
            Span::raw("<l>").style(*INSTRUCTIONS_STYLE),
            " Move left ".into(),
            Span::raw(" <h> ").style(*INSTRUCTIONS_STYLE),
            " Read ".into(),
            Span::raw("<Enter>").style(*INSTRUCTIONS_STYLE),
        ]);

        Block::bordered()
            .title(instructions)
            .render(recently_added_mangas_area, buf);

        StatefulWidget::render(
            self.carrousel_recently_added.clone(),
            inner_area,
            buf,
            &mut self.carrousel_recently_added.selected_item_index,
        );
    }

    fn render_app_information(&mut self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)])
            .margin(1)
            .split(area);

        Block::bordered()
            .title(format!("Manga-tui V{}", env!("CARGO_PKG_VERSION")))
            .render(area, buf);

        if let Some(protocol) = self.support_image.as_mut() {
            let image = StatefulImage::new(None).resize(Resize::Fit(None));
            StatefulWidget::render(image, layout[0], buf, protocol);
        }

        Widget::render(
            List::new([
                Line::from(vec![
                    "Support mangadex: ".into(),
                    "<m>".to_span().style(*INSTRUCTIONS_STYLE),
                ]),
                Line::from(vec![
                    "Support this project".into(),
                    "<g>".to_span().style(*INSTRUCTIONS_STYLE),
                ]),
            ]),
            layout[1],
            buf,
        )
    }

    pub fn handle_key_events(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('w') => {
                self.local_action_tx
                    .send(HomeActions::SelectNextPopularManga)
                    .ok();
            }

            KeyCode::Char('b') => {
                self.local_action_tx
                    .send(HomeActions::SelectPreviousPopularManga)
                    .ok();
            }
            KeyCode::Char('r') => {
                self.local_action_tx
                    .send(HomeActions::GoToPopularMangaPage)
                    .ok();
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.local_action_tx
                    .send(HomeActions::SelectNextRecentlyAddedManga)
                    .ok();
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.local_action_tx
                    .send(HomeActions::SelectPreviousRecentlyAddedManga)
                    .ok();
            }
            KeyCode::Enter => {
                self.local_action_tx
                    .send(HomeActions::GoToRecentlyAddedMangaPage)
                    .ok();
            }
            KeyCode::Char('m') => {
                self.local_action_tx.send(HomeActions::SupportMangadex).ok();
            }
            KeyCode::Char('g') => {
                self.local_action_tx.send(HomeActions::SupportProject).ok();
            }
            _ => {}
        }
    }
}
