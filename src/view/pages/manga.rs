use crate::backend::database::{get_chapters_history_status, save_history, SetChapterDownloaded};
use crate::backend::database::{set_chapter_downloaded, MangaReadingHistorySave};
use crate::backend::download::{download_chapter, DownloadChapter};
use crate::backend::error_log::{self, write_to_error_log};
use crate::backend::fetch::{MangadexClient, ITEMS_PER_PAGE_CHAPTERS};
use crate::backend::filter::Languages;
use crate::backend::tui::Events;
use crate::backend::{ChapterResponse, MangaStatisticsResponse, Statistics};
use crate::common::Manga;
use crate::global::INSTRUCTIONS_STYLE;
use crate::utils::{set_status_style, set_tags_style};
use crate::view::widgets::manga::{ChapterItem, ChaptersListWidget};
use crate::view::widgets::Component;
use crate::PICKER;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{prelude::*, widgets::*};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};
use strum::Display;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;


#[derive(PartialEq, Eq)]
pub enum PageState {
    DownloadingChapters,
    SearchingChapters,
    SearchingChapterData,
    DisplayingChapters,
    ChaptersNotFound,
}

pub enum MangaPageActions {
    DownloadChapter,
    ScrollChapterDown,
    ScrollChapterUp,
    ToggleOrder,
    ReadChapter,
    OpenAvailableLanguagesList,
    ScrollDownAvailbleLanguages,
    ScrollUpAvailbleLanguages,
    GoMangasAuthor,
    GoMangasArtist,
    SearchNextChapterPage,
    SearchPreviousChapterPage,
}

pub enum MangaPageEvents {
    SearchChapters,
    FethStatistics,
    CheckChapterStatus,
    ChapterFinishedDownloading(String),
    SetDownloadProgress(f64, String),
    SaveChapterDownloadStatus(String, String),
    DownloadError(String),
    ReadError(String),
    ReadSuccesful,
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
    pub manga: Manga,
    image_state: Option<Box<dyn StatefulProtocol>>,
    global_event_tx: UnboundedSender<Events>,
    local_action_tx: UnboundedSender<MangaPageActions>,
    pub local_action_rx: UnboundedReceiver<MangaPageActions>,
    local_event_tx: UnboundedSender<MangaPageEvents>,
    local_event_rx: UnboundedReceiver<MangaPageEvents>,
    chapters: Option<ChaptersData>,
    chapter_order: ChapterOrder,
    state: PageState,
    statistics: Option<MangaStatistics>,
    tasks: JoinSet<()>,
    available_languages_state: ListState,
    is_list_languages_open: bool,
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

impl MangaPage {
    pub fn new(
        manga: Manga,
        image_state: Option<Box<dyn StatefulProtocol>>,
        global_event_tx: UnboundedSender<Events>,
    ) -> Self {
        let (local_action_tx, local_action_rx) = mpsc::unbounded_channel::<MangaPageActions>();
        let (local_event_tx, local_event_rx) = mpsc::unbounded_channel::<MangaPageEvents>();

        local_event_tx.send(MangaPageEvents::SearchChapters).ok();
        local_event_tx.send(MangaPageEvents::FethStatistics).ok();

        Self {
            manga,
            image_state,
            global_event_tx,
            local_action_tx,
            local_action_rx,
            local_event_tx,
            local_event_rx,
            chapters: None,
            chapter_order: ChapterOrder::default(),
            state: PageState::SearchingChapters,
            statistics: None,
            tasks: JoinSet::new(),
            available_languages_state: ListState::default(),
            is_list_languages_open: false,
        }
    }
    fn render_cover(&mut self, area: Rect, buf: &mut Buffer) {
        let [cover_area, more_details_area] =
            Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(area);

        Paragraph::new(format!(
            " \n Publication date : \n {}",
            self.manga.created_at
        ))
        .render(more_details_area, buf);

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
            self.manga.author.name, self.manga.artist.name
        ));

        let instructions = vec![
            "More about author/artist ".into(),
            Span::raw("<f>/<g>").style(*INSTRUCTIONS_STYLE),
        ];

        Block::bordered()
            .title_top(Line::from(vec![self.manga.title.clone().into()]))
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

        let mut tags: Vec<Span<'_>> = self
            .manga
            .tags
            .iter()
            .map(|tag| set_tags_style(tag))
            .collect();

