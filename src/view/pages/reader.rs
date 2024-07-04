use std::sync::Arc;

use bytes::Bytes;
use crossterm::event::KeyCode;
use image::io::Reader;
use image::DynamicImage;
use ratatui::{prelude::*, widgets::*};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::Resize;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;

use crate::backend::fetch::MangadexClient;
use crate::backend::tui::Events;
use crate::view::widgets::{Component, ThreadImage, ThreadProtocol};

pub enum MangaReaderActions {
    NextPage,
    PreviousPage,
    GoBackToMangaPage,
}

pub enum MangaReaderEvents {
    FetchPages,
    DecodeImage(Option<Bytes>, usize),
    LoadPage(Option<DynamicImage>, usize),
    Redraw(Box<dyn StatefulProtocol>, usize),
}

pub enum PageType {
    HighQuality,
    LowQuality,
}

pub struct Page {
    pub image_state: Option<ThreadProtocol>,
    pub url: String,
    pub page_type: PageType,
}

impl Page {
    pub fn new(url: String, page_type: PageType) -> Self {
        Self {
            image_state: None,
            url,
            page_type,
        }
    }
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
    pub global_event_tx: UnboundedSender<Events>,
    pub local_action_tx: UnboundedSender<MangaReaderActions>,
    pub local_action_rx: UnboundedReceiver<MangaReaderActions>,
    pub local_event_tx: UnboundedSender<MangaReaderEvents>,
    pub local_event_rx: UnboundedReceiver<MangaReaderEvents>,
}

impl Component for MangaReader {
    type Actions = MangaReaderActions;
    fn render(&mut self, area: ratatui::prelude::Rect, frame: &mut Frame<'_>) {
        let buf = frame.buffer_mut();

        let layout = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Fill(2),
            Constraint::Fill(1),
        ]);

        let [left, center, right] = layout.areas(area);

        Block::bordered().render(left, buf);
        Block::bordered().render(right, buf);

        match self.pages.get_mut(self.current_page_index) {
            Some(page) => match page.image_state.as_mut() {
                Some(img_state) => {
                    let image = ThreadImage::new().resize(Resize::Fit(None));
                    StatefulWidget::render(image, center, buf, img_state);
                }
                None => {
                    Block::bordered()
                        .title("Loading page")
                        .render(center, frame.buffer_mut());
                }
            },
            None => Block::bordered()
                .title("Loading page")
                .render(center, frame.buffer_mut()),
        };
    }

    fn update(&mut self, action: Self::Actions) {
        match action {
            MangaReaderActions::NextPage => self.next_page(),
            MangaReaderActions::PreviousPage => self.previous_page(),
            MangaReaderActions::GoBackToMangaPage => self.go_back_manga_page(),
        }
    }

    fn handle_events(&mut self, events: crate::backend::tui::Events) {
        match events {
            Events::Key(key_event) => match key_event.code {
                KeyCode::Down | KeyCode::Char('j') => self
                    .local_action_tx
                    .send(MangaReaderActions::NextPage)
                    .unwrap(),
                KeyCode::Up | KeyCode::Char('k') => self
                    .local_action_tx
                    .send(MangaReaderActions::PreviousPage)
                    .unwrap(),
                KeyCode::Tab => self
                    .local_action_tx
                    .send(MangaReaderActions::GoBackToMangaPage)
                    .unwrap(),

                _ => {}
            },
            Events::Tick => self.tick(),
            _ => {}
        }
    }
}

impl MangaReader {
    pub fn new(
        global_event_tx: UnboundedSender<Events>,
        chapter_id: String,
        base_url: String,
        picker: Picker,
        client: Arc<MangadexClient>,
        url_imgs: Vec<String>,
        url_imgs_high_quality: Vec<String>,
    ) -> Self {
        let set: JoinSet<()> = JoinSet::new();
        let (local_action_tx, local_action_rx) = mpsc::unbounded_channel::<MangaReaderActions>();
        let (local_event_tx, local_event_rx) = mpsc::unbounded_channel::<MangaReaderEvents>();

        let mut pages: Vec<Page> = vec![];

        for url in url_imgs {
            pages.push(Page::new(url, PageType::LowQuality));
        }

        for url in url_imgs_high_quality {
            pages.push(Page::new(url, PageType::HighQuality));
        }

        local_event_tx.send(MangaReaderEvents::FetchPages).unwrap();

        Self {
            global_event_tx,
            chapter_id,
            base_url,
            pages,
            current_page_index: 0,
            image_tasks: set,
            picker,
            client,
            local_action_tx,
            local_action_rx,
            local_event_tx,
            local_event_rx,
        }
    }

    fn next_page(&mut self) {
        if (self.current_page_index + 1) < self.pages.len() {
            self.current_page_index += 1;
        }
    }

    fn previous_page(&mut self) {
        self.current_page_index = self.current_page_index.saturating_sub(1);
    }

    fn abort_fetch_pages(&mut self) {
        self.image_tasks.abort_all();
    }

    fn go_back_manga_page(&mut self) {
        self.abort_fetch_pages();
        self.global_event_tx.send(Events::GoBackMangaPage).unwrap();
    }

    fn tick(&mut self) {
        if let Ok(background_event) = self.local_event_rx.try_recv() {
            match background_event {
                MangaReaderEvents::FetchPages => {
                    for (index, page) in self.pages.iter().enumerate() {
                        let file_name = page.url.clone();
                        let endpoint = format!(
                            "{}/{}/{}",
                            self.base_url,
                            match page.page_type {
                                PageType::LowQuality => "data-saver",
                                PageType::HighQuality => "data",
                            },
                            self.chapter_id
                        );
                        let client = Arc::clone(&self.client);
                        let tx = self.local_event_tx.clone();
                        self.image_tasks.spawn(async move {
                            let image_response =
                                client.get_chapter_page(&endpoint, &file_name).await;
                            match image_response {
                                Ok(bytes) => tx
                                    .send(MangaReaderEvents::DecodeImage(Some(bytes), index))
                                    .unwrap(),
                                Err(e) => panic!("could not get chapter :{e}"),
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
                                panic!("could note load image")
                            }
                        }
                    }
                    None => {
                        panic!("could note load image")
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
