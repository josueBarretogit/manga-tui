use crossterm::event::{KeyCode, KeyEvent};
use image::io::Reader;
use ratatui::{prelude::*, widgets::*};
use std::io::Cursor;
use throbber_widgets_tui::ThrobberState;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;

use crate::backend::database::{get_reading_history, MangaHistory};
use crate::backend::fetch::MangadexClient;
use crate::backend::tui::Events;
use crate::utils::from_manga_response;
use crate::view::widgets::feed::{FeedTabs, HistoryWidget, MangasRead};
use crate::view::widgets::search::MangaItem;
use crate::view::widgets::Component;
use crate::PICKER;

pub enum FeedActions {
    ScrollHistoryUp,
    ScrollHistoryDown,
    NextPage,
    PreviousPage,
    ChangeTab,
    GoToMangaPage,
}

pub enum FeedEvents {
    SearchHistory,
    SearchRecentChapters,
    LoadHistory(u32, Option<(Vec<MangaHistory>, u32)>),
}

pub struct Feed {
    pub tabs: FeedTabs,
    pub history: Option<HistoryWidget>,
    pub loading_state: Option<ThrobberState>,
    pub global_event_tx: UnboundedSender<Events>,
    pub local_action_tx: UnboundedSender<FeedActions>,
    pub local_action_rx: UnboundedReceiver<FeedActions>,
    pub local_event_tx: UnboundedSender<FeedEvents>,
    pub local_event_rx: UnboundedReceiver<FeedEvents>,
    tasks: JoinSet<()>,
}

impl Feed {
    pub fn new(global_event_tx: UnboundedSender<Events>) -> Self {
        let (local_action_tx, local_action_rx) = mpsc::unbounded_channel::<FeedActions>();
        let (local_event_tx, local_event_rx) = mpsc::unbounded_channel::<FeedEvents>();
        Self {
            tabs: FeedTabs::History,
            loading_state: None,
            history: None,
            global_event_tx,
            local_action_tx,
            local_action_rx,
            local_event_tx,
            local_event_rx,
            tasks: JoinSet::new(),
        }
    }

    fn render_history(&mut self, area: Rect, buf: &mut Buffer) {
        match self.history.as_mut() {
            Some(history) => StatefulWidget::render(history.clone(), area, buf, &mut history.state),
            None => {
                Paragraph::new("You have not read any mangas yet").render(area, buf);
            }
        }
    }

    pub fn init_search(&mut self) {
        self.local_event_tx.send(FeedEvents::SearchHistory).ok();
    }

    fn render_plan_to_read(&mut self, area: Rect, buf: &mut Buffer) {}

    fn handle_key_events(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.local_action_tx
                    .send(FeedActions::ScrollHistoryDown)
                    .ok();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.local_action_tx.send(FeedActions::ScrollHistoryUp).ok();
            }
            KeyCode::Char('w') => {
                self.local_action_tx.send(FeedActions::NextPage).ok();
            }

