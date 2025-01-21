use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent};
use image::DynamicImage;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, ToSpan};
use ratatui::widgets::{Block, List, StatefulWidget, Widget};
use ratatui::Frame;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::Protocol;
use ratatui_image::{Image, Resize};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;

use crate::backend::error_log::{write_to_error_log, ErrorType};
use crate::backend::manga_provider::{HomePageMangaProvider, PopularManga, RecentlyAddedManga};
use crate::backend::tui::Events;
use crate::common::ImageState;
use crate::global::INSTRUCTIONS_STYLE;
use crate::view::widgets::home::{CarrouselItemPopularManga, CarrouselState, PopularMangaCarrousel, RecentlyAddedCarrousel};
use crate::view::widgets::Component;

#[derive(PartialEq, Eq)]
pub enum HomeState {
    Unused,
}

#[derive(Debug, PartialEq)]
pub enum HomeEvents {
    SearchPopularNewMangas,
    SearchPopularMangasCover,
    SearchRecentlyAddedMangas,
    SearchRecentlyCover,
    SearchSupportImage,
    LoadSupportImage(DynamicImage),
    LoadPopularMangas(Option<Vec<PopularManga>>),
    LoadRecentlyAddedMangas(Option<Vec<RecentlyAddedManga>>),
    LoadCover(Option<DynamicImage>, String),
    LoadRecentlyAddedMangasCover(Option<DynamicImage>, String),
}

#[derive(Debug, PartialEq, Eq)]
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

pub struct Home<T>
where
    T: HomePageMangaProvider,
{
    carrousel_popular_mangas: PopularMangaCarrousel,
    carrousel_recently_added: RecentlyAddedCarrousel,
    state: HomeState,
    pub global_event_tx: Option<UnboundedSender<Events>>,
    pub local_action_tx: UnboundedSender<HomeActions>,
    pub local_action_rx: UnboundedReceiver<HomeActions>,
    pub local_event_tx: UnboundedSender<HomeEvents>,
    pub local_event_rx: UnboundedReceiver<HomeEvents>,
    pub support_image: Option<Box<dyn Protocol>>,
    image_support_area: Rect,
    popular_manga_carrousel_state: ImageState,
    recently_added_manga_state: ImageState,
    picker: Option<Picker>,
    tasks: JoinSet<()>,
    manga_provider: T,
}

impl<T> Component for Home<T>
where
    T: HomePageMangaProvider,
{
    type Actions = HomeActions;

    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) {
        let layout = Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).margin(1);
        let buf = frame.buffer_mut();

        let [carrousel_popular_mangas_area, latest_updates_area] = layout.areas(area);

        self.render_popular_mangas_carrousel(carrousel_popular_mangas_area, buf);

        self.render_recently_added_mangas_area(latest_updates_area, buf);
    }

    fn update(&mut self, action: Self::Actions) {
        match action {
            HomeActions::SelectNextPopularManga => {
                self.carrousel_popular_mangas.next_item();
            },
            HomeActions::SelectPreviousPopularManga => self.carrousel_popular_mangas.previous_item(),
            HomeActions::GoToPopularMangaPage => self.go_to_manga_page_popular(),
            HomeActions::SelectNextRecentlyAddedManga => self.carrousel_recently_added.select_next(),
            HomeActions::SelectPreviousRecentlyAddedManga => self.carrousel_recently_added.select_previous(),
            HomeActions::GoToRecentlyAddedMangaPage => {
                //if let Some(item) = self.carrousel_recently_added.get_current_selected_manga() {
                //    self.global_event_tx
                //        .as_mut()
                //        .unwrap()
                //        .send(Events::GoToMangaPage(MangaItem::new(item.manga.clone())))
                //        .ok();
                //}
            },
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
        self.recently_added_manga_state = ImageState::default();
        self.popular_manga_carrousel_state = ImageState::default();
    }

    fn handle_events(&mut self, events: Events) {
        match events {
            Events::Key(key_event) => self.handle_key_events(key_event),
            Events::Tick => self.tick(),
            _ => {},
        }
    }
}

