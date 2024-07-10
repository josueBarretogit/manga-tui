use crossterm::event::KeyEvent;
use ratatui::{prelude::*, widgets::*};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;

use crate::backend::fetch::MangadexClient;
use crate::backend::tui::Events;
use crate::backend::SearchMangaResponse;
use crate::view::widgets::home::Carrousel;
use crate::view::widgets::Component;

pub enum HomeEvents {
    SearchPopularNewMangas,
    LoadPopularMangas(Option<SearchMangaResponse>),
}

pub enum HomeActions {
    SelectNextPopularManga,
    SelectPreviousPopularManga,
}

pub struct Home {
    pub global_event_tx: UnboundedSender<Events>,
    carrousel: Carrousel,
    pub local_action_tx: UnboundedSender<HomeActions>,
    pub local_action_rx: UnboundedReceiver<HomeActions>,
    pub local_event_tx: UnboundedSender<HomeEvents>,
    pub local_event_rx: UnboundedReceiver<HomeEvents>,
    taks: JoinSet<()>,
}

impl Component for Home {
    type Actions = HomeActions;
    fn render(&mut self, area: ratatui::prelude::Rect, frame: &mut Frame<'_>) {
        let layout =
            Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).margin(1);
        let buf = frame.buffer_mut();

        let [carrousel_popular_mangas_area] = layout.areas(area);
        self.render_carrousel(carrousel_popular_mangas_area, buf);
    }

    fn update(&mut self, action: Self::Actions) {}

    fn clean_up(&mut self) {}

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
            global_event_tx: tx,
            local_event_tx,
            local_event_rx,
            local_action_tx,
            local_action_rx,
            taks: JoinSet::new(),
        }
    }
    pub fn render_carrousel(&mut self, area: Rect, buf: &mut Buffer) {
        StatefulWidget::render(self.carrousel.clone(), area, buf, &mut self.carrousel.state);
    }

    pub fn go_to_manga_page(&mut self) {}

    pub fn tick(&mut self) {
        if let Ok(local_event) = self.local_event_rx.try_recv() {
            match local_event {
                HomeEvents::SearchPopularNewMangas => self.search_popular_mangas(),
                HomeEvents::LoadPopularMangas(maybe_response) => {
                    self.load_popular_mangas(maybe_response);
                }
            }
        }
    }

    fn load_popular_mangas(&mut self, maybe_response: Option<SearchMangaResponse>) {}

    fn search_popular_mangas(&mut self) {
        let tx = self.local_event_tx.clone();
        self.taks.spawn(async move {
            let response = MangadexClient::global().get_popular_mangas().await;
            match response {
                Ok(mangas) => {
                    tx.send(HomeEvents::LoadPopularMangas(Some(mangas))).ok();
                }
                Err(_) => {
                    tx.send(HomeEvents::LoadPopularMangas(None)).ok();
                }
            }
        });
    }

    pub fn handle_key_events(&mut self, key_event: KeyEvent) {}
}
