use crate::backend::database::{
    get_chapters_history_status, save_history, SetChapterDownloaded, DBCONN,
};
use crate::backend::database::{set_chapter_downloaded, MangaReadingHistorySave};
use crate::backend::download::{download_chapter, DownloadChapter};
use crate::backend::error_log::{self, write_to_error_log};
use crate::backend::fetch::MangadexClient;
use crate::backend::tui::Events;
use crate::backend::{ChapterResponse, Languages, MangaStatisticsResponse, Statistics};
use crate::utils::{set_status_style, set_tags_style};
use crate::view::widgets::manga::{ChapterItem, ChaptersListWidget};
use crate::view::widgets::Component;
use crate::PICKER;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};
use strum::Display;
use throbber_widgets_tui::ThrobberState;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;

#[derive(PartialEq, Eq)]
pub enum PageState {
    DownloadingChapters,
    SearchingChapters,
    SearchingChapterData,
    SearchingStopped,
}

pub enum MangaPageActions {
    DownloadChapter,
    ScrollChapterDown,
    ScrollChapterUp,
    ToggleOrder,
    ReadChapter,
}

pub enum MangaPageEvents {
    FetchChapters,
    FethStatistics,
    CheckChapterStatus,
    ChapterFinishedDownloading(String),
    SaveChapterDownloadStatus(String, String),
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
        let [cover_area, more_details_area] =
            Layout::vertical([Constraint::Percentage(80), Constraint::Percentage(20)]).areas(area);
        match self.image_state.as_mut() {
            Some(state) => {
                let image = StatefulImage::new(None).resize(Resize::Fit(None));
                StatefulWidget::render(image, cover_area, buf, state);
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
                statistics.rating.round(),
                statistics.follows
            )),
            None => Span::raw("⭐ follows : "),
        };

        let author_and_artist = Span::raw(format!(
            "Author : {} | Artist : {}",
            self.author, self.artist
        ));

        let instructions = vec![
            "More about author/artist ".into(),
            "<u>/<a>".bold().fg(Color::Yellow),
        ];

        Block::bordered()
            .title_top(Line::from(vec![self.title.clone().into()]))
            .title_bottom(Line::from(vec![statistics, "".into(), author_and_artist]))
            .title_bottom(Line::from(instructions).right_aligned())
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
            Layout::vertical([Constraint::Percentage(10), Constraint::Percentage(90)]).margin(2);

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

                let chapter_instructions = vec![
                    "Scroll Down/Up ".into(),
                    " <j>/<k> ".bold().fg(Color::Yellow),
                    " Download chapter ".into(),
                    " <d> ".bold().fg(Color::Yellow),
                ];

                Block::bordered()
                    .title_top(Line::from(chapter_instructions))
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
                if PICKER.is_some() {
                    self.local_action_tx
                        .send(MangaPageActions::ReadChapter)
                        .ok();
                }
            }
            KeyCode::Char('d') => {
                self.local_action_tx
                    .send(MangaPageActions::DownloadChapter)
                    .ok();
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

    fn get_current_selected_chapter(&self) -> Option<&ChapterItem> {
        match self.chapters.as_ref() {
            Some(chapters_data) => match chapters_data.state.selected {
                Some(selected_chapter_index) => {
                    return chapters_data.widget.chapters.get(selected_chapter_index)
                }
                None => None,
            },
            None => None,
        }
    }

    fn read_chapter(&mut self) {
        self.state = PageState::SearchingChapterData;
        match self.get_current_selected_chapter() {
            Some(chapter_selected) => {
                let id_chapter = chapter_selected.id.clone();
                let tx = self.global_event_tx.clone();
                if DBCONN.lock().unwrap().is_some() && !chapter_selected.is_read {
                    let save_response = save_history(MangaReadingHistorySave {
                        id: &self.id,
                        title: &self.title,
                        img_url: self.img_url.as_deref(),
                        chapter_id: &chapter_selected.id,
                        chapter_title: &chapter_selected.title,
                    });

                    if let Err(e) = save_response {
                        write_to_error_log(error_log::ErrorType::FromError(Box::new(e)));
                    }
                }

                let local_tx = self.local_event_tx.clone();

                tokio::spawn(async move {
                    let chapter_response = MangadexClient::global()
                        .get_chapter_pages(&id_chapter)
                        .await;
                    match chapter_response {
                        Ok(response) => {
                            tx.send(Events::ReadChapter(response)).unwrap();

                            local_tx
                                .send(MangaPageEvents::StoppedSearchingChapterData)
                                .ok();

                            local_tx.send(MangaPageEvents::CheckChapterStatus).ok();
                        }
                        Err(e) => {
                            local_tx
                                .send(MangaPageEvents::StoppedSearchingChapterData)
                                .ok();

                            write_to_error_log(error_log::ErrorType::FromError(Box::new(e)));
                        }
                    }
                });
            }
            None => self.state = PageState::SearchingStopped,
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
                Err(e) => {
                    write_to_error_log(error_log::ErrorType::FromError(Box::new(e)));
                    tx.send(MangaPageEvents::LoadChapters(None)).unwrap()
                }
            }
        });
    }

    fn fetch_statistics(&mut self) {
        let manga_id = self.id.clone();
        let tx = self.local_event_tx.clone();
        self.tasks.spawn(async move {
            let response = MangadexClient::global()
                .get_manga_statistics(&manga_id)
                .await;

            match response {
                Ok(res) => {
                    tx.send(MangaPageEvents::LoadStatistics(Some(res))).ok();
                }
                Err(e) => {
                    write_to_error_log(error_log::ErrorType::FromError(Box::new(e)));
                    tx.send(MangaPageEvents::LoadStatistics(None)).ok();
                }
            };
        });
    }

    fn check_chapters_read(&mut self) {
        let history = get_chapters_history_status(&self.id);
        match history {
            Ok(his) => {
                for chapter in self.chapters.as_mut().unwrap().widget.chapters.iter_mut() {
                    let chapter_found = his.iter().find(|chap| chap.id == chapter.id);
                    if let Some(chapt) = chapter_found {
                        chapter.is_read = chapt.is_read;
                        chapter.is_downloaded = chapt.is_downloaded
                    }
                }
            }
            Err(e) => {
                write_to_error_log(error_log::ErrorType::FromError(Box::new(e)));
            }
        }
    }

    fn download_chapter_selected(&mut self) {
        let manga_id = self.id.clone();
        let manga_title = self.title.clone();
        let tx = self.local_event_tx.clone();
        let lang: &str = self.chapter_language.into();
        let lang = lang.to_string();
        self.state = PageState::DownloadingChapters;
        if let Some(chapter) = self.get_current_selected_chapter_mut() {
            let title = chapter.title.clone();
            let number = chapter.chapter_number.clone();
            let scanlator = chapter.scanlator.clone();
            let chapter_id = chapter.id.clone();

            if chapter.download_loading_state.is_some() {
                return;
            }

            chapter.download_loading_state = Some(ThrobberState::default());

            self.tasks.spawn(async move {
                let manga_response = MangadexClient::global()
                    .get_chapter_pages(&chapter_id)
                    .await;
                match manga_response {
                    Ok(res) => {
                        download_chapter(
                            DownloadChapter {
                                id_chapter: &chapter_id,
                                manga_id: &manga_id,
                                manga_title: &manga_title,
                                title: &title,
                                number: &number,
                                scanlator: &scanlator,
                                lang: &lang,
                            },
                            res,
                            tx.clone(),
                        )
                        .unwrap();

                        tx.send(MangaPageEvents::SaveChapterDownloadStatus(
                            chapter_id, title,
                        ))
                        .ok();
                    }
                    Err(e) => {
                        write_to_error_log(error_log::ErrorType::FromError(Box::new(e)));
                    }
                }
            });
        }
    }

    fn stop_loader_for_chapter(&mut self, chapter_id: String) {
        let chapters = self.chapters.as_mut().unwrap();
        if let Some(chap) = chapters
            .widget
            .chapters
            .iter_mut()
            .find(|chap| chap.id == chapter_id)
        {
            chap.download_loading_state = None;
            self.local_event_tx
                .send(MangaPageEvents::CheckChapterStatus)
                .ok();
        }
    }
    fn save_download_status(&mut self, id_chapter: String, title: String) {
        let save_download_operation = set_chapter_downloaded(SetChapterDownloaded {
            id: &id_chapter,
            title: &title,
            manga_id: &self.id,
            manga_title: &self.title,
            img_url: self.img_url.as_deref(),
        });

        if let Err(e) = save_download_operation {
            write_to_error_log(error_log::ErrorType::FromError(Box::new(e)));
        }
    }

    fn tick(&mut self) {
        if self.state == PageState::DownloadingChapters {
            let chapters = self.chapters.as_mut().unwrap();
            for chapt in chapters
                .widget
                .chapters
                .iter_mut()
                .filter(|chap| chap.download_loading_state.is_some())
            {
                chapt.download_loading_state.as_mut().unwrap().calc_next();
            }
        }

        if let Ok(background_event) = self.local_event_rx.try_recv() {
            match background_event {
                MangaPageEvents::SaveChapterDownloadStatus(id_chapter, title) => {
                    self.save_download_status(id_chapter, title)
                }
                MangaPageEvents::ChapterFinishedDownloading(id_chapter) => {
                    self.stop_loader_for_chapter(id_chapter)
                }
                MangaPageEvents::FethStatistics => self.fetch_statistics(),
                MangaPageEvents::FetchChapters => self.search_chapters(),
                MangaPageEvents::LoadChapters(response) => {
                    self.state = PageState::SearchingStopped;
                    match response {
                        Some(response) => {
                            let mut list_state = tui_widget_list::ListState::default();

                            list_state.select(Some(0));

                            let chapter_widget = ChaptersListWidget::from_response(&response);

                            self.chapters = Some(ChaptersData {
                                state: list_state,
                                widget: chapter_widget,
                                page: 1,
                                total_result: response.total as u32,
                            });

                            self.local_event_tx
                                .send(MangaPageEvents::CheckChapterStatus)
                                .ok();
                        }
                        None => self.chapters = None,
                    }
                }
                MangaPageEvents::CheckChapterStatus => {
                    self.check_chapters_read();
                }
                MangaPageEvents::LoadStatistics(maybe_statistics) => {
                    if let Some(response) = maybe_statistics {
                        let statistics: &Statistics = &response.statistics[&self.id];
                        self.statistics = Some(MangaStatistics::new(
                            statistics.rating.average.unwrap_or_default(),
                            statistics.follows.unwrap_or_default(),
                        ));
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

            MangaPageActions::DownloadChapter => self.download_chapter_selected(),
        }
    }

    fn handle_events(&mut self, events: Events) {
        match events {
            Events::Key(key_event) => self.handle_key_events(key_event),
            _ => self.tick(),
        }
    }
    fn clean_up(&mut self) {
        self.abort_tasks();
        self.tags = vec![];
        self.description = String::new();
    }
}
