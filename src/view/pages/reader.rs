use crate::backend::error_log::{write_to_error_log, ErrorType};
use crate::backend::fetch::MangadexClient;
use crate::backend::tui::Events;
use crate::global::INSTRUCTIONS_STYLE;
use crate::view::widgets::reader::{PageItemState, PagesItem, PagesList};
use crate::view::widgets::Component;
use crate::PICKER;
use crossterm::event::KeyCode;
use image::io::Reader;
use image::GenericImageView;
use ratatui::{prelude::*, widgets::*};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};
use strum::Display;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;

// Todo! resize layout depending on page ratio,

pub enum MangaReaderActions {
    NextPage,
    PreviousPage,
}

pub enum State {
    SearchingPages,
    StoppedSearching,
}

pub struct PageData {
    pub protocol: Box<dyn StatefulProtocol>,
    pub index: usize,
    pub dimensions: (u32, u32),
}

pub enum MangaReaderEvents {
    // Todo! make a way to fetch pages for every 2 or 3 seconds
    FetchPages(usize),
    LoadPage(Option<PageData>),
}

#[derive(Display)]
pub enum PageType {
    #[strum(to_string = "data")]
    HighQuality,
    #[strum(to_string = "data-saver")]
    LowQuality,
}

pub struct Page {
    pub image_state: Option<Box<dyn StatefulProtocol>>,
    pub url: String,
    pub page_type: PageType,
    pub dimensions: Option<(u32, u32)>,
}

impl Page {
    pub fn new(url: String, page_type: PageType) -> Self {
        Self {
            image_state: None,
            dimensions: None,
            url,
            page_type,
        }
    }
}

pub struct MangaReader {
    chapter_id: String,
    base_url: String,
    pages: Vec<Page>,
    pages_list: PagesList,
    page_list_state: tui_widget_list::ListState,
    state: State,
    /// Handle fetching the images
    image_tasks: JoinSet<()>,
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
        self.render_page_list(left, buf);

        Paragraph::new(Line::from(vec![
            "Go back: ".into(),
            Span::raw("<Backspace>").style(*INSTRUCTIONS_STYLE),
        ]))
        .render(right, buf);

        match self
            .pages
            .get_mut(self.page_list_state.selected.unwrap_or(0))
        {
            Some(page) => match page.image_state.as_mut() {
                Some(img_state) => {
                    let (width, height) = page.dimensions.unwrap();
                    if width > height {
                        let image = StatefulImage::new(None).resize(Resize::Fit(None));
                        StatefulWidget::render(image, area, buf, img_state);
                    } else {
                        let image = StatefulImage::new(None).resize(Resize::Fit(None));
                        StatefulWidget::render(image, center, buf, img_state);
                    }
                }
                None => {
                    Block::bordered()
                        .title("Loading page")
                        .render(center, frame.buffer_mut());
                }
            },
            None => {


Block::bordered()
                .title("Loading page")
                .render(center, frame.buffer_mut())
            },
        };
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
                KeyCode::Down | KeyCode::Char('j') => {
                    self.local_action_tx.send(MangaReaderActions::NextPage).ok();
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.local_action_tx
                        .send(MangaReaderActions::PreviousPage)
                        .ok();
                }

                _ => {}
            },
            Events::Tick => self.tick(),
            _ => {}
        }
    }

    fn clean_up(&mut self) {
        self.image_tasks.abort_all();
        self.pages = vec![];
        self.pages_list.pages = vec![];
    }
}

impl MangaReader {
    pub fn new(
        global_event_tx: UnboundedSender<Events>,
        chapter_id: String,
        base_url: String,
        url_imgs: Vec<String>,
        url_imgs_high_quality: Vec<String>,
    ) -> Self {
        let set: JoinSet<()> = JoinSet::new();
        let (local_action_tx, local_action_rx) = mpsc::unbounded_channel::<MangaReaderActions>();
        let (local_event_tx, local_event_rx) = mpsc::unbounded_channel::<MangaReaderEvents>();

        let mut pages: Vec<Page> = vec![];

        for url in url_imgs.iter().take(5) {
            pages.push(Page::new(url.to_string(), PageType::LowQuality));
        }

        for url in url_imgs_high_quality.iter().skip(5) {
            pages.push(Page::new(url.to_string(), PageType::HighQuality));
        }

        local_event_tx.send(MangaReaderEvents::FetchPages(5)).ok();

        Self {
            global_event_tx,
            chapter_id,
            base_url,
            pages,
            page_list_state: tui_widget_list::ListState::default(),
            image_tasks: set,
            local_action_tx,
            local_action_rx,
            local_event_tx,
            local_event_rx,
            state: State::SearchingPages,
            pages_list: PagesList::default(),
        }
    }

    fn next_page(&mut self) {
        self.page_list_state.next()
    }

    fn previous_page(&mut self) {
        self.page_list_state.previous();
    }

    fn render_page_list(&mut self, area: Rect, buf: &mut Buffer) {
        let inner_area = area.inner(Margin {
            horizontal: 1,
            vertical: 1,
        });
        StatefulWidget::render(
            self.pages_list.clone(),
            inner_area,
            buf,
            &mut self.page_list_state,
        );
    }

    fn load_page(&mut self, maybe_data: Option<PageData>) {
        if let Some(data) = maybe_data {
            match self.pages.get_mut(data.index) {
                Some(page) => {
                    page.image_state = Some(data.protocol);
                    page.dimensions = Some(data.dimensions);
                }
                None => {
                    // Todo! indicate that the page couldnot be loaded
                }
            };
            match self.pages_list.pages.get_mut(data.index) {
                Some(page_item) => page_item.state = PageItemState::FinishedLoad,
                None => {
                    // Todo! indicate with an x that some page didnt load
                }
            }
        }
    }

    fn tick(&mut self) {
        self.pages_list.on_tick();
        if let Ok(background_event) = self.local_event_rx.try_recv() {
            match background_event {
                MangaReaderEvents::FetchPages(amount) => {
                    let mut pages_list: Vec<PagesItem> = vec![];
                    for (index, page) in self.pages.iter().enumerate() {
                        let file_name = page.url.clone();
                        let endpoint =
                            format!("{}/{}/{}", self.base_url, page.page_type, self.chapter_id);
                        let tx = self.local_event_tx.clone();
                        pages_list.push(PagesItem::new(index));
                        self.image_tasks.spawn(async move {
                            let image_response = MangadexClient::global()
                                .get_chapter_page(&endpoint, &file_name)
                                .await;
                            match image_response {
                                Ok(bytes) => {
                                    let dyn_img = Reader::new(std::io::Cursor::new(bytes))
                                        .with_guessed_format();

                                    if let Err(err) = dyn_img {
                                        return write_to_error_log(ErrorType::FromError(Box::new(
                                            err,
                                        )));
                                    }

                                    let maybe_decoded = dyn_img.unwrap().decode();

                                    if let Ok(decoded) = maybe_decoded {
                                        let page_data = PageData {
                                            dimensions: decoded.dimensions(),
                                            protocol: PICKER.unwrap().new_resize_protocol(decoded),
                                            index,
                                        };
                                        tx.send(MangaReaderEvents::LoadPage(Some(page_data))).ok();
                                    }
                                }
                                Err(e) => {
                                    write_to_error_log(ErrorType::FromError(Box::new(e)));
                                }
                            };
                        });
                    }
                    self.pages_list = PagesList::new(pages_list);
                }
                MangaReaderEvents::LoadPage(maybe_data) => self.load_page(maybe_data),
            }
        }
    }
}