impl<T> Home<T>
where
    T: HomePageMangaProvider,
{
    pub fn new(picker: Option<Picker>, provider: T) -> Self {
        let (local_action_tx, local_action_rx) = mpsc::unbounded_channel::<HomeActions>();
        let (local_event_tx, local_event_rx) = mpsc::unbounded_channel::<HomeEvents>();

        Self {
            carrousel_popular_mangas: PopularMangaCarrousel::default(),
            carrousel_recently_added: RecentlyAddedCarrousel::new(picker.is_some()),
            state: HomeState::Unused,
            global_event_tx: None,
            local_event_tx,
            local_event_rx,
            local_action_tx,
            local_action_rx,
            support_image: None,
            image_support_area: Rect::default(),
            picker,
            popular_manga_carrousel_state: ImageState::default(),
            recently_added_manga_state: ImageState::default(),
            tasks: JoinSet::new(),
            manga_provider: provider,
        }
    }

    pub fn with_global_sender(mut self, tx: UnboundedSender<Events>) -> Self {
        self.global_event_tx = Some(tx);
        self
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

        Block::bordered()
            .title(Line::from(vec!["Popular new titles".bold()]))
            .title_bottom(instructions)
            .render(area, buf);

        StatefulWidget::render(self.carrousel_popular_mangas.clone(), inner, buf, &mut self.popular_manga_carrousel_state);
    }

    pub fn go_to_manga_page_popular(&self) {
        //if let Some(item) = self.get_current_popular_manga() {
        //    self.global_event_tx
        //        .as_ref()
        //        .unwrap()
        //        .send(Events::GoToMangaPage(MangaItem::new(item.manga.clone())))
        //        .ok();
        //}
    }

    fn get_current_popular_manga(&self) -> Option<&CarrouselItemPopularManga> {
        self.carrousel_popular_mangas.get_current_item()
    }

    pub fn require_search(&mut self) -> bool {
        self.carrousel_popular_mangas.items.is_empty() || self.carrousel_recently_added.items.is_empty()
    }

    pub fn init_search(&mut self) {
        self.local_event_tx.send(HomeEvents::SearchPopularNewMangas).ok();

        self.local_event_tx.send(HomeEvents::SearchRecentlyAddedMangas).ok();
        if self.picker.is_some() {
            self.local_event_tx.send(HomeEvents::SearchSupportImage).ok();
        }
    }

    pub fn init_search_popular_mangas_cover(&self) {
        if self.picker.is_some() {
            self.local_event_tx.send(HomeEvents::SearchPopularMangasCover).ok();
        }
    }

    pub fn init_search_recently_added_mangas_cover(&self) {
        if self.picker.is_some() {
            self.local_event_tx.send(HomeEvents::SearchRecentlyCover).ok();
        }
    }

    fn search_support_image(&mut self) {
        //let tx = self.local_event_tx.clone();
        //self.tasks.spawn(async move {
        //    let response = MangadexClient::global().get_mangadex_image_support().await;
        //    if let Ok(bytes) = response {
        //        let dyn_img = Reader::new(Cursor::new(bytes)).with_guessed_format().unwrap();
        //
        //        let maybe_decoded = dyn_img.decode();
        //        if let Ok(image) = maybe_decoded {
        //            tx.send(HomeEvents::LoadSupportImage(image)).ok();
        //        }
        //    }
        //});
    }

    fn load_support_image(&mut self, img: DynamicImage) {
        if let Some(picker) = self.picker.as_mut() {
            if let Ok(protocol) = picker.new_protocol(img, self.image_support_area, Resize::Fit(None)) {
                self.support_image = Some(protocol);
            }
        }
    }

    pub fn tick(&mut self) {
        self.carrousel_popular_mangas.tick();
        self.carrousel_recently_added.tick();
        if let Ok(local_event) = self.local_event_rx.try_recv() {
            match local_event {
                HomeEvents::SearchPopularMangasCover => self.search_popular_mangas_cover(),
                HomeEvents::SearchPopularNewMangas => self.search_popular_mangas(),
                HomeEvents::LoadPopularMangas(maybe_response) => {
                    self.load_popular_mangas(maybe_response);
                },
                HomeEvents::LoadCover(maybe_cover, index) => self.load_popular_manga_cover(maybe_cover, index),
                HomeEvents::SearchRecentlyAddedMangas => {
                    self.search_recently_added_mangas();
                },
                HomeEvents::LoadRecentlyAddedMangas(maybe_response) => {
                    self.load_recently_added_mangas(maybe_response);
                },
                HomeEvents::SearchRecentlyCover => {
                    self.search_recently_added_mangas_cover();
                },
                HomeEvents::LoadRecentlyAddedMangasCover(maybe_image, id) => {
                    self.load_recently_added_mangas_cover(maybe_image, id);
                },
                HomeEvents::SearchSupportImage => self.search_support_image(),
                HomeEvents::LoadSupportImage(image) => self.load_support_image(image),
            }
        }
    }

    fn load_popular_mangas(&mut self, maybe_response: Option<Vec<PopularManga>>) {
        match maybe_response {
            Some(response) => {
                self.carrousel_popular_mangas = PopularMangaCarrousel::from_response(response, self.picker.is_some());
                self.init_search_popular_mangas_cover();
            },
            None => {
                self.carrousel_popular_mangas.state = CarrouselState::NotFound;
            },
        }
    }

    fn load_popular_manga_cover(&mut self, maybe_cover: Option<DynamicImage>, id: String) {
        if let Some(cover) = maybe_cover {
            if let Some(picker) = self.picker.as_mut() {
                let fixed_protocol =
                    picker.new_protocol(cover, self.popular_manga_carrousel_state.get_img_area(), Resize::Fit(None));
                if let Ok(protocol) = fixed_protocol {
                    self.popular_manga_carrousel_state.insert_manga(protocol, id);
                }
            }
        }
    }

    fn search_popular_mangas(&mut self) {
        let tx = self.local_event_tx.clone();
        self.carrousel_popular_mangas.state = CarrouselState::Searching;
        let manga_provider = self.manga_provider.clone();
        self.tasks.spawn(async move {
            let response = manga_provider.get_popular_mangas().await;
            match response {
                Ok(res) => {
                    if res.is_empty() {
                        tx.send(HomeEvents::LoadPopularMangas(None)).ok();
                    } else {
                        tx.send(HomeEvents::LoadPopularMangas(Some(res))).ok();
                    }
                },
                Err(e) => {
                    write_to_error_log(ErrorType::Error(e));
                    tx.send(HomeEvents::LoadPopularMangas(None)).ok();
                },
            }
        });
    }

    fn search_popular_mangas_cover(&mut self) {
        std::thread::sleep(Duration::from_millis(250));
        let mangas = self.carrousel_popular_mangas.items.clone();

        let tx = self.local_event_tx.clone();
        let client = self.manga_provider.clone();
        self.tasks.spawn(async move {
            for item in mangas {
                match item.manga.cover_img_url.as_ref() {
                    Some(url) => {
                        let response = client.get_manga_cover(url).await;
                        if let Ok(res) = response {
                            tx.send(HomeEvents::LoadCover(Some(res), item.manga.id)).ok();
                        }
                    },
                    None => {
                        tx.send(HomeEvents::LoadCover(None, item.manga.id)).ok();
                    },
                };
            }
        });
    }

    fn search_recently_added_mangas(&mut self) {
        let tx = self.local_event_tx.clone();
        self.carrousel_recently_added.state = CarrouselState::Searching;
        let client = self.manga_provider.clone();
        self.tasks.spawn(async move {
            let response = client.get_recently_added_mangas().await;
            match response {
                Ok(mangas) => {
                    tx.send(HomeEvents::LoadRecentlyAddedMangas(Some(mangas))).ok();
                },
                Err(e) => {
                    write_to_error_log(e.into());
                    tx.send(HomeEvents::LoadRecentlyAddedMangas(None)).ok();
                },
            }
        });
    }

    fn load_recently_added_mangas(&mut self, maybe_response: Option<Vec<RecentlyAddedManga>>) {
        match maybe_response {
            Some(response) => {
                self.carrousel_recently_added = RecentlyAddedCarrousel::from_response(response, self.picker.is_some());
                self.init_search_recently_added_mangas_cover();
            },
            None => {
                self.carrousel_recently_added.state = CarrouselState::NotFound;
            },
        }
    }

    fn search_recently_added_mangas_cover(&mut self) {
        std::thread::sleep(Duration::from_millis(250));

        let mangas = self.carrousel_recently_added.items.clone();
        let tx = self.local_event_tx.clone();
        let client = self.manga_provider.clone();
        self.tasks.spawn(async move {
            for item in mangas {
                match item.manga.cover_img_url.as_ref() {
                    Some(file_name) => {
                        let response = client.get_manga_cover_lower_quality(&file_name).await;
                        if let Ok(res) = response {
                            tx.send(HomeEvents::LoadRecentlyAddedMangasCover(Some(res), item.manga.id)).ok();
                        } else {
                            tx.send(HomeEvents::LoadRecentlyAddedMangasCover(None, item.manga.id)).ok();
                        }
                    },
                    None => {
                        tx.send(HomeEvents::LoadRecentlyAddedMangasCover(None, item.manga.id)).ok();
                    },
                };
            }
        });
    }

    fn load_recently_added_mangas_cover(&mut self, maybe_cover: Option<DynamicImage>, id: String) {
        if let Some(cover) = maybe_cover {
            if let Some(picker) = self.picker.as_mut() {
                let fixed_protocol = picker.new_protocol(cover, self.recently_added_manga_state.get_img_area(), Resize::Fit(None));

                if let Ok(protocol) = fixed_protocol {
                    self.recently_added_manga_state.insert_manga(protocol, id);
                }
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

        Block::bordered().title(instructions).render(recently_added_mangas_area, buf);

        StatefulWidget::render(self.carrousel_recently_added.clone(), inner_area, buf, &mut self.recently_added_manga_state);
    }

    fn render_app_information(&mut self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).margin(1).split(area);

        Block::bordered()
            .title(format!("Manga-tui V{}", env!("CARGO_PKG_VERSION")))
            .render(area, buf);

        match self.support_image.as_ref() {
            Some(image) => {
                let image = Image::new(image.as_ref());
                Widget::render(image, layout[0], buf);
            },
            None => {
                self.image_support_area = layout[0];
            },
        }

        Widget::render(
            List::new([
                Line::from(vec!["Support mangadex: ".into(), "<m>".to_span().style(*INSTRUCTIONS_STYLE)]),
                Line::from(vec!["Support this project ".into(), "<g>".to_span().style(*INSTRUCTIONS_STYLE)]),
            ]),
            layout[1],
            buf,
        )
    }

    pub fn handle_key_events(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('w') => {
                self.local_action_tx.send(HomeActions::SelectNextPopularManga).ok();
            },

            KeyCode::Char('b') => {
                self.local_action_tx.send(HomeActions::SelectPreviousPopularManga).ok();
            },
            KeyCode::Char('r') => {
                self.local_action_tx.send(HomeActions::GoToPopularMangaPage).ok();
            },
            KeyCode::Char('l') | KeyCode::Right => {
                self.local_action_tx.send(HomeActions::SelectNextRecentlyAddedManga).ok();
            },
            KeyCode::Char('h') | KeyCode::Left => {
                self.local_action_tx.send(HomeActions::SelectPreviousRecentlyAddedManga).ok();
            },
            KeyCode::Enter => {
                self.local_action_tx.send(HomeActions::GoToRecentlyAddedMangaPage).ok();
            },
            KeyCode::Char('m') => {
                self.local_action_tx.send(HomeActions::SupportMangadex).ok();
            },
            KeyCode::Char('g') => {
                self.local_action_tx.send(HomeActions::SupportProject).ok();
            },
            _ => {},
        }
    }
}

