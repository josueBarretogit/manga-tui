use std::sync::Arc;

use bytes::Bytes;
use crossterm::event::KeyCode;
use image::io::Reader;
use image::DynamicImage;
use ratatui::{prelude::*, widgets::*};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::Resize;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;

use crate::backend::fetch::MangadexClient;
use crate::backend::tui::Events;
use crate::view::widgets::{Component, ThreadImage, ThreadProtocol};

pub enum MangaReaderActions {
    NextPage,
    PreviousPage,
}

pub enum MangaReaderEvents {
    FetchPages,
    DecodeImage(Option<Bytes>, usize),
    LoadPage(Option<DynamicImage>, usize),
    Redraw(Box<dyn StatefulProtocol>, usize),
}

pub struct Page {
    pub image_state: Option<ThreadProtocol>,
    pub url: String,
    pub data_save_url: String,
}

pub struct MangaReader {
    pub chapter_id: String,
    pub base_url: String,
    pub pages: Vec<Page>,
    pub current_page_index: usize,
    /// Handle fetching the images
    pub image_tasks: JoinSet<()>,
    pub picker: Picker,
    pub client: Arc<MangadexClient>,
    pub local_action_tx: UnboundedSender<MangaReaderActions>,
    pub local_action_rx: UnboundedReceiver<MangaReaderActions>,
    pub local_event_tx: UnboundedSender<MangaReaderEvents>,
    pub local_event_rx: UnboundedReceiver<MangaReaderEvents>,
}

impl Component for MangaReader {
    type Actions = MangaReaderActions;
    fn render(&mut self, area: ratatui::prelude::Rect, frame: &mut Frame<'_>) {
        match self.pages.get_mut(self.current_page_index) {
            Some(page) => {
                if let Some(img_state) = page.image_state.as_mut() {
                    let image = ThreadImage::new().resize(Resize::Fit(None));
                    StatefulWidget::render(image, area, frame.buffer_mut(), img_state);
                }
            }
            None => Block::bordered().render(area, frame.buffer_mut()),
        }
    }

    fn update(&mut self, action: Self::Actions) {
        match action {
            MangaReaderActions::NextPage => self.next_page(),
            MangaReaderActions::PreviousPage => self.previous_page(),
        }
    }

    fn handle_events(&mut self, events: crate::backend::tui::Events) {
        match events {
            Events::Key(key_event) => match key_event.code {
                KeyCode::Down | KeyCode::Char('j') => self.next_page(),
                KeyCode::Up | KeyCode::Char('k') => self.previous_page(),
                _ => {}
            },
            Events::Tick => self.tick(),
            _ => {}
        }
    }
}

impl MangaReader {
    fn next_page(&mut self) {
        if (self.current_page_index - 1) == 0 {
            self.current_page_index -= 1;
        }
    }

    fn previous_page(&mut self) {
        if (self.current_page_index + 1) > self.pages.len() {
            self.current_page_index += 1;
        }
    }

    fn abort_fetch_pages(&mut self) {
        self.image_tasks.abort_all();
    }

    fn tick(&mut self) {
        if let Ok(background_event) = self.local_event_rx.try_recv() {
            match background_event {
                MangaReaderEvents::FetchPages => {
                    for (index, page) in self.pages.iter().enumerate() {
                        let file_name = page.url.clone();
                        let endpoint = format!("{}/{}", self.base_url, self.chapter_id);
                        let client = Arc::clone(&self.client);
                        let tx = self.local_event_tx.clone();
                        self.image_tasks.spawn(async move {
                            let image_response =
                                client.get_chapter_page(&endpoint, &file_name).await;
                            match image_response {
                                Ok(bytes) => tx
                                    .send(MangaReaderEvents::DecodeImage(Some(bytes), index))
                                    .unwrap(),
                                Err(_) => tx
                                    .send(MangaReaderEvents::DecodeImage(None, index))
                                    .unwrap(),
                            };
                        });
                    }
                }
                MangaReaderEvents::LoadPage(maybe_image, index_page) => match maybe_image {
                    Some(image) => {
                        let tx = self.local_event_tx.clone();

                        let (tx_worker, rec_worker) = std::sync::mpsc::channel::<(
                            Box<dyn StatefulProtocol>,
                            Resize,
                            ratatui::prelude::Rect,
                        )>();

                        let image = self.picker.new_resize_protocol(image);

                        match self.pages.get_mut(index_page) {
                            Some(page) => {
                                page.image_state =
                                    Some(ThreadProtocol::new(tx_worker.clone(), image));

                                std::thread::spawn(move || loop {
                                    match rec_worker.recv() {
                                        Ok((mut protocol, resize, area)) => {
                                            protocol.resize_encode(&resize, None, area);
                                            tx.send(MangaReaderEvents::Redraw(
                                                protocol, index_page,
                                            ))
                                            .unwrap();
                                        }
                                        Err(_e) => break,
                                    }
                                });
                            }
                            None => {
                                //Todo! indicate that the image for a page could not be loadded
                            }
                        }
                    }
                    None => {
                        //Todo! indicate that the image for a page could not be loadded
                    }
                },

                MangaReaderEvents::DecodeImage(maybe_bytes, index_page) => {
                    let tx = self.local_event_tx.clone();
                    match maybe_bytes {
                        Some(bytes) => {
                            let dyn_img = Reader::new(std::io::Cursor::new(bytes))
                                .with_guessed_format()
                                .unwrap();

                            std::thread::spawn(move || {
                                let maybe_decoded = dyn_img.decode();
                                match maybe_decoded {
                                    Ok(image) => {
                                        tx.send(MangaReaderEvents::LoadPage(
                                            Some(image),
                                            index_page,
                                        ))
                                        .unwrap();
                                    }
                                    Err(_) => {
                                        tx.send(MangaReaderEvents::LoadPage(None, index_page))
                                            .unwrap();
                                    }
                                };
                            });
                        }
                        None => tx
                            .send(MangaReaderEvents::LoadPage(None, index_page))
                            .unwrap(),
                    }
                }
                MangaReaderEvents::Redraw(protocol, index_page) => {
                    if let Some(page) = self.pages.get_mut(index_page) {
                        if let Some(img_state) = page.image_state.as_mut() {
                            img_state.inner = Some(protocol);
                        }
                    }
                }
            }
        }
    }
}
