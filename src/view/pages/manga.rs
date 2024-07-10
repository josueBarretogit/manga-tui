use crate::backend::fetch::MangadexClient;
use crate::backend::tui::Events;
use crate::backend::{ChapterResponse, Languages, MangaStatisticsResponse, Statistics};
use crate::utils::{set_status_style, set_tags_style};
use crate::view::widgets::manga::{ChapterItem, ChaptersListWidget};
use crate::view::widgets::Component;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};
use std::sync::Arc;
use strum::Display;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;

#[derive(PartialEq, Eq)]
pub enum PageState {
    SearchingChapters,
    SearchingChapterData,
    SearchingStopped,
}

pub enum MangaPageActions {
    ScrollChapterDown,
    ScrollChapterUp,
    ToggleOrder,
    ReadChapter,
    GoBackSearchPage,
}

pub enum MangaPageEvents {
    FetchChapters,
    FethStatistics,
    StoppedSearchingChapterData,
    LoadChapters(Option<ChapterResponse>),
    LoadStatistics(Option<MangaStatisticsResponse>),
}

#[derive(Display, Default, Clone, Copy)]
pub enum ChapterOrder {
    #[strum(to_string = "asc")]
    Ascending,
    #[strum(to_string = "desc")]
    #[default]
    Descending,
}

impl ChapterOrder {
    fn toggle(self) -> Self {
        match self {
            ChapterOrder::Ascending => ChapterOrder::Descending,
            ChapterOrder::Descending => ChapterOrder::Ascending,
        }
    }
}

pub struct MangaPage {
    id: String,
    pub title: String,
    description: String,
    tags: Vec<String>,
    img_url: Option<String>,
    image_state: Option<Box<dyn StatefulProtocol>>,
    status: String,
    content_rating: String,
    author: String,
    artist: String,
    global_event_tx: UnboundedSender<Events>,
    local_action_tx: UnboundedSender<MangaPageActions>,
    pub local_action_rx: UnboundedReceiver<MangaPageActions>,
    local_event_tx: UnboundedSender<MangaPageEvents>,
    local_event_rx: UnboundedReceiver<MangaPageEvents>,
    chapters: Option<ChaptersData>,
    chapter_order: ChapterOrder,
    chapter_language: Languages,
    state: PageState,
    statistics: Option<MangaStatistics>,
    tasks: JoinSet<()>,
}

struct MangaStatistics {
    rating: f64,
    follows: u64,
}

impl MangaStatistics {
    fn new(rating: f64, follows: u64) -> Self {
        Self { rating, follows }
    }
}

struct ChaptersData {
    state: tui_widget_list::ListState,
    widget: ChaptersListWidget,
    page: u32,
    total_result: u32,
}

#[allow(clippy::too_many_arguments)]
impl MangaPage {
    pub fn new(
        id: String,
        title: String,
        description: String,
        tags: Vec<String>,
        img_url: Option<String>,
        image_state: Option<Box<dyn StatefulProtocol>>,
        status: String,
        content_rating: String,
        author: String,
        artist: String,
        global_event_tx: UnboundedSender<Events>,
    ) -> Self {
        let (local_action_tx, local_action_rx) = mpsc::unbounded_channel::<MangaPageActions>();
        let (local_event_tx, local_event_rx) = mpsc::unbounded_channel::<MangaPageEvents>();

        local_event_tx.send(MangaPageEvents::FetchChapters).ok();
        local_event_tx.send(MangaPageEvents::FethStatistics).ok();

        Self {
            id,
            title,
            description,
            tags,
            img_url,
            image_state,
            status,
            content_rating,
            author,
            artist,
            global_event_tx,
            local_action_tx,
            local_action_rx,
            local_event_tx,
            local_event_rx,
            chapters: None,
            chapter_language: Languages::default(),
            chapter_order: ChapterOrder::default(),
            state: PageState::SearchingChapters,
            statistics: None,
            tasks: JoinSet::new(),
        }
    }
    fn render_cover(&mut self, area: Rect, buf: &mut Buffer) {
        match self.image_state.as_mut() {
            Some(state) => {
                let image = StatefulImage::new(None).resize(Resize::Fit(None));
                StatefulWidget::render(image, area, buf, state);
            }
            None => {
                Block::bordered().render(area, buf);
            }
        }
    }

