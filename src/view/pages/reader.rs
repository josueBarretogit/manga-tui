use crossterm::event::KeyCode;
use image::DynamicImage;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, StatefulWidget, Widget};
use ratatui::Frame;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;

use crate::backend::fetch::{MangadexClient, MockApiClient};
use crate::backend::tui::Events;
use crate::common::PageType;
use crate::global::INSTRUCTIONS_STYLE;
use crate::view::tasks::reader::get_manga_panel;
use crate::view::widgets::reader::{PageItemState, PagesItem, PagesList};
use crate::view::widgets::Component;

#[derive(Debug, PartialEq, Eq)]
pub enum MangaReaderActions {
    NextPage,
    PreviousPage,
}

#[derive(Debug, PartialEq, Eq)]
pub enum State {
    SearchingPages,
}

#[derive(Debug, PartialEq)]
pub struct PageData {
    pub img: DynamicImage,
    pub index: usize,
    pub dimensions: (u32, u32),
}

#[derive(Debug, PartialEq)]
pub enum MangaReaderEvents {
    FetchPages,
    LoadPage(Option<PageData>),
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
    current_page_size: u16,
    page_list_state: tui_widget_list::ListState,
    _state: State,
    /// Handle fetching the images
    image_tasks: JoinSet<()>,
    picker: Picker,
    pub _global_event_tx: UnboundedSender<Events>,
    pub local_action_tx: UnboundedSender<MangaReaderActions>,
    pub local_action_rx: UnboundedReceiver<MangaReaderActions>,
    pub local_event_tx: UnboundedSender<MangaReaderEvents>,
    pub local_event_rx: UnboundedReceiver<MangaReaderEvents>,
}