#[cfg(test)]
mod tests {
    //use pretty_assertions::assert_eq;
    //
    //use super::*;
    //use crate::backend::api_responses::Data;
    //use crate::backend::manga_provider::mock::MockMangaPageProvider;
    //
    //#[test]
    //fn searches_popular_manga_cover_after_mangas_are_loaded_if_picker_is_some() {
    //    let mut home: Home<MockMangaPageProvider> = Home::new(Some(Picker::new((8, 8))), MockMangaPageProvider::new());
    //
    //    home.load_popular_mangas(Some(vec![PopularManga::default()]));
    //
    //    let event = home.local_event_rx.blocking_recv().expect("no event was");
    //
    //    assert_eq!(event, HomeEvents::SearchPopularMangasCover)
    //}
    //#[test]
    //fn searches_recently_added_manga_cover_after_mangas_are_loaded_if_picker_is_some() {
    //    let mut home: Home<MockMangaPageProvider> = Home::new(Some(Picker::new((8, 8))), MockMangaPageProvider::new());
    //
    //    home.load_recently_added_mangas(Some(SearchMangaResponse {
    //        data: vec![Data::default()],
    //        ..Default::default()
    //    }));
    //
    //    let event = home.local_event_rx.blocking_recv().expect("no event was");
    //
    //    assert_eq!(event, HomeEvents::SearchRecentlyCover)
    //}
    //
    //#[test]
    //fn doesnt_search_manga_cover_if_picker_is_none() {
    //    let mut home: Home<MockMangaPageProvider> = Home::new(Some(Picker::new((8, 8))), MockMangaPageProvider::new());
    //
    //    home.load_popular_mangas(Some(vec![PopularManga::default()]));
    //
    //    assert!(home.local_event_rx.is_empty());
    //
    //    home.load_recently_added_mangas(Some(SearchMangaResponse {
    //        data: vec![Data::default()],
    //        ..Default::default()
    //    }));
    //
    //    assert!(home.local_event_rx.is_empty());
    //}
}