        tags.push(set_status_style(&self.manga.publication_demographic));

        tags.push(set_tags_style(&self.manga.content_rating));

        tags.push(set_status_style(&self.manga.status));

        Paragraph::new(Line::from(tags))
            .wrap(Wrap { trim: true })
            .render(tags_area, buf);

        Paragraph::new(self.manga.description.clone())
            .wrap(Wrap { trim: true })
            .render(description_area, buf);
    }

    fn render_chapters_area(&mut self, area: Rect, frame: &mut Frame<'_>) {
        let buf = frame.buffer_mut();
        let layout =
            Layout::vertical([Constraint::Percentage(10), Constraint::Percentage(90)]).margin(2);

        let [sorting_buttons_area, chapters_area] = layout.areas(area);

        match self.chapters.as_mut() {
            Some(chapters) => {
                let tota_pages = chapters.total_result as f64 / 16_f64;
                let page = format!("Page {} of : {}", chapters.page, tota_pages.ceil());
                let total = format!("Total chapters {}", chapters.total_result);

                let mut chapter_instructions = vec![
                    "Scroll Down/Up ".into(),
                    Span::raw(" <j>/<k> ").style(*INSTRUCTIONS_STYLE),
                    " Download chapter ".into(),
                    Span::raw(" <d> ").style(*INSTRUCTIONS_STYLE),
                ];

                if PICKER.is_some() {
                    chapter_instructions.push(" Read chapter ".into());
                    chapter_instructions.push(Span::raw(" <r> ").style(*INSTRUCTIONS_STYLE));
                }

                let pagination_instructions: Vec<Span<'_>> = vec![
                    page.into(),
                    " | ".into(),
                    total.into(),
                    " Next ".into(),
                    Span::raw("<w>").style(*INSTRUCTIONS_STYLE),
                    " Previous ".into(),
                    Span::raw("<b>").style(*INSTRUCTIONS_STYLE),
                ];

                Block::bordered()
                    .title_top(Line::from(chapter_instructions))
                    .title_bottom(Line::from(pagination_instructions))
                    .render(area, buf);

                StatefulWidget::render(
                    chapters.widget.clone(),
                    chapters_area,
                    buf,
                    &mut chapters.state,
                );

                self.render_sorting_buttons(sorting_buttons_area, buf);
            }

            None => {
                let title = if self.state == PageState::ChaptersNotFound {
                    "Could not get chapters, please try again"
                } else {
                    "Searching chapters"
                };

                Block::bordered()
                    .title(title)
                    .render(area, frame.buffer_mut());
            }
        }
    }

    fn render_sorting_buttons(&mut self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]);
        let [sorting_area, language_area] = layout.areas(area);

        let order_title = format!(
            "Order: {} ",
            match self.chapter_order {
                ChapterOrder::Descending => "Descending",
                ChapterOrder::Ascending => "Ascending",
            }
        );

        Paragraph::new(Line::from(vec![
            order_title.into(),
            " Change order : ".into(),
            Span::raw("<t>").style(*INSTRUCTIONS_STYLE),
        ]))
        .render(sorting_area, buf);

        let languages_list_area = Rect::new(
            language_area.x,
            language_area.y,
            language_area.width,
            language_area.height + 10,
        );

        if self.is_list_languages_open {
            Clear.render(languages_list_area, buf);
            let instructions = Line::from(vec![
                "Close".into(),
                Span::raw(" <Esc> ").style(*INSTRUCTIONS_STYLE),
                "Up/Down".into(),
                Span::raw(" <k><j> ").style(*INSTRUCTIONS_STYLE),
                "Search ".into(),
                Span::raw("<s>").style(*INSTRUCTIONS_STYLE),
            ]);

            let available_language_list = List::new(
                self.manga
                    .available_languages
                    .iter()
                    .map(|lang| format!("{} {}", lang.as_emoji(), lang.as_human_readable())),
            )
            .block(Block::bordered().title(instructions))
            .highlight_style(Style::default().on_blue());

            StatefulWidget::render(
                available_language_list,
                languages_list_area,
                buf,
                &mut self.available_languages_state,
            );
        } else {
            Paragraph::new(Line::from(vec![
                "Available languages : ".into(),
                "<l>".bold().yellow(),
            ]))
            .render(language_area, buf);
        }
    }

    fn handle_key_events(&mut self, key_event: KeyEvent) {
        if self.is_list_languages_open {
            match key_event.code {
                KeyCode::Char('j') => {
                    self.local_action_tx
                        .send(MangaPageActions::ScrollDownAvailbleLanguages)
                        .ok();
                }
                KeyCode::Char('k') => {
                    self.local_action_tx
                        .send(MangaPageActions::ScrollUpAvailbleLanguages)
                        .unwrap();
                }
                KeyCode::Enter | KeyCode::Char('s') => {
                    self.chapters = None;
                    self.search_chapters();
                }
                KeyCode::Char('l') | KeyCode::Esc => {
                    self.local_action_tx
                        .send(MangaPageActions::OpenAvailableLanguagesList)
                        .ok();
                }
                _ => {}
            }
        } else if self.state != PageState::SearchingChapterData {
            match key_event.code {
                KeyCode::Char('j') => {
                    self.local_action_tx
                        .send(MangaPageActions::ScrollChapterDown)
                        .ok();
                }
                KeyCode::Char('k') => {
                    self.local_action_tx
                        .send(MangaPageActions::ScrollChapterUp)
                        .ok();
                }
                KeyCode::Char('t') => {
                    self.local_action_tx
                        .send(MangaPageActions::ToggleOrder)
                        .ok();
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
                KeyCode::Char('f') => {
                    self.local_action_tx
                        .send(MangaPageActions::GoMangasAuthor)
                        .ok();
                }
                KeyCode::Char('g') => {
                    self.local_action_tx
                        .send(MangaPageActions::GoMangasArtist)
                        .ok();
                }
                KeyCode::Char('l') | KeyCode::Esc => {
                    self.local_action_tx
                        .send(MangaPageActions::OpenAvailableLanguagesList)
                        .ok();
                }
                KeyCode::Char('w') => {
                    self.local_action_tx
                        .send(MangaPageActions::SearchNextChapterPage)
                        .ok();
                }
                KeyCode::Char('b') => {
                    self.local_action_tx
                        .send(MangaPageActions::SearchPreviousChapterPage)
                        .ok();
                }
                _ => {}
            }
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

    fn scroll_language_down(&mut self) {
        self.available_languages_state.select_next();
    }

    fn scroll_language_up(&mut self) {
        self.available_languages_state.select_previous();
    }

    fn open_available_languages_list(&mut self) {
        self.is_list_languages_open = !self.is_list_languages_open;
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
                let chapter_title = chapter_selected.title.clone();
                let is_read = chapter_selected.is_read;
                let manga_id = self.manga.id.clone();
                let title = self.manga.title.clone();
                let img_url = self.manga.img_url.clone();
                let tx = self.global_event_tx.clone();
                let local_tx = self.local_event_tx.clone();

                tokio::spawn(async move {
                    let chapter_response = MangadexClient::global()
                        .get_chapter_pages(&id_chapter)
                        .await;
                    match chapter_response {
                        Ok(response) => {
                            if !is_read {
                                let save_response = save_history(MangaReadingHistorySave {
                                    id: &manga_id,
                                    title: &title,
                                    img_url: img_url.as_deref(),
                                    chapter_id: &id_chapter,
                                    chapter_title: &chapter_title,
                                });

                                if let Err(e) = save_response {
                                    write_to_error_log(error_log::ErrorType::FromError(Box::new(
                                        e,
                                    )));
                                }
                            }

                            tx.send(Events::ReadChapter(response)).ok();
                            local_tx.send(MangaPageEvents::CheckChapterStatus).ok();
                            local_tx.send(MangaPageEvents::ReadSuccesful).ok();
                        }
                        Err(e) => {
                            write_to_error_log(error_log::ErrorType::FromError(Box::new(e)));
                            local_tx.send(MangaPageEvents::ReadError(id_chapter)).ok();
                        }
                    }
                });
            }
            None => self.state = PageState::DisplayingChapters,
        }
    }

    fn get_current_selected_language(&mut self) -> Languages {
        let preferred_language = Languages::get_preferred_lang();
        match self.available_languages_state.selected() {
            Some(index) => *self
                .manga
                .available_languages
                .get(index)
                .unwrap_or(preferred_language),
            None => {
                let maybe_preferred_language = self
                    .manga
                    .available_languages
                    .iter()
                    .find(|lang| *lang == preferred_language);

                maybe_preferred_language.cloned().unwrap_or(
                    self.manga
                        .available_languages
                        .first()
                        .cloned()
                        .unwrap_or(*preferred_language),
                )
            }
        }
    }

    fn search_next_chapters(&mut self) {
        if let Some(chapters) = self.chapters.as_mut() {
            if chapters.page * ITEMS_PER_PAGE_CHAPTERS < chapters.total_result {
                chapters.page += 1;
                self.search_chapters();
            }
        }
    }

    fn search_previous_chapters(&mut self) {
        if let Some(chapters) = self.chapters.as_mut() {
            if chapters.page != 1 {
                chapters.page -= 1;
                self.search_chapters();
            }
        }
    }

    fn search_chapters(&mut self) {
        self.state = PageState::SearchingChapters;
        let manga_id = self.manga.id.clone();
        let tx = self.local_event_tx.clone();
        let language = self.get_current_selected_language();
        let chapter_order = self.chapter_order;

        let page = if let Some(chapters) = self.chapters.as_ref() {
            chapters.page
        } else {
            1
        };

        self.tasks.spawn(async move {
            let response = MangadexClient::global()
                .get_manga_chapters(manga_id, page, language, chapter_order)
                .await;

            match response {
                Ok(chapters_response) => tx
                    .send(MangaPageEvents::LoadChapters(Some(chapters_response)))
                    .unwrap(),
                Err(e) => {
                    write_to_error_log(error_log::ErrorType::FromError(Box::new(e)));
                    tx.send(MangaPageEvents::LoadChapters(None)).ok();
                }
            }
        });
    }

    fn fetch_statistics(&mut self) {
        let manga_id = self.manga.id.clone();
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
        let history = get_chapters_history_status(&self.manga.id);
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
        let manga_id = self.manga.id.clone();
        let manga_title = self.manga.title.clone();
        let tx = self.local_event_tx.clone();

        self.state = PageState::DownloadingChapters;
        if let Some(chapter) = self.get_current_selected_chapter_mut() {
            if chapter.download_loading_state.is_some() {
                return;
            }
            let title = chapter.title.clone();
            let number = chapter.chapter_number.clone();
            let scanlator = chapter.scanlator.clone();
            let chapter_id = chapter.id.clone();
            let lang = chapter.translated_language.as_human_readable().to_string();

            chapter.download_loading_state = Some(0.001);

            self.tasks.spawn(async move {
                let manga_response = MangadexClient::global()
                    .get_chapter_pages(&chapter_id)
                    .await;
                match manga_response {
                    Ok(res) => {
                        let download_chapter_task = download_chapter(
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
                        );

                        if let Err(e) = download_chapter_task {
                            write_to_error_log(error_log::ErrorType::FromError(Box::new(e)));
                            tx.send(MangaPageEvents::DownloadError(chapter_id)).ok();
                            return;
                        }

                        tx.send(MangaPageEvents::SaveChapterDownloadStatus(
                            chapter_id, title,
                        ))
                        .ok();
                    }
                    Err(e) => {
                        write_to_error_log(error_log::ErrorType::FromError(Box::new(e)));
                        tx.send(MangaPageEvents::DownloadError(chapter_id)).ok();
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
            manga_id: &self.manga.id,
            manga_title: &self.manga.title,
            img_url: self.manga.img_url.as_deref(),
        });

        if let Err(e) = save_download_operation {
            write_to_error_log(error_log::ErrorType::FromError(Box::new(e)));
        }
    }

    fn go_mangas_author(&mut self) {
        self.global_event_tx
            .send(Events::GoSearchMangasAuthor(self.manga.author.clone()))
            .ok();
    }

    fn go_mangas_artist(&mut self) {
        self.global_event_tx
            .send(Events::GoSearchMangasArtist(self.manga.artist.clone()))
            .ok();
    }

    fn set_download_progress_for_chapter(&mut self, progress: f64, id_chapter: String) {
        if let Some(chapters) = self.chapters.as_mut() {
            if let Some(chap) = chapters
                .widget
                .chapters
                .iter_mut()
                .find(|chap| chap.id == id_chapter)
            {
                chap.download_loading_state = Some(progress);
            }
        }
    }

    fn set_chapter_download_error(&mut self, chapter_id: String) {
        if let Some(chapters) = self.chapters.as_mut() {
            if let Some(chapter) = chapters
                .widget
                .chapters
                .iter_mut()
                .find(|chap| chap.id == chapter_id)
            {
                chapter.set_download_error();
            }
        }
    }

    fn set_chapter_read_error(&mut self, chapter_id: String) {
        self.state = PageState::DisplayingChapters;
        if let Some(chapters) = self.chapters.as_mut() {
            if let Some(chapter) = chapters
                .widget
                .chapters
                .iter_mut()
                .find(|chap| chap.id == chapter_id)
            {
                chapter.set_read_error();
            }
        }
    }

    fn load_chapters(&mut self, response: Option<ChapterResponse>) {
        self.state = PageState::DisplayingChapters;
        match response {
            Some(response) => {
                let mut list_state = tui_widget_list::ListState::default();

                list_state.select(Some(0));

                let chapter_widget = ChaptersListWidget::from_response(&response);

                let page = if let Some(previous) = self.chapters.as_ref() {
                    previous.page
                } else {
                    1
                };

                self.chapters = Some(ChaptersData {
                    state: list_state,
                    widget: chapter_widget,
                    page,
                    total_result: response.total as u32,
                });

                self.local_event_tx
                    .send(MangaPageEvents::CheckChapterStatus)
                    .ok();
            }
            None => {
                self.state = PageState::ChaptersNotFound;
                self.chapters = None;
            }
        }
    }

    fn tick(&mut self) {
        if let Ok(background_event) = self.local_event_rx.try_recv() {
            match background_event {
                MangaPageEvents::ReadError(chapter_id) => {
                    self.set_chapter_read_error(chapter_id);
                }
                MangaPageEvents::DownloadError(chapter_id) => {
                    self.set_chapter_download_error(chapter_id)
                }
                MangaPageEvents::SetDownloadProgress(progress, id_chapter) => {
                    self.set_download_progress_for_chapter(progress, id_chapter)
                }
                MangaPageEvents::SaveChapterDownloadStatus(id_chapter, title) => {
                    self.save_download_status(id_chapter, title)
                }
                MangaPageEvents::ChapterFinishedDownloading(id_chapter) => {
                    self.stop_loader_for_chapter(id_chapter)
                }
                MangaPageEvents::FethStatistics => self.fetch_statistics(),
                MangaPageEvents::SearchChapters => self.search_chapters(),
                MangaPageEvents::LoadChapters(response) => self.load_chapters(response),
                MangaPageEvents::CheckChapterStatus => {
                    self.check_chapters_read();
                }
                MangaPageEvents::LoadStatistics(maybe_statistics) => {
                    if let Some(response) = maybe_statistics {
                        let statistics: &Statistics = &response.statistics[&self.manga.id];
                        self.statistics = Some(MangaStatistics::new(
                            statistics.rating.average.unwrap_or_default(),
                            statistics.follows.unwrap_or_default(),
                        ));
                    }
                }
                MangaPageEvents::ReadSuccesful => self.state = PageState::DisplayingChapters,
            }
        }
    }
}

impl Component for MangaPage {
    type Actions = MangaPageActions;
    fn render(&mut self, area: ratatui::prelude::Rect, frame: &mut ratatui::Frame<'_>) {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(15), Constraint::Percentage(85)]);

        let [cover_area, information_area] = layout.areas(area);

        self.render_cover(cover_area, frame.buffer_mut());
        self.render_manga_information(information_area, frame);
    }
    fn update(&mut self, action: Self::Actions) {
        match action {
            MangaPageActions::SearchPreviousChapterPage => self.search_previous_chapters(),
            MangaPageActions::SearchNextChapterPage => self.search_next_chapters(),
            MangaPageActions::ScrollDownAvailbleLanguages => self.scroll_language_down(),
            MangaPageActions::ScrollUpAvailbleLanguages => self.scroll_language_up(),
            MangaPageActions::OpenAvailableLanguagesList => self.open_available_languages_list(),
            MangaPageActions::GoMangasArtist => self.go_mangas_artist(),
            MangaPageActions::GoMangasAuthor => self.go_mangas_author(),
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
        self.manga.tags = vec![];
        self.manga.description = String::new();
    }
}