impl Component for MangaReader {
    type Actions = MangaReaderActions;

    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) {
        let buf = frame.buffer_mut();


        let layout =
            Layout::horizontal([Constraint::Fill(1), Constraint::Fill(self.current_page_size), Constraint::Fill(1)]).spacing(1);

        let [left, center, right] = layout.areas(area);

        Block::bordered().render(left, buf);
        self.render_page_list(left, buf);

        Paragraph::new(Line::from(vec!["Go back: ".into(), Span::raw("<Backspace>").style(*INSTRUCTIONS_STYLE)]))
            .render(right, buf);

        match self.pages.get_mut(self.page_list_state.selected.unwrap_or(0)) {
            Some(page) => match page.image_state.as_mut() {
                Some(img_state) => {
                    let (width, height) = page.dimensions.unwrap();
                    if width > height {
                        if width - height > 250 {
                            self.current_page_size = 5;
                        }
                    } else {
                        self.current_page_size = 2;
                    }
                    let image = StatefulImage::new(None).resize(Resize::Fit(None));
                    StatefulWidget::render(image, center, buf, img_state);
                },
                None => {
                    Block::bordered().title("Loading page").render(center, frame.buffer_mut());
                },
            },
            None => Block::bordered().title("Loading page").render(center, frame.buffer_mut()),
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
                },
                KeyCode::Up | KeyCode::Char('k') => {
                    self.local_action_tx.send(MangaReaderActions::PreviousPage).ok();
                },

                _ => {},
            },
            Events::Mouse(mouse_event) => match mouse_event.kind {
                crossterm::event::MouseEventKind::ScrollUp => {
                    self.local_action_tx.send(MangaReaderActions::PreviousPage).ok();
                },
                crossterm::event::MouseEventKind::ScrollDown => {
                    self.local_action_tx.send(MangaReaderActions::NextPage).ok();
                },
                _ => {},
            },
            Events::Tick => self.tick(),
            _ => {},
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
        picker: Picker,
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

        local_event_tx.send(MangaReaderEvents::FetchPages).ok();

        Self {
            _global_event_tx: global_event_tx,
            chapter_id,
            base_url,
            pages,
            page_list_state: tui_widget_list::ListState::default(),
            image_tasks: set,
            local_action_tx,
            local_action_rx,
            local_event_tx,
            local_event_rx,
            _state: State::SearchingPages,
            current_page_size: 2,
            pages_list: PagesList::default(),
            picker,
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
        StatefulWidget::render(self.pages_list.clone(), inner_area, buf, &mut self.page_list_state);
    }

    fn load_page(&mut self, maybe_data: Option<PageData>) {
        if let Some(data) = maybe_data {
            match self.pages.get_mut(data.index) {
                Some(page) => {
                    let protocol = self.picker.new_resize_protocol(data.img);
                    page.image_state = Some(protocol);
                    page.dimensions = Some(data.dimensions);
                },
                None => {
                    // Todo! indicate that the page couldnot be loaded
                },
            };
            match self.pages_list.pages.get_mut(data.index) {
                Some(page_item) => page_item.state = PageItemState::FinishedLoad,
                None => {
                    // Todo! indicate with an x that some page didnt load
                },
            }
        }
    }

    fn fech_pages(&mut self) {
        let mut pages_list: Vec<PagesItem> = vec![];

        #[cfg(not(test))]
        let api_client = MangadexClient::global();

        #[cfg(test)]
        let api_client = MockApiClient::new();

        for (index, page) in self.pages.iter().enumerate() {
            let file_name = page.url.clone();
            let endpoint = format!("{}/{}/{}", self.base_url, page.page_type, self.chapter_id);
            let tx = self.local_event_tx.clone();
            pages_list.push(PagesItem::new(index));

            self.image_tasks.spawn(get_manga_panel(api_client, endpoint, file_name, tx, index));
        }
        self.pages_list = PagesList::new(pages_list);
    }

    fn tick(&mut self) {
        self.pages_list.on_tick();
        if let Ok(background_event) = self.local_event_rx.try_recv() {
            match background_event {
                MangaReaderEvents::FetchPages => self.fech_pages(),
                MangaReaderEvents::LoadPage(maybe_data) => self.load_page(maybe_data),
            }
        }

    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::view::widgets::press_key;

    fn initialize_reader_page() -> MangaReader {
        let (tx, _) = tokio::sync::mpsc::unbounded_channel::<Events>();
        let picker = Picker::new((8, 19));
        let chapter_id = "some_id".to_string();
        let base_url = "some_base_url".to_string();
        let url_imgs = vec!["some_page_url1".into(), "some_page_url2".into()];
        let url_imgs_high_quality = vec!["some_page_url1".into(), "some_page_url2".into()];
        MangaReader::new(tx, chapter_id, base_url, url_imgs, url_imgs_high_quality, picker)
    }

    #[tokio::test]
    async fn trigget_key_events() {
        let mut reader_page = initialize_reader_page();

        press_key(&mut reader_page, KeyCode::Char('j'));
        let action = reader_page.local_action_rx.recv().await.unwrap();

        assert_eq!(MangaReaderActions::NextPage, action);

        press_key(&mut reader_page, KeyCode::Char('k'));
        let action = reader_page.local_action_rx.recv().await.unwrap();

        assert_eq!(MangaReaderActions::PreviousPage, action);
    }

    #[tokio::test]
    async fn correct_initialization() {
        let mut reader_page = initialize_reader_page();

        let fetch_pages_event = reader_page.local_event_rx.recv().await.expect("the event to fetch pages is not sent");



        assert_eq!(MangaReaderEvents::FetchPages, fetch_pages_event);
        assert!(!reader_page.pages.is_empty());
    }

    #[test]
    fn handle_key_events() {
        let mut reader_page = initialize_reader_page();

        reader_page.pages_list = PagesList::new(vec![PagesItem::new(0), PagesItem::new(1), PagesItem::new(2)]);

        let area = Rect::new(0, 0, 20, 20);
        let mut buf = Buffer::empty(area);

        reader_page.render_page_list(area, &mut buf);

        let action = MangaReaderActions::NextPage;
        reader_page.update(action);

        assert_eq!(0, reader_page.page_list_state.selected.expect("no page is selected"));

        let action = MangaReaderActions::NextPage;
        reader_page.update(action);

        assert_eq!(1, reader_page.page_list_state.selected.expect("no page is selected"));

        let action = MangaReaderActions::PreviousPage;
        reader_page.update(action);

        assert_eq!(0, reader_page.page_list_state.selected.expect("no page is selected"));
    }

    #[tokio::test]
    async fn handle_events() {
        let mut reader_page = initialize_reader_page();
        assert!(reader_page.pages_list.pages.is_empty());

        reader_page.tick();

        assert!(!reader_page.pages_list.pages.is_empty());

        reader_page
            .local_event_tx
            .send(MangaReaderEvents::LoadPage(Some(PageData {
                img: DynamicImage::default(),
                index: 1,
                dimensions: (10, 20),
            })))
            .expect("error sending event");

        reader_page.tick();

        let loaded_page = reader_page.pages.get(1).expect("could not load page");

        assert!(loaded_page.dimensions.is_some_and(|dimensions| dimensions == (10, 20)));
    }
}