    fn render_manga_information(&mut self, area: Rect, frame: &mut Frame<'_>) {
        let buf = frame.buffer_mut();

        let layout = Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]);

        let [manga_information_area, manga_chapters_area] = layout.areas(area);

        let statistics = match &self.statistics {
            Some(statistics) => Span::raw(format!(
                "⭐ {} follows : {} ",
                statistics.rating, statistics.follows
            )),
            None => Span::raw("⭐ follows : ".to_string()),
        };

        let author_and_artist = Span::raw(format!(
            "Author : {} | Artist : {}",
            self.author, self.artist
        ));

        Block::bordered()
            .title_top(Line::from(vec![self.title.clone().into()]))
            .title_bottom(Line::from(vec![statistics, "".into(), author_and_artist]))
            .render(manga_information_area, buf);

        self.render_details(manga_information_area, frame.buffer_mut());

        self.render_chapters_area(manga_chapters_area, frame);
    }

    fn render_details(&mut self, area: Rect, buf: &mut Buffer) {
        let layout =
            Layout::vertical([Constraint::Percentage(20), Constraint::Percentage(80)]).margin(1);
        let [tags_area, description_area] = layout.areas(area);

        let mut tags: Vec<Span<'_>> = self.tags.iter().map(|tag| set_tags_style(tag)).collect();

        tags.push(set_tags_style(&self.content_rating));

        tags.push(set_status_style(&self.status));

        Paragraph::new(Line::from(tags))
            .wrap(Wrap { trim: true })
            .render(tags_area, buf);

        Paragraph::new(self.description.clone())
            .wrap(Wrap { trim: true })
            .render(description_area, buf);
    }

    fn render_chapters_area(&mut self, area: Rect, frame: &mut Frame<'_>) {
        let layout =
            Layout::vertical([Constraint::Percentage(10), Constraint::Percentage(90)]).margin(1);

        let [sorting_buttons_area, chapters_area] = layout.areas(area);

        MangaPage::render_sorting_buttons(
            sorting_buttons_area,
            frame.buffer_mut(),
            self.chapter_order,
            self.chapter_language,
        );

        match self.chapters.as_mut() {
            Some(chapters) => {
                let page = format!("Page {}", chapters.page);
                let total = format!("Total chapters {}", chapters.total_result);
                Block::bordered()
                    .title_bottom(Line::from(page).left_aligned())
                    .title_bottom(Line::from(total).right_aligned())
                    .render(area, frame.buffer_mut());

                StatefulWidget::render(
                    chapters.widget.clone(),
                    chapters_area,
                    frame.buffer_mut(),
                    &mut chapters.state,
                );
            }
            None => {
                Block::bordered().render(area, frame.buffer_mut());
                // Todo! show chapters are loading
            }
        }
    }

    fn render_sorting_buttons(
        area: Rect,
        buf: &mut Buffer,
        order: ChapterOrder,
        language: Languages,
    ) {
        let layout = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]);
        let [sorting_area, language_area] = layout.areas(area);

        let order_title = format!(
            "Order: {} ",
            match order {
                ChapterOrder::Descending => "Descending",
                ChapterOrder::Ascending => "Ascending",
            }
        );

        Paragraph::new(Line::from(vec![
            order_title.into(),
            " Change order : <o>".into(),
        ]))
        .render(sorting_area, buf);

        // Todo! bring in selectable widget
        let language = format!("Language: {}", language);

        Paragraph::new(language).render(language_area, buf);
    }

    fn handle_key_events(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Tab => {
                if self.state != PageState::SearchingChapterData {
                    self.local_action_tx
                        .send(MangaPageActions::GoBackSearchPage)
                        .ok();
                }
            }
            KeyCode::Char('j') => {
                if self.state != PageState::SearchingChapterData {
                    self.local_action_tx
                        .send(MangaPageActions::ScrollChapterDown)
                        .unwrap();
                }
            }
            KeyCode::Char('k') => {
                if self.state != PageState::SearchingChapterData {
                    self.local_action_tx
                        .send(MangaPageActions::ScrollChapterUp)
                        .unwrap();
                }
            }
            KeyCode::Char('o') => {
                if self.state != PageState::SearchingChapters {
                    self.local_action_tx
                        .send(MangaPageActions::ToggleOrder)
                        .unwrap();
                }
            }
            KeyCode::Char('r') => {
                self.local_action_tx
                    .send(MangaPageActions::ReadChapter)
                    .unwrap();
            }
            _ => {}
        }
    }

    fn abort_tasks(&mut self) {
        self.tasks.abort_all();
    }

    fn scroll_chapter_down(&mut self) {
        if let Some(chapters) = self.chapters.as_mut() {
            chapters.state.next();
        }
    }

    fn scroll_chapter_up(&mut self) {
        if let Some(chapters) = self.chapters.as_mut() {
            chapters.state.previous();
        }
    }

    fn toggle_chapter_order(&mut self) {
        self.chapter_order = self.chapter_order.toggle();
        self.search_chapters();
    }

    // Todo! filter by language
    fn change_language(&mut self) {
        self.search_chapters();
    }

    fn get_current_selected_chapter_mut(&mut self) -> Option<&mut ChapterItem> {
        match self.chapters.as_mut() {
            Some(chapters_data) => match chapters_data.state.selected {
                Some(selected_chapter_index) => {
                    return chapters_data
                        .widget
                        .chapters
                        .get_mut(selected_chapter_index)
                }
                None => None,
            },
            None => None,
        }
    }

    fn read_chapter(&mut self) {
        self.state = PageState::SearchingChapterData;
        if let Some(chapter_selected) = self.get_current_selected_chapter_mut() {
            let id_chapter = chapter_selected.id.clone();
            let tx = self.global_event_tx.clone();
            let local_tx = self.local_event_tx.clone();
            tokio::spawn(async move {
                let chapter_response = MangadexClient::global().get_chapter_pages(&id_chapter).await;
                match chapter_response {
                    Ok(response) => {
                        tx.send(Events::ReadChapter(response)).unwrap();
                        local_tx
                            .send(MangaPageEvents::StoppedSearchingChapterData)
                            .unwrap();
                    }
                    Err(e) => {
                        panic!("{e}");
                    }
                }
            });
        }
    }

    fn search_chapters(&mut self) {
        self.state = PageState::SearchingChapters;
        let manga_id = self.id.clone();
        let tx = self.local_event_tx.clone();
        let language = self.chapter_language;
        let chapter_order = self.chapter_order;
        self.tasks.spawn(async move {
            let response = MangadexClient::global()
                .get_manga_chapters(manga_id, 1, language, chapter_order)
                .await;

            match response {
                Ok(chapters_response) => tx
                    .send(MangaPageEvents::LoadChapters(Some(chapters_response)))
                    .unwrap(),
                Err(_e) => tx.send(MangaPageEvents::LoadChapters(None)).unwrap(),
            }
        });
    }

    fn fetch_statistics(&mut self) {
        let manga_id = self.id.clone();
        let tx = self.local_event_tx.clone();
        self.tasks.spawn(async move {
            let response = MangadexClient::global().get_manga_statistics(&manga_id).await;

            match response {
                Ok(res) => {
                    tx.send(MangaPageEvents::LoadStatistics(Some(res))).ok();
                }
                Err(_) => {
                    tx.send(MangaPageEvents::LoadStatistics(None)).ok();
                }
            };
        });
    }

    fn tick(&mut self) {
        if let Ok(background_event) = self.local_event_rx.try_recv() {
            match background_event {
                MangaPageEvents::FethStatistics => self.fetch_statistics(),
                MangaPageEvents::FetchChapters => self.search_chapters(),
                MangaPageEvents::LoadChapters(response) => {
                    self.state = PageState::SearchingStopped;
                    match response {
                        Some(response) => {
                            let mut list_state = tui_widget_list::ListState::default();

                            list_state.select(Some(0));

                            self.chapters = Some(ChaptersData {
                                state: list_state,
                                widget: ChaptersListWidget::from_response(&response),
                                page: 1,
                                total_result: response.total as u32,
                            });
                        }
                        None => self.chapters = None,
                    }
                }
                MangaPageEvents::LoadStatistics(maybe_statistics) => {
                    //todo! set this task as finished
                    match maybe_statistics {
                        Some(response) => {
                            let statistics: &Statistics = &response.statistics[&self.id];
                            self.statistics = Some(MangaStatistics::new(
                                statistics.rating.average,
                                statistics.follows,
                            ));
                        }
                        None => {
                            // Todo! show that statistics could not be found
                        }
                    }
                }
                MangaPageEvents::StoppedSearchingChapterData => {
                    self.state = PageState::SearchingStopped
                }
            }
        }
    }
}

impl Component for MangaPage {
    type Actions = MangaPageActions;
    fn render(&mut self, area: ratatui::prelude::Rect, frame: &mut ratatui::Frame<'_>) {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)]);

        let [cover_area, information_area] = layout.areas(area);

        self.render_cover(cover_area, frame.buffer_mut());
        self.render_manga_information(information_area, frame);
    }
    fn update(&mut self, action: Self::Actions) {
        match action {
            MangaPageActions::ScrollChapterUp => self.scroll_chapter_up(),
            MangaPageActions::ScrollChapterDown => self.scroll_chapter_down(),
            MangaPageActions::ToggleOrder => {
                if self.state != PageState::SearchingChapters {
                    self.toggle_chapter_order()
                }
            }
            MangaPageActions::ReadChapter => {
                if self.state != PageState::SearchingChapterData {
                    self.read_chapter();
                }
            }
            MangaPageActions::GoBackSearchPage => {
                self.abort_tasks();
                self.global_event_tx.send(Events::GoSearchPage).unwrap();
            }
        }
    }
    fn handle_events(&mut self, events: Events) {
        match events {
            Events::Key(key_event) => self.handle_key_events(key_event),
            _ => self.tick(),
        }
    }
}