            KeyCode::Char('b') => {
                self.local_action_tx.send(FeedActions::PreviousPage).ok();
            }
            KeyCode::Char('r') => {
                self.local_action_tx.send(FeedActions::GoToMangaPage).ok();
            }
            _ => {}
        }
    }

    pub fn tick(&mut self) {
        if let Ok(local_event) = self.local_event_rx.try_recv() {
            match local_event {
                FeedEvents::SearchHistory => self.search_history(),
                FeedEvents::LoadHistory(page, maybe_history) => {
                    self.load_history(page, maybe_history)
                }
                FeedEvents::SearchRecentChapters => todo!(),
            }
        }
    }

    fn search_history(&mut self) {
        let tx = self.local_event_tx.clone();

        let page = match &self.history {
            Some(history) => history.page,
            None => 1,
        };

        self.tasks.spawn(async move {
            let maybe_reading_history = get_reading_history(page);
            tx.send(FeedEvents::LoadHistory(page, maybe_reading_history.ok()))
                .ok();
        });
    }

    fn search_next_page(&mut self) {
        if let Some(history) = self.history.as_mut() {
            history.next_page();
            self.search_history();
        }
    }

    fn search_previous_page(&mut self) {
        if let Some(history) = self.history.as_mut() {
            history.previous_page();
            self.search_history();
        }
    }

    fn load_history(&mut self, page: u32, maybe_history: Option<(Vec<MangaHistory>, u32)>) {
        if let Some(history) = maybe_history {
            self.history = Some(HistoryWidget {
                page,
                total_results: history.1,
                mangas: history
                    .0
                    .iter()
                    .map(|history| MangasRead {
                        id: history.id.clone(),
                        title: history.title.clone(),
                        recent_chapters: None,
                        style: Style::default(),
                    })
                    .collect(),
                state: tui_widget_list::ListState::default(),
            });
        }
    }

    fn select_next_manga(&mut self) {
        if let Some(mangas) = self.history.as_mut() {
            mangas.select_next();
        }
    }

    fn select_previous_manga(&mut self) {
        if let Some(mangas) = self.history.as_mut() {
            mangas.select_previous();
        }
    }

    fn change_tab(&mut self) {
        match self.tabs {
            FeedTabs::History => self.tabs = FeedTabs::PlantToRead,
            FeedTabs::PlantToRead => self.tabs = FeedTabs::History,
        }
    }

    fn search_plan_to_read(&mut self) {
        todo!()
    }

    fn go_to_manga_page(&mut self) {
        if let Some(history) = self.history.as_mut() {
            if let Some(currently_selected_manga) = history.get_current_manga_selected() {
                let tx = self.global_event_tx.clone();
                let manga_id = currently_selected_manga.id.clone();

                self.tasks.spawn(async move {
                    let response = MangadexClient::global().get_one_manga(&manga_id).await;
                    match response {
                        Ok(manga) => {
                            let manga_found = from_manga_response(manga.data);

                            if PICKER.is_some() {
                                let cover = MangadexClient::global()
                                    .get_cover_for_manga(
                                        &manga_id,
                                        manga_found.img_url.unwrap_or_default().as_str(),
                                    )
                                    .await;

                                match cover {
                                    Ok(bytes) => {
                                        let dyn_img = Reader::new(Cursor::new(bytes))
                                            .with_guessed_format()
                                            .unwrap();

                                        let maybe_decoded = dyn_img.decode().unwrap();
                                        let image =
                                            PICKER.unwrap().new_resize_protocol(maybe_decoded);

                                        tx.send(Events::GoToMangaPage(MangaItem::new(
                                            manga_found.id,
                                            manga_found.title,
                                            manga_found.description,
                                            manga_found.tags,
                                            manga_found.content_rating,
                                            manga_found.status,
                                            None,
                                            manga_found.author,
                                            manga_found.artist,
                                            Some(image),
                                        )))
                                        .ok();
                                    }
                                    Err(_) => {
                                        tx.send(Events::GoToMangaPage(MangaItem::new(
                                            manga_found.id,
                                            manga_found.title,
                                            manga_found.description,
                                            manga_found.tags,
                                            manga_found.content_rating,
                                            manga_found.status,
                                            None,
                                            manga_found.author,
                                            manga_found.artist,
                                            None,
                                        )))
                                        .ok();
                                    }
                                }
                            } else {
                                tx.send(Events::GoToMangaPage(MangaItem::new(
                                    manga_found.id,
                                    manga_found.title,
                                    manga_found.description,
                                    manga_found.tags,
                                    manga_found.content_rating,
                                    manga_found.status,
                                    None,
                                    manga_found.author,
                                    manga_found.artist,
                                    None,
                                )))
                                .ok();
                            }
                        }
                        Err(e) => println!("Could not get manga info : {e}"),
                    }
                });
            }
        }
    }
}

impl Component for Feed {
    type Actions = FeedActions;
    fn render(&mut self, area: ratatui::prelude::Rect, frame: &mut Frame<'_>) {
        let buf = frame.buffer_mut();
        let layout = Layout::vertical([Constraint::Percentage(10), Constraint::Percentage(90)]);

        let [tabs_area, history_area] = layout.areas(area);

        let selected_tab = match self.tabs {
            FeedTabs::History => 0,
            FeedTabs::PlantToRead => 1,
        };

        Tabs::new(vec!["History", "Plan to Read"])
            .select(selected_tab)
            .render(tabs_area, buf);

        match self.tabs {
            FeedTabs::History => self.render_history(history_area, buf),
            FeedTabs::PlantToRead => todo!(),
        }
    }

    fn update(&mut self, action: Self::Actions) {
        match action {
            FeedActions::NextPage => self.search_next_page(),
            FeedActions::PreviousPage => self.search_previous_page(),
            FeedActions::GoToMangaPage => self.go_to_manga_page(),
            FeedActions::ScrollHistoryUp => self.select_previous_manga(),
            FeedActions::ScrollHistoryDown => self.select_next_manga(),
            FeedActions::ChangeTab => self.change_tab(),
        }
    }

    fn clean_up(&mut self) {
        self.history = None;
    }

    fn handle_events(&mut self, events: crate::backend::tui::Events) {
        match events {
            Events::Key(key_event) => {
                self.handle_key_events(key_event);
            }
            Events::Tick => self.tick(),
            _ => {}
        }
    }
}
