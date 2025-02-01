use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use image::DynamicImage;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::{Line, Span, ToSpan};
use ratatui::widgets::{Block, Clear, List, ListState, Paragraph, StatefulWidget, Widget, Wrap};
use ratatui::Frame;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::Protocol;
use ratatui_image::{Image, Resize};
use strum::EnumIs;
use throbber_widgets_tui::{Throbber, ThrobberState};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;

use super::reader::ListOfChapters;
use crate::backend::database::{
    get_chapters_history_status, save_history, set_chapter_downloaded, Bookmark, ChapterBookmarked, ChapterToBookmark,
    ChapterToSaveHistory, Database, MangaReadingHistorySave, RetrieveBookmark, SetChapterDownloaded, DBCONN,
};
use crate::backend::error_log::{self, write_to_error_log, ErrorType};
use crate::backend::manga_provider::{
    ChapterFilters, ChapterOrderBy, ChapterToRead, GetChaptersResponse, Languages, Manga, MangaPageProvider, Pagination,
};
use crate::backend::tracker::{track_manga, MangaTracker};
use crate::backend::tui::Events;
use crate::backend::AppDirectories;
use crate::common::format_error_message_tracking_reading_history;
use crate::config::MangaTuiConfig;
use crate::global::{ERROR_STYLE, INSTRUCTIONS_STYLE};
use crate::view::app::MangaToRead;
use crate::view::tasks::manga::{download_all_chapters, download_single_chapter};
use crate::view::widgets::manga::{
    ChapterItem, ChaptersListWidget, DownloadAllChaptersState, DownloadAllChaptersWidget, DownloadPhase,
};
use crate::view::widgets::Component;

#[derive(Debug, PartialEq, Eq, Default)]
pub enum BookmarkPhase {
    SearchingFromApi,
    FailedToFetch,
    NotFoundDatabase,
    Found,
    #[default]
    NotSearching,
}

#[derive(Debug, Default)]
pub struct BookMarkState {
    auto_bookmark: bool,
    phase: BookmarkPhase,
    loader: ThrobberState,
}

#[derive(PartialEq, Eq, Debug)]
pub enum PageState {
    DownloadingChapters,
    SearchingChapters,
    SearchingChapterData,
    DisplayingChapters,
    ChaptersNotFound,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MangaPageActions {
    GoToReadBookmarkedChapter,
    /// This event starts the process of downloading a single chapter which is split into two
    /// steps: 1 get the chapter pages and 2 save those pages in the user's filesystem
    DownloadChapter,
    ConfirmDownloadAll,
    CancelDownloadAll,
    AskDownloadAllChapters,
    AskAbortProcces,
    AbortDownloadAllChapters,
    ScrollChapterDown,
    ScrollChapterUp,
    ToggleOrder,
    ReadChapter,
    ToggleAvailableLanguagesList,
    ScrollDownAvailbleLanguages,
    ScrollUpAvailbleLanguages,
    SearchByLanguage,
    SearchNextChapterPage,
    SearchPreviousChapterPage,
    BookMarkChapterSelected,
}

#[derive(Debug, PartialEq, EnumIs)]
pub enum MangaPageEvents {
    ReadChapterBookmarked(ChapterToRead, ListOfChapters),
    FetchBookmarkFailed,
    SearchChapters,
    SearchCover,
    FetchChapterBookmarked(ChapterBookmarked),
    LoadCover(DynamicImage),
    CheckChapterStatus,
    ChapterFinishedDownloading(String),
    DownloadAllChaptersError,
    /// Percentage, id chapter
    SetDownloadProgress(f64, String),
    StartDownloadProgress(f64),
    SetDownloadAllChaptersProgress,
    FinishedDownloadingAllChapters,
    /// id_chapter, chapter_title
    SaveChapterDownloadStatus(String, String),
    /// id_chapter
    DownloadError(String),
    ReadError(String),

    ReadSuccesful(ChapterToRead, ListOfChapters),
    LoadChapters(Option<GetChaptersResponse>),
    TrackingFailed(String),
}

pub struct MangaPage<T, S>
where
    T: MangaPageProvider,
    S: MangaTracker,
{
    pub manga: Manga,
    image_state: Option<Box<dyn Protocol>>,
    cover_area: Rect,
    global_event_tx: Option<UnboundedSender<Events>>,
    local_action_tx: UnboundedSender<MangaPageActions>,
    pub local_action_rx: UnboundedReceiver<MangaPageActions>,
    local_event_tx: UnboundedSender<MangaPageEvents>,
    local_event_rx: UnboundedReceiver<MangaPageEvents>,
    chapters: Option<ChaptersData>,
    chapter_filters: ChapterFilters,
    state: PageState,
    bookmark_state: BookMarkState,
    tasks: JoinSet<()>,
    picker: Option<Picker>,
    available_languages_state: ListState,
    is_list_languages_open: bool,
    download_all_chapters_state: DownloadAllChaptersState,
    manga_tracker: Option<S>,
    manga_provider: Arc<T>,
}

#[derive(Clone, Debug, Default)]
struct ChaptersData {
    state: tui_widget_list::ListState,
    widget: ChaptersListWidget,
    pagination: Pagination,
}

impl<T, S> MangaPage<T, S>
where
    T: MangaPageProvider,
    S: MangaTracker,
{
    pub fn new(manga: Manga, picker: Option<Picker>, provider: Arc<T>) -> Self {
        let (local_action_tx, local_action_rx) = mpsc::unbounded_channel::<MangaPageActions>();
        let (local_event_tx, local_event_rx) = mpsc::unbounded_channel::<MangaPageEvents>();

        local_event_tx.send(MangaPageEvents::SearchChapters).ok();
        local_event_tx.send(MangaPageEvents::SearchCover).ok();

        let cover_area = Rect::default();

        let chapter_language = manga.languages.iter().find(|lang| *lang == Languages::get_preferred_lang()).cloned();

        Self {
            manga,
            image_state: None,
            picker,
            global_event_tx: None,
            local_action_tx,
            local_action_rx,
            local_event_tx: local_event_tx.clone(),
            local_event_rx,
            chapters: None,
            state: PageState::SearchingChapters,
            bookmark_state: BookMarkState::default(),
            tasks: JoinSet::new(),
            available_languages_state: ListState::default(),
            is_list_languages_open: false,
            download_all_chapters_state: DownloadAllChaptersState::new(local_event_tx),
            cover_area,
            manga_tracker: None,
            manga_provider: provider,
            chapter_filters: ChapterFilters {
                order: ChapterOrderBy::default(),
                language: chapter_language.unwrap_or_default(),
            },
        }
    }

    pub fn with_global_sender(mut self, sender: UnboundedSender<Events>) -> Self {
        self.global_event_tx = Some(sender);
        self
    }

    pub fn auto_bookmark(mut self, auto_bookmark: bool) -> Self {
        self.bookmark_state.auto_bookmark = auto_bookmark;
        self
    }

    pub fn with_manga_tracker(mut self, tracker: Option<S>) -> Self {
        self.manga_tracker = tracker;
        self
    }

    fn render_cover(&mut self, area: Rect, buf: &mut Buffer) {
        let [cover_area, more_details_area] =
            Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(area);

        match self.bookmark_state.phase {
            BookmarkPhase::Found => {},
            BookmarkPhase::NotSearching => {},
            BookmarkPhase::SearchingFromApi => {
                let loader = Throbber::default()
                    .label("Searching chapter bookmarked".to_span().style(*INSTRUCTIONS_STYLE))
                    .throbber_set(throbber_widgets_tui::BRAILLE_SIX)
                    .use_type(throbber_widgets_tui::WhichUse::Spin);

                StatefulWidget::render(loader, more_details_area, buf, &mut self.bookmark_state.loader);
            },
            BookmarkPhase::FailedToFetch => {
                Paragraph::new("Failed to get chapter please try again".to_span().style(*ERROR_STYLE))
                    .wrap(Wrap { trim: true })
                    .render(more_details_area, buf);
            },
            BookmarkPhase::NotFoundDatabase => {
                Paragraph::new("There is no bookmarked chapter".to_span().style(Style::new().underlined()))
                    .wrap(Wrap { trim: true })
                    .render(more_details_area, buf);
            },
        };

        match self.image_state.as_ref() {
            Some(state) => {
                let image = Image::new(state.as_ref());
                Widget::render(image, cover_area, buf);
            },
            None => {
                self.cover_area = cover_area;
                Block::bordered().render(area, buf);
            },
        }
    }

    fn render_manga_information(&mut self, area: Rect, frame: &mut Frame<'_>) {
        let buf = frame.buffer_mut();

        let layout = Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]);

        let [manga_information_area, manga_chapters_area] = layout.areas(area);

        let statistics = Span::raw(format!("Rating ⭐ {} ", self.manga.rating.ceil()));

        let authors = match &self.manga.author {
            Some(author) => format!("Author : {} ", author.name),
            None => format!(""),
        };
        let artist = match &self.manga.artist {
            Some(artist) => format!("Artist : {} ", artist.name),
            None => format!(""),
        };

        Block::bordered()
            .title_top(self.manga.title.clone())
            .title_bottom(Line::from(vec![statistics, " ".into(), Span::raw(authors), Span::raw(artist)]))
            .render(manga_information_area, buf);

        self.render_details(manga_information_area, frame.buffer_mut());

        self.render_chapters_area(manga_chapters_area, frame.buffer_mut());
    }

    fn render_details(&mut self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::vertical([Constraint::Percentage(20), Constraint::Percentage(80)]).margin(1);
        let [tags_area, description_area] = layout.areas(area);

        let [tags_area, status_area] =
            Layout::horizontal([Constraint::Percentage(80), Constraint::Percentage(20)]).areas(tags_area);

        Paragraph::new(Line::from_iter(self.manga.genres.clone()))
            .wrap(Wrap { trim: true })
            .render(tags_area, buf);

        Paragraph::new(Line::from(Span::from(self.manga.status)))
            .wrap(Wrap { trim: true })
            .render(status_area, buf);

        Paragraph::new(self.manga.description.clone())
            .wrap(Wrap { trim: true })
            .render(description_area, buf);
    }

    fn render_chapters_area(&mut self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::vertical([Constraint::Percentage(10), Constraint::Percentage(90)]).margin(2);

        let [sorting_buttons_area, chapters_area] = layout.areas(area);

        if self.download_process_started() {
            self.render_download_all_chapters_area(area, buf);
            return;
        }

        match self.chapters.as_mut() {
            Some(chapters) => {
                let tota_pages = chapters.pagination.get_total_pages();
                let page = format!("Page {} of : {}", chapters.pagination.current_page, tota_pages);
                let total = format!("Total chapters {}", chapters.pagination.total_items);

                let mut chapter_instructions = vec![
                    "Scroll Down/Up ".into(),
                    Span::raw(" <j>/<k> ").style(*INSTRUCTIONS_STYLE),
                    " Download chapter ".into(),
                    Span::raw(" <d> ").style(*INSTRUCTIONS_STYLE),
                    " Download all chapters ".into(),
                    Span::raw(" <a> ").style(*INSTRUCTIONS_STYLE),
                ];

                if self.picker.is_some() {
                    chapter_instructions.push(" Read chapter ".into());
                    chapter_instructions.push(Span::raw(" <r> ").style(*INSTRUCTIONS_STYLE));

                    chapter_instructions.push(" Read bookmark ".into());
                    chapter_instructions.push(Span::raw(" <Tab> ").style(*INSTRUCTIONS_STYLE));
                }

                let mut bottom_instructions: Vec<Span<'_>> = vec![
                    page.into(),
                    " | ".into(),
                    total.into(),
                    " Next ".into(),
                    "<w>".to_span().style(*INSTRUCTIONS_STYLE),
                    " Previous ".into(),
                    "<b>".to_span().style(*INSTRUCTIONS_STYLE),
                ];
                if !self.bookmark_state.auto_bookmark {
                    bottom_instructions.push(" Bookmark chapter ".into());
                    bottom_instructions.push("<m>".to_span().style(*INSTRUCTIONS_STYLE));
                }

                Block::bordered()
                    .title_top(Line::from(chapter_instructions))
                    .title_bottom(Line::from(bottom_instructions))
                    .render(area, buf);

                StatefulWidget::render(chapters.widget.clone(), chapters_area, buf, &mut chapters.state);

                self.render_sorting_buttons(sorting_buttons_area, buf);
            },

            None => {
                let title: Span<'_> = if self.state == PageState::ChaptersNotFound {
                    "Could not get chapters, please try again".to_span().style(*ERROR_STYLE)
                } else {
                    "Searching chapters".to_span()
                };

                Block::bordered().title(title).render(area, buf);
            },
        }
    }

    fn render_download_all_chapters_area(&mut self, area: Rect, buf: &mut Buffer) {
        StatefulWidget::render(DownloadAllChaptersWidget::new(&self.manga.title), area, buf, &mut self.download_all_chapters_state);
    }

    fn render_sorting_buttons(&mut self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]);
        let [sorting_area, language_area] = layout.areas(area);

        let order_title = format!("Order: {} ", match self.chapter_filters.order {
            ChapterOrderBy::Descending => "Descending",
            ChapterOrderBy::Ascending => "Ascending",
        });

        Paragraph::new(Line::from(vec![
            order_title.into(),
            " Change order : ".into(),
            Span::raw("<t>").style(*INSTRUCTIONS_STYLE),
        ]))
        .render(sorting_area, buf);

        let languages_list_area = Rect::new(language_area.x, language_area.y, language_area.width, language_area.height + 10);

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
                    .languages
                    .iter()
                    .map(|lang| format!("{} {}", lang.as_emoji(), lang.as_human_readable())),
            )
            .block(Block::bordered().title(instructions))
            .highlight_style(Style::default().on_blue());

            StatefulWidget::render(available_language_list, languages_list_area, buf, &mut self.available_languages_state);
        } else {
            Paragraph::new(Line::from(vec![
                "Language: ".into(),
                self.chapter_filters.language.as_emoji().into(),
                " | ".into(),
                "Available languages: ".into(),
                "<l>".bold().yellow(),
            ]))
            .render(language_area, buf);
        }
    }

    fn download_process_started(&self) -> bool {
        self.download_all_chapters_state.process_started()
    }

    fn search_by_language(&mut self) {
        self.chapters = None;
        self.chapter_filters.language = self.get_current_selected_language();
        self.search_chapters();
    }

    fn handle_key_events(&mut self, key_event: KeyEvent) {
        if self.is_list_languages_open {
            match key_event.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.local_action_tx.send(MangaPageActions::ScrollDownAvailbleLanguages).ok();
                },
                KeyCode::Char('k') | KeyCode::Up => {
                    self.local_action_tx.send(MangaPageActions::ScrollUpAvailbleLanguages).ok();
                },
                KeyCode::Enter | KeyCode::Char('s') => {
                    self.local_action_tx.send(MangaPageActions::SearchByLanguage).ok();
                },
                KeyCode::Char('l') | KeyCode::Esc => {
                    self.local_action_tx.send(MangaPageActions::ToggleAvailableLanguagesList).ok();
                },
                _ => {},
            }
        } else if self.state != PageState::SearchingChapterData {
            if self.download_process_started() {
                match key_event.code {
                    KeyCode::Esc => {
                        if self.download_all_chapters_state.phase == DownloadPhase::DownloadingChapters {
                            self.local_action_tx.send(MangaPageActions::AskAbortProcces).ok();
                        } else if self.download_all_chapters_state.phase == DownloadPhase::AskAbortProcess {
                            self.download_all_chapters_state.continue_download();
                        } else {
                            self.local_action_tx.send(MangaPageActions::CancelDownloadAll).ok();
                        }
                    },

                    KeyCode::Enter => {
                        if self.download_all_chapters_state.phase == DownloadPhase::AskAbortProcess {
                            self.local_action_tx.send(MangaPageActions::AbortDownloadAllChapters).ok();
                        } else {
                            self.local_action_tx.send(MangaPageActions::ConfirmDownloadAll).ok();
                        }
                    },

                    _ => {},
                }
            } else {
                match key_event.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        self.local_action_tx.send(MangaPageActions::ScrollChapterDown).ok();
                    },
                    KeyCode::Char('k') | KeyCode::Up => {
                        self.local_action_tx.send(MangaPageActions::ScrollChapterUp).ok();
                    },
                    KeyCode::Char('t') => {
                        self.local_action_tx.send(MangaPageActions::ToggleOrder).ok();
                    },
                    KeyCode::Char('r') | KeyCode::Enter => {
                        self.local_action_tx.send(MangaPageActions::ReadChapter).ok();
                    },
                    KeyCode::Char('d') => {
                        self.local_action_tx.send(MangaPageActions::DownloadChapter).ok();
                    },
                    KeyCode::Char('a') => {
                        self.local_action_tx.send(MangaPageActions::AskDownloadAllChapters).ok();
                    },
                    KeyCode::Char('l') => {
                        self.local_action_tx.send(MangaPageActions::ToggleAvailableLanguagesList).ok();
                    },
                    KeyCode::Char('w') => {
                        self.local_action_tx.send(MangaPageActions::SearchNextChapterPage).ok();
                    },
                    KeyCode::Char('b') => {
                        self.local_action_tx.send(MangaPageActions::SearchPreviousChapterPage).ok();
                    },
                    KeyCode::Char('m') => {
                        if !self.bookmark_state.auto_bookmark {
                            self.local_action_tx.send(MangaPageActions::BookMarkChapterSelected).ok();
                        }
                    },
                    KeyCode::Tab => {
                        self.local_action_tx.send(MangaPageActions::GoToReadBookmarkedChapter).ok();
                    },

                    _ => {},
                }
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
        self.chapter_filters.order = self.chapter_filters.order.toggle();
        self.search_chapters();
    }

    fn scroll_language_down(&mut self) {
        self.available_languages_state.select_next();
    }

    fn scroll_language_up(&mut self) {
        self.available_languages_state.select_previous();
    }

    fn toggle_available_languages_list(&mut self) {
        self.is_list_languages_open = !self.is_list_languages_open;
    }

    fn get_current_selected_chapter_mut(&mut self) -> Option<&mut ChapterItem> {
        match self.chapters.as_mut() {
            Some(chapters_data) => match chapters_data.state.selected {
                Some(selected_chapter_index) => return chapters_data.widget.chapters.get_mut(selected_chapter_index),
                None => None,
            },
            None => None,
        }
    }

    fn _get_current_selected_chapter(&self) -> Option<&ChapterItem> {
        match self.chapters.as_ref() {
            Some(chapters_data) => match chapters_data.state.selected {
                Some(selected_chapter_index) => return chapters_data.widget.chapters.get(selected_chapter_index),
                None => None,
            },
            None => None,
        }
    }

    fn read_chapter(&mut self) {
        if self.picker.is_none() {
            return;
        }
        self.state = PageState::SearchingChapterData;
        match self.get_current_selected_chapter_mut() {
            Some(chapter_selected) => {
                chapter_selected.set_normal_state();
                let id_chapter = chapter_selected.chapter.id.clone();
                let manga_id = self.manga.id.clone();
                let local_tx = self.local_event_tx.clone();
                let client = Arc::clone(&self.manga_provider);

                tokio::spawn(async move {
                    let search_chapter_response = client.read_chapter(&id_chapter, &manga_id).await;

                    match search_chapter_response {
                        Ok((chapter, manga_to_read)) => {
                            local_tx.send(MangaPageEvents::ReadSuccesful(chapter, manga_to_read)).ok();
                        },
                        Err(e) => {
                            write_to_error_log(error_log::ErrorType::Error(
                                format!(
                                    "cannot read chapter with id {} of manga with id {}, more details : {e}",
                                    id_chapter, manga_id
                                )
                                .into(),
                            ));
                            local_tx.send(MangaPageEvents::ReadError(id_chapter)).ok();
                        },
                    }
                });
            },
            None => self.state = PageState::DisplayingChapters,
        }
    }

    fn get_current_selected_language(&self) -> Languages {
        match self.available_languages_state.selected() {
            Some(index) => self.manga.languages[index],
            None => self.chapter_filters.language,
        }
    }

    fn search_next_chapters(&mut self) {
        if let Some(chapters) = self.chapters.as_mut() {
            if chapters.pagination.can_go_next_page() {
                chapters.pagination.go_next_page();
                self.search_chapters();
            }
        }
    }

    fn search_previous_chapters(&mut self) {
        if let Some(chapters) = self.chapters.as_mut() {
            if chapters.pagination.can_go_previous_page() {
                chapters.pagination.go_previous_page();
                self.search_chapters();
            }
        }
    }

    fn search_chapters(&mut self) {
        self.state = PageState::SearchingChapters;
        let manga_id = self.manga.id.clone();
        let tx = self.local_event_tx.clone();
        let client = Arc::clone(&self.manga_provider);

        let pagination =
            if let Some(chapters) = self.chapters.as_ref() { chapters.pagination.clone() } else { Pagination::default() };
        let filters = self.chapter_filters.clone();

        self.tasks.spawn(async move {
            let response = client.get_chapters(&manga_id, filters, pagination).await;

            match response {
                Ok(res) => {
                    tx.send(MangaPageEvents::LoadChapters(Some(res))).ok();
                },
                Err(e) => {
                    write_to_error_log(e.into());
                },
            }
        });
    }

    fn check_chapters_read(&mut self) {
        let binding = DBCONN.lock().unwrap();
        let conn = binding.as_ref().unwrap();
        let history = get_chapters_history_status(&self.manga.id, conn);
        match history {
            Ok(his) => {
                if let Some(chapters) = self.chapters.as_mut() {
                    for item in chapters.widget.chapters.iter_mut() {
                        let chapter_found = his.iter().find(|chap| chap.id == item.chapter.id);
                        if let Some(chapt) = chapter_found {
                            item.is_read = chapt.is_read;
                            item.is_downloaded = chapt.is_downloaded
                        }
                    }
                }
            },
            Err(e) => {
                write_to_error_log(error_log::ErrorType::Error(Box::new(e)));
            },
        }
    }

    fn clear_chapters_as_bookmarked(&mut self) {
        if let Some(chapters) = self.chapters.as_mut() {
            chapters.widget.chapters.iter_mut().for_each(|chap| chap.is_bookmarked = false);
        }
    }

    fn bookmark_current_chapter_selected(&mut self, database: &mut dyn Bookmark) {
        self.clear_chapters_as_bookmarked();
        let manga_id = self.manga.id.clone();
        let manga_title = self.manga.title.clone();
        let cover_img_url = self.manga.cover_img_url.clone();
        let chapter_language = self.get_current_selected_language();
        if let Some(chapter_selected) = self.get_current_selected_chapter_mut() {
            let chapter_to_bookmark: ChapterToBookmark = ChapterToBookmark {
                chapter_id: &chapter_selected.chapter.id,
                manga_id: &manga_id,
                chapter_title: &chapter_selected.chapter.title,
                manga_title: &manga_title,
                manga_cover_url: cover_img_url.as_deref(),
                translated_language: chapter_language,
                page_number: None,
            };

            match database.bookmark(chapter_to_bookmark) {
                Ok(()) => chapter_selected.is_bookmarked = true,
                Err(e) => write_to_error_log(ErrorType::Error(e)),
            }
        }
    }

    fn get_chapter_bookmarked_from_db(&mut self, datatabase: impl RetrieveBookmark) {
        match datatabase.get_bookmarked(&self.manga.id) {
            Ok(maybe_chapter) => match maybe_chapter {
                Some(chapter) => {
                    self.local_event_tx.send(MangaPageEvents::FetchChapterBookmarked(chapter)).ok();
                },
                None => {
                    self.bookmark_state.phase = BookmarkPhase::NotFoundDatabase;
                },
            },
            Err(e) => write_to_error_log(ErrorType::Error(e)),
        };
    }

    fn fetch_chapter_bookmarked(&mut self, bookmarked_chapter: ChapterBookmarked) {
        let sender = self.local_event_tx.clone();
        self.bookmark_state.phase = BookmarkPhase::SearchingFromApi;
        let client = Arc::clone(&self.manga_provider);

        self.tasks.spawn(async move {
            let response = client.fetch_chapter_bookmarked(bookmarked_chapter).await;

            match response {
                Ok(response) => {
                    sender.send(MangaPageEvents::ReadChapterBookmarked(response.0, response.1)).ok();
                },
                Err(e) => {
                    #[cfg(not(test))]
                    {
                        write_to_error_log(ErrorType::Error(e));
                    }
                    sender.send(MangaPageEvents::FetchBookmarkFailed).ok();
                },
            }
        });
    }

    fn download_chapter_selected(&mut self) {
        let manga_id = self.manga.id.clone();
        let manga_title = self.manga.title.clone();
        let tx = self.local_event_tx.clone();
        let client = Arc::clone(&self.manga_provider);
        let manga_tracker = self.manga_tracker.clone();

        self.state = PageState::DownloadingChapters;
        if let Some(chapter) = self.get_current_selected_chapter_mut() {
            if chapter.download_loading_state.is_some() {
                return;
            }
            chapter.set_normal_state();
            chapter.download_loading_state = Some(0.001);
            let config = MangaTuiConfig::get();

            tokio::spawn(download_single_chapter(
                client,
                manga_tracker,
                manga_id,
                manga_title,
                chapter.chapter.clone(),
                config.clone(),
                tx,
            ));
        }
    }

    fn set_chapter_finished_downloading(&mut self, chapter_id: String) {
        if let Some(chapters) = self.chapters.as_mut() {
            if let Some(chap) = chapters.widget.chapters.iter_mut().find(|chap| chap.chapter.id == chapter_id) {
                chap.download_loading_state = None;
                self.local_event_tx.send(MangaPageEvents::CheckChapterStatus).ok();
            }
        }
    }

    fn save_download_status(&mut self, id_chapter: String, title: String) {
        let binding = DBCONN.lock().unwrap();
        let conn = binding.as_ref().unwrap();

        let save_download_operation = set_chapter_downloaded(
            SetChapterDownloaded {
                id: &id_chapter,
                title: &title,
                manga_id: &self.manga.id,
                manga_title: &self.manga.title,
                img_url: self.manga.cover_img_url.as_deref(),
            },
            conn,
        );

        if let Err(e) = save_download_operation {
            write_to_error_log(error_log::ErrorType::Error(Box::new(e)));
        }
    }

    fn set_download_progress_for_chapter(&mut self, progress: f64, id_chapter: String) {
        if let Some(chapters) = self.chapters.as_mut() {
            if let Some(chap) = chapters.widget.chapters.iter_mut().find(|chap| chap.chapter.id == id_chapter) {
                chap.download_loading_state = Some(progress);
            }
        }
    }

    fn set_chapter_download_error(&mut self, chapter_id: String) {
        if let Some(chapters) = self.chapters.as_mut() {
            if let Some(chapter) = chapters.widget.chapters.iter_mut().find(|chap| chap.chapter.id == chapter_id) {
                chapter.set_download_error();
            }
        }
    }

    fn set_chapter_read_error(&mut self, chapter_id: String) {
        self.state = PageState::DisplayingChapters;
        if let Some(chapters) = self.chapters.as_mut() {
            if let Some(chapter) = chapters.widget.chapters.iter_mut().find(|chap| chap.chapter.id == chapter_id) {
                chapter.set_read_error();
            }
        }
    }

    fn load_chapters(&mut self, response: Option<GetChaptersResponse>) {
        self.state = PageState::DisplayingChapters;
        match response {
            Some(response) => {
                let mut list_state = tui_widget_list::ListState::default();

                list_state.select(Some(0));

                let chapter_widget = ChaptersListWidget::from_response(response.chapters);

                let chapters = match &self.chapters {
                    Some(chapters) => Some(ChaptersData {
                        state: list_state,
                        widget: chapter_widget,
                        pagination: chapters.pagination.clone(),
                    }),
                    None => Some(ChaptersData {
                        state: list_state,
                        widget: chapter_widget,
                        pagination: Pagination::from_total_items(response.total_chapters),
                    }),
                };

                self.chapters = chapters;
                self.local_event_tx.send(MangaPageEvents::CheckChapterStatus).ok();
            },
            None => {
                self.state = PageState::ChaptersNotFound;
                self.chapters = None;
            },
        }
    }

    fn set_manga_download_progress(&mut self) {
        self.download_all_chapters_state.set_download_progress();
    }

    fn ask_download_all_chapters(&mut self) {
        self.download_all_chapters_state.ask_for_confirmation();
    }

    fn confirm_download_all_chapters(&mut self) {
        self.download_all_chapters_state.fetch_chapters_data();
        let manga_id = self.manga.id.clone();
        let manga_title = self.manga.title.clone();
        let lang = self.get_current_selected_language();
        let tx = self.local_event_tx.clone();
        let client = Arc::clone(&self.manga_provider);
        let config = MangaTuiConfig::get();
        self.tasks
            .spawn(download_all_chapters(client, manga_id, manga_title, lang, config.clone(), tx));
    }

    fn cancel_download_all_chapters(&mut self) {
        if !self.download_all_chapters_state.is_downloading() {
            self.state = PageState::DisplayingChapters;
            self.download_all_chapters_state.cancel();
        }
    }

    fn start_download_all_chapters(&mut self, total_chapters: f64) {
        self.download_all_chapters_state.start_download();
        self.download_all_chapters_state.set_total_chapters(total_chapters);
        self.download_all_chapters_state
            .set_download_location(AppDirectories::MangaDownloads.get_full_path().join(&self.manga.title));
    }

    pub fn is_downloading_all_chapters(&self) -> bool {
        self.download_all_chapters_state.is_downloading()
    }

    fn finish_download_all_chapters(&mut self) {
        self.download_all_chapters_state.reset();
        self.state = PageState::DisplayingChapters;
        self.local_event_tx.send(MangaPageEvents::CheckChapterStatus).ok();
    }

    fn ask_abort_download_chapters(&mut self) {
        self.download_all_chapters_state.ask_abort_proccess();
    }

    fn abort_download_all_chapters(&mut self) {
        self.download_all_chapters_state.abort_proccess();
        self.tasks.abort_all();
        self.local_event_tx.send(MangaPageEvents::CheckChapterStatus).ok();
    }

    fn set_download_all_chapters_error(&mut self) {
        self.download_all_chapters_state.set_download_error();
    }

    fn search_cover(&mut self) {
        if self.picker.is_none() {
            return;
        }
        let tx = self.local_event_tx.clone();
        let client = Arc::clone(&self.manga_provider);
        let file_name = self.manga.cover_img_url_lower_quality.as_ref().cloned();

        if let Some(url) = file_name {
            self.tasks.spawn(async move {
                let response = client.get_image(&url).await;
                match response {
                    Ok(res) => {
                        tx.send(MangaPageEvents::LoadCover(res)).ok();
                    },
                    Err(e) => {
                        write_to_error_log(e.into());
                    },
                }
            });
        }
    }

    fn load_cover(&mut self, img: DynamicImage) {
        let fixed_protocol = self.picker.as_mut().unwrap().new_protocol(img, self.cover_area, Resize::Fit(None));
        if let Ok(protocol) = fixed_protocol {
            self.image_state = Some(protocol);
        }
    }

    fn handle_mouse_events(&mut self, mouse_event: MouseEvent) {
        if self.is_list_languages_open {
            match mouse_event.kind {
                MouseEventKind::ScrollUp => {
                    self.local_action_tx.send(MangaPageActions::ScrollUpAvailbleLanguages).ok();
                },
                MouseEventKind::ScrollDown => {
                    self.local_action_tx.send(MangaPageActions::ScrollDownAvailbleLanguages).ok();
                },
                _ => {},
            }
        } else {
            match mouse_event.kind {
                MouseEventKind::ScrollUp => {
                    self.local_action_tx.send(MangaPageActions::ScrollChapterUp).ok();
                },
                MouseEventKind::ScrollDown => {
                    self.local_action_tx.send(MangaPageActions::ScrollChapterDown).ok();
                },
                _ => {},
            }
        }
    }

    fn fetch_bookmarked_chapter_failed(&mut self) {
        self.bookmark_state.phase = BookmarkPhase::FailedToFetch;
    }

    fn mark_chapter_as_read_in_db(&self, chapter: &ChapterToRead) {
        let connection = Database::get_connection();

        if let Ok(conn) = connection {
            let save_read_history_result = save_history(
                MangaReadingHistorySave {
                    id: &self.manga.id,
                    title: &self.manga.title,
                    img_url: self.manga.cover_img_url.as_deref(),
                    chapter: ChapterToSaveHistory {
                        id: &chapter.id,
                        title: &chapter.title,
                        translated_language: chapter.language.as_iso_code(),
                    },
                },
                &conn,
            );

            if let Err(e) = save_read_history_result {
                write_to_error_log(format!("error saving reading history in local db: {e}").into());
            }
        }
    }

    fn read_chapter_bookmarked(&mut self, chapter: ChapterToRead, list_of_chapters: ListOfChapters) {
        self.bookmark_state.phase = BookmarkPhase::default();
        self.mark_chapter_as_read_in_db(&chapter);

        self.global_event_tx
            .as_ref()
            .unwrap()
            .send(Events::ReadChapter(chapter, MangaToRead {
                title: self.manga.title.clone(),
                manga_id: self.manga.id.clone(),
                list: list_of_chapters,
            }))
            .ok();
    }

    fn track_manga(&self, tracker: Option<S>, manga_title: String, chapter_number: u32, volume_number: Option<u32>) {
        let tx = self.local_event_tx.clone();
        track_manga(tracker, manga_title, chapter_number, volume_number, move |error| {
            tx.send(MangaPageEvents::TrackingFailed(error)).ok();
        });
    }

    fn log_tracking_manga_error(&self, message: String) {
        write_to_error_log(format_error_message_tracking_reading_history("", self.manga.title.clone(), message).into());
    }

    fn tick(&mut self) {
        if self.download_process_started() {
            self.download_all_chapters_state.tick();
        } else if self.bookmark_state.phase == BookmarkPhase::SearchingFromApi {
            self.bookmark_state.loader.calc_next();
        }

        while let Ok(background_event) = self.local_event_rx.try_recv() {
            match background_event {
                MangaPageEvents::TrackingFailed(error_message) => self.log_tracking_manga_error(error_message),
                MangaPageEvents::ReadChapterBookmarked(chapter, list) => self.read_chapter_bookmarked(chapter, list),
                MangaPageEvents::FetchBookmarkFailed => self.fetch_bookmarked_chapter_failed(),
                MangaPageEvents::FetchChapterBookmarked(chapter_bookmarked) => {
                    self.fetch_chapter_bookmarked(chapter_bookmarked);
                },
                MangaPageEvents::LoadCover(img) => self.load_cover(img),
                MangaPageEvents::SearchCover => self.search_cover(),
                MangaPageEvents::FinishedDownloadingAllChapters => self.finish_download_all_chapters(),
                MangaPageEvents::DownloadAllChaptersError => self.set_download_all_chapters_error(),
                MangaPageEvents::StartDownloadProgress(total_chapters) => self.start_download_all_chapters(total_chapters),
                MangaPageEvents::SetDownloadAllChaptersProgress => self.set_manga_download_progress(),
                MangaPageEvents::ReadError(chapter_id) => {
                    self.set_chapter_read_error(chapter_id);
                },
                MangaPageEvents::DownloadError(chapter_id) => self.set_chapter_download_error(chapter_id),
                MangaPageEvents::SetDownloadProgress(progress, id_chapter) => {
                    self.set_download_progress_for_chapter(progress, id_chapter)
                },
                MangaPageEvents::SaveChapterDownloadStatus(id_chapter, title) => self.save_download_status(id_chapter, title),
                MangaPageEvents::ChapterFinishedDownloading(id_chapter) => self.set_chapter_finished_downloading(id_chapter),
                MangaPageEvents::SearchChapters => self.search_chapters(),
                MangaPageEvents::LoadChapters(response) => self.load_chapters(response),
                MangaPageEvents::CheckChapterStatus => {
                    self.check_chapters_read();
                },

                MangaPageEvents::ReadSuccesful(chapter_to_read, list) => {
                    self.state = PageState::DisplayingChapters;
                    let volume = chapter_to_read.clone().volume_number.and_then(|vol| vol.parse::<u32>().ok());
                    self.mark_chapter_as_read_in_db(&chapter_to_read);
                    self.track_manga(self.manga_tracker.clone(), self.manga.title.clone(), chapter_to_read.number as u32, volume);

                    self.local_event_tx.send(MangaPageEvents::CheckChapterStatus).ok();

                    self.global_event_tx
                        .as_ref()
                        .unwrap()
                        .send(Events::ReadChapter(chapter_to_read, MangaToRead {
                            title: self.manga.title.clone(),
                            manga_id: self.manga.id.clone(),
                            list,
                        }))
                        .ok();
                },
            }
        }
    }

    #[cfg(test)]
    fn get_index_chapter_selected(&self) -> usize {
        self.chapters.as_ref().unwrap().state.selected.unwrap()
    }

    #[cfg(test)]
    fn get_chapter_data(&self) -> ChaptersData {
        self.chapters.as_ref().cloned().unwrap()
    }

    #[cfg(test)]
    pub fn start_downloading_all_chapters(&mut self) {
        self.start_download_all_chapters(10.0);
    }
}

impl<T, S> Component for MangaPage<T, S>
where
    T: MangaPageProvider,
    S: MangaTracker,
{
    type Actions = MangaPageActions;

    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(15), Constraint::Percentage(85)]);

        let [cover_area, information_area] = layout.areas(area);

        self.render_cover(cover_area, frame.buffer_mut());
        self.render_manga_information(information_area, frame);
    }

    fn update(&mut self, action: Self::Actions) {
        match action {
            MangaPageActions::GoToReadBookmarkedChapter => {
                let connection = Database::get_connection();
                if let Ok(conn) = connection {
                    let database = Database::new(&conn);

                    self.get_chapter_bookmarked_from_db(database);
                }
            },
            MangaPageActions::BookMarkChapterSelected => {
                let connection = Database::get_connection();

                if let Ok(conn) = connection {
                    let mut database = Database::new(&conn);

                    self.bookmark_current_chapter_selected(&mut database);
                }
            },
            MangaPageActions::AbortDownloadAllChapters => self.abort_download_all_chapters(),
            MangaPageActions::AskAbortProcces => self.ask_abort_download_chapters(),
            MangaPageActions::SearchByLanguage => self.search_by_language(),
            MangaPageActions::CancelDownloadAll => self.cancel_download_all_chapters(),
            MangaPageActions::AskDownloadAllChapters => self.ask_download_all_chapters(),
            MangaPageActions::ConfirmDownloadAll => self.confirm_download_all_chapters(),
            MangaPageActions::SearchPreviousChapterPage => self.search_previous_chapters(),
            MangaPageActions::SearchNextChapterPage => self.search_next_chapters(),
            MangaPageActions::ScrollDownAvailbleLanguages => self.scroll_language_down(),
            MangaPageActions::ScrollUpAvailbleLanguages => self.scroll_language_up(),
            MangaPageActions::ToggleAvailableLanguagesList => self.toggle_available_languages_list(),
            MangaPageActions::ScrollChapterUp => self.scroll_chapter_up(),
            MangaPageActions::ScrollChapterDown => self.scroll_chapter_down(),
            MangaPageActions::ToggleOrder => {
                if self.state != PageState::SearchingChapters {
                    self.toggle_chapter_order()
                }
            },
            MangaPageActions::ReadChapter => {
                if self.state != PageState::SearchingChapterData {
                    self.read_chapter();
                }
            },

            MangaPageActions::DownloadChapter => self.download_chapter_selected(),
        }
    }

    fn handle_events(&mut self, events: Events) {
        match events {
            Events::Key(key_event) => self.handle_key_events(key_event),
            Events::Mouse(mouse_event) => self.handle_mouse_events(mouse_event),
            _ => self.tick(),
        }
    }

    fn clean_up(&mut self) {
        self.abort_tasks();
        self.manga.genres = vec![];
        self.manga.description = String::new();
    }
}

#[cfg(test)]
mod test {
    //use std::time::Duration;
    //
    //use pretty_assertions::assert_eq;
    //use tokio::time::timeout;
    //
    //use self::mpsc::unbounded_channel;
    //use super::*;
    //use crate::backend::api_responses::ChapterData;
    //use crate::backend::database::ChapterBookmarked;
    //use crate::backend::tracker::MangaTracker;
    //use crate::global::test_utils::TrackerTest;
    //use crate::view::widgets::press_key;
    //
    //fn get_chapters_response() -> ChapterResponse {
    //    ChapterResponse {
    //        data: vec![ChapterData::default(), ChapterData::default(), ChapterData::default()],
    //        total: 30,
    //        ..Default::default()
    //    }
    //}
    //
    //fn render_chapters<T: MangaTracker>(manga_page: &mut MangaPage<T>) {
    //    let area = Rect::new(0, 0, 50, 50);
    //    let mut buf = Buffer::empty(area);
    //    let chapters = manga_page.chapters.as_mut().unwrap();
    //    StatefulWidget::render(chapters.widget.clone(), area, &mut buf, &mut chapters.state);
    //}
    //
    //fn render_available_languages_list<T: MangaTracker>(manga_page: &mut MangaPage<T>) {
    //    let area = Rect::new(0, 0, 50, 50);
    //    let mut buf = Buffer::empty(area);
    //    let languages: Vec<String> = manga_page.manga.available_languages.iter().map(|lang| lang.as_human_readable()).collect();
    //    let list = List::new(languages);
    //
    //    StatefulWidget::render(list, area, &mut buf, &mut manga_page.available_languages_state);
    //}
    //
    //#[tokio::test]
    //async fn key_events_trigger_expected_actions() {
    //    let mut manga_page: MangaPage<TrackerTest> = MangaPage::new(Manga::default(), None);
    //
    //    // Scroll down chapters list
    //    press_key(&mut manga_page, KeyCode::Char('j'));
    //    let action = manga_page.local_action_rx.recv().await.unwrap();
    //
    //    assert_eq!(MangaPageActions::ScrollChapterDown, action);
    //
    //    // Scroll up chapters list
    //    press_key(&mut manga_page, KeyCode::Char('k'));
    //    let action = manga_page.local_action_rx.recv().await.unwrap();
    //
    //    assert_eq!(MangaPageActions::ScrollChapterUp, action);
    //
    //    // toggle chapter order
    //    press_key(&mut manga_page, KeyCode::Char('t'));
    //    let action = manga_page.local_action_rx.recv().await.unwrap();
    //
    //    assert_eq!(MangaPageActions::ToggleOrder, action);
    //
    //    // Go next chapter page
    //    press_key(&mut manga_page, KeyCode::Char('w'));
    //    let action = manga_page.local_action_rx.recv().await.unwrap();
    //
    //    assert_eq!(MangaPageActions::SearchNextChapterPage, action);
    //
    //    // Go previous chapter page
    //    press_key(&mut manga_page, KeyCode::Char('b'));
    //    let action = manga_page.local_action_rx.recv().await.unwrap();
    //
    //    assert_eq!(MangaPageActions::SearchPreviousChapterPage, action);
    //
    //    // Open available_languages list
    //    press_key(&mut manga_page, KeyCode::Char('l'));
    //    let action = manga_page.local_action_rx.recv().await.unwrap();
    //
    //    assert_eq!(MangaPageActions::ToggleAvailableLanguagesList, action);
    //
    //    manga_page.toggle_available_languages_list();
    //
    //    // scroll down available languages list
    //    press_key(&mut manga_page, KeyCode::Char('j'));
    //    let action = manga_page.local_action_rx.recv().await.unwrap();
    //
    //    assert_eq!(MangaPageActions::ScrollDownAvailbleLanguages, action);
    //
    //    // scroll down available languages list
    //    press_key(&mut manga_page, KeyCode::Char('k'));
    //    let action = manga_page.local_action_rx.recv().await.unwrap();
    //
    //    assert_eq!(MangaPageActions::ScrollUpAvailbleLanguages, action);
    //
    //    // search by a language selected
    //    press_key(&mut manga_page, KeyCode::Char('s'));
    //    let action = manga_page.local_action_rx.recv().await.unwrap();
    //
    //    assert_eq!(MangaPageActions::SearchByLanguage, action);
    //
    //    // close available languages list
    //    press_key(&mut manga_page, KeyCode::Esc);
    //    let action = manga_page.local_action_rx.recv().await.unwrap();
    //
    //    assert_eq!(MangaPageActions::ToggleAvailableLanguagesList, action);
    //
    //    manga_page.toggle_available_languages_list();
    //
    //    // download chapter
    //    press_key(&mut manga_page, KeyCode::Char('d'));
    //    let action = manga_page.local_action_rx.recv().await.unwrap();
    //
    //    assert_eq!(MangaPageActions::DownloadChapter, action);
    //
    //    // start download all chapter proccess
    //    press_key(&mut manga_page, KeyCode::Char('a'));
    //    let action = manga_page.local_action_rx.recv().await.unwrap();
    //
    //    assert_eq!(MangaPageActions::AskDownloadAllChapters, action);
    //
    //    manga_page.ask_download_all_chapters();
    //
    //    // confirm download all chapters
    //    press_key(&mut manga_page, KeyCode::Enter);
    //    let action = manga_page.local_action_rx.recv().await.unwrap();
    //
    //    assert_eq!(MangaPageActions::ConfirmDownloadAll, action);
    //
    //    manga_page.confirm_download_all_chapters();
    //
    //    // cancel download all chapters operation
    //    press_key(&mut manga_page, KeyCode::Esc);
    //    let action = manga_page.local_action_rx.recv().await.unwrap();
    //
    //    assert_eq!(MangaPageActions::CancelDownloadAll, action);
    //
    //    manga_page.cancel_download_all_chapters();
    //
    //    // read a chapter
    //    press_key(&mut manga_page, KeyCode::Char('r'));
    //    let action = manga_page.local_action_rx.recv().await.unwrap();
    //
    //    assert_eq!(MangaPageActions::ReadChapter, action);
    //
    //    // see more about author
    //    press_key(&mut manga_page, KeyCode::Char('c'));
    //    let action = manga_page.local_action_rx.recv().await.unwrap();
    //
    //    assert_eq!(MangaPageActions::GoMangasAuthor, action);
    //
    //    // see more about artist
    //    press_key(&mut manga_page, KeyCode::Char('v'));
    //    let action = manga_page.local_action_rx.recv().await.unwrap();
    //
    //    assert_eq!(MangaPageActions::GoMangasArtist, action);
    //}
    //
    //#[tokio::test]
    //async fn listen_to_key_events_based_on_conditions() {
    //    let mut manga_page: MangaPage<TrackerTest> = MangaPage::new(Manga::default(), None);
    //
    //    assert!(!manga_page.is_list_languages_open);
    //    manga_page.handle_events(Events::Key(KeyCode::Char('j').into()));
    //
    //    let action = manga_page.local_action_rx.recv().await.unwrap();
    //
    //    assert_eq!(MangaPageActions::ScrollChapterDown, action);
    //
    //    manga_page.toggle_available_languages_list();
    //    manga_page.handle_events(Events::Key(KeyCode::Char('j').into()));
    //
    //    let action = manga_page.local_action_rx.recv().await.unwrap();
    //
    //    assert_eq!(MangaPageActions::ScrollDownAvailbleLanguages, action);
    //
    //    manga_page.toggle_available_languages_list();
    //    manga_page.ask_download_all_chapters();
    //
    //    manga_page.handle_events(Events::Key(KeyCode::Enter.into()));
    //
    //    let action = manga_page.local_action_rx.recv().await.unwrap();
    //
    //    assert_eq!(MangaPageActions::ConfirmDownloadAll, action);
    //}
    //
    //async fn manga_page_initialized_correctly<T: MangaTracker>(manga_page: &mut MangaPage<T>) {
    //    assert_eq!(manga_page.chapter_language, Languages::default());
    //
    //    assert_eq!(ChapterOrder::default(), manga_page.chapter_order);
    //
    //    assert_eq!(PageState::SearchingChapters, manga_page.state);
    //
    //    assert!(!manga_page.is_list_languages_open);
    //
    //    let first_event = manga_page.local_event_rx.recv().await.unwrap();
    //    let second_event = manga_page.local_event_rx.recv().await.unwrap();
    //
    //    assert!(first_event == MangaPageEvents::FethStatistics || first_event == MangaPageEvents::SearchChapters);
    //    assert!(second_event == MangaPageEvents::FethStatistics || second_event == MangaPageEvents::SearchChapters);
    //}
    //
    //#[tokio::test]
    //async fn handle_events() {
    //    let mut manga_page: MangaPage<TrackerTest> = MangaPage::new(Manga::default(), None);
    //
    //    manga_page_initialized_correctly(&mut manga_page).await;
    //}
    //
    //#[tokio::test]
    //async fn handle_key_events() {
    //    let mut manga_page: MangaPage<TrackerTest> = MangaPage::new(Manga::default(), None);
    //    manga_page.state = PageState::SearchingChapters;
    //    manga_page.manga.available_languages =
    //        vec![Languages::default(), Languages::Spanish, Languages::German, Languages::Japanese];
    //
    //    assert_eq!(ChapterOrder::default(), manga_page.chapter_order);
    //
    //    let action = MangaPageActions::ToggleOrder;
    //    manga_page.update(action);
    //
    //    // when searching chapters avoid triggering another search by toggling order
    //    assert_eq!(ChapterOrder::default(), manga_page.chapter_order);
    //
    //    manga_page.state = PageState::DisplayingChapters;
    //    manga_page.load_chapters(Some(get_chapters_response()));
    //    render_chapters(&mut manga_page);
    //
    //    let action = MangaPageActions::ToggleOrder;
    //    manga_page.update(action);
    //
    //    assert_eq!(ChapterOrder::Ascending, manga_page.chapter_order);
    //
    //    let action = MangaPageActions::ScrollChapterDown;
    //    manga_page.update(action);
    //
    //    assert_eq!(1, manga_page.get_index_chapter_selected());
    //
    //    let action = MangaPageActions::ScrollChapterUp;
    //    manga_page.update(action);
    //
    //    assert_eq!(0, manga_page.get_index_chapter_selected());
    //
    //    let action = MangaPageActions::SearchNextChapterPage;
    //    manga_page.update(action);
    //
    //    assert_eq!(2, manga_page.get_chapter_data().page);
    //
    //    let action = MangaPageActions::SearchPreviousChapterPage;
    //    manga_page.update(action);
    //
    //    assert_eq!(1, manga_page.get_chapter_data().page);
    //
    //    let action = MangaPageActions::ToggleAvailableLanguagesList;
    //    manga_page.update(action);
    //
    //    assert!(manga_page.is_list_languages_open);
    //
    //    let action = MangaPageActions::ScrollUpAvailbleLanguages;
    //    manga_page.update(action);
    //
    //    render_available_languages_list(&mut manga_page);
    //
    //    assert_eq!(3, manga_page.available_languages_state.selected().unwrap());
    //
    //    manga_page.available_languages_state.select(Some(1));
    //
    //    let action = MangaPageActions::ScrollDownAvailbleLanguages;
    //    manga_page.update(action);
    //
    //    assert_eq!(2, manga_page.available_languages_state.selected().unwrap());
    //
    //    let action = MangaPageActions::SearchByLanguage;
    //    manga_page.update(action);
    //
    //    assert_eq!(PageState::SearchingChapters, manga_page.state);
    //    assert!(manga_page.chapters.is_none());
    //
    //    let action = MangaPageActions::ToggleAvailableLanguagesList;
    //    manga_page.update(action);
    //
    //    assert!(!manga_page.is_list_languages_open);
    //
    //    let action = MangaPageActions::AskDownloadAllChapters;
    //    manga_page.update(action);
    //
    //    assert!(manga_page.download_process_started());
    //
    //    let action = MangaPageActions::ConfirmDownloadAll;
    //    manga_page.update(action);
    //
    //    assert_eq!(DownloadPhase::FetchingChaptersData, manga_page.download_all_chapters_state.phase);
    //
    //    let action = MangaPageActions::CancelDownloadAll;
    //    manga_page.update(action);
    //
    //    assert!(!manga_page.download_process_started());
    //}
    //
    //#[test]
    //fn doesnt_go_to_reader_if_picker_is_none() {
    //    let mut manga_page: MangaPage<TrackerTest> = MangaPage::new(Manga::default(), None);
    //
    //    manga_page.load_chapters(Some(get_chapters_response()));
    //
    //    render_chapters(&mut manga_page);
    //
    //    manga_page.scroll_chapter_down();
    //
    //    manga_page.read_chapter();
    //}
    //
    //#[test]
    //fn doesnt_search_manga_cover_if_picker_is_none() {
    //    let mut manga_page: MangaPage<TrackerTest> = MangaPage::new(Manga::default(), None);
    //
    //    manga_page.search_cover();
    //}
    //
    //#[derive(Default, Clone)]
    //struct ChapterTest {
    //    id: String,
    //    was_bookmarked: bool,
    //}
    //
    //#[derive(Default, Clone)]
    //struct TestDatabase {
    //    should_fail: bool,
    //    chapter: ChapterTest,
    //    chapter_bookmarked: Option<ChapterBookmarked>,
    //}
    //
    //impl TestDatabase {
    //    fn new() -> Self {
    //        Self {
    //            should_fail: false,
    //            chapter: ChapterTest::default(),
    //            chapter_bookmarked: None,
    //        }
    //    }
    //
    //    fn with_bookmarked_chapter(chapter: ChapterBookmarked) -> Self {
    //        Self {
    //            should_fail: false,
    //            chapter: ChapterTest::default(),
    //            chapter_bookmarked: Some(chapter),
    //        }
    //    }
    //
    //    fn was_bookmarked(&self) -> bool {
    //        self.chapter.was_bookmarked
    //    }
    //}
    //
    //impl Bookmark for TestDatabase {
    //    fn bookmark(&mut self, _chapter_to_bookmark: ChapterToBookmark) -> Result<(), Box<dyn std::error::Error>> {
    //        self.chapter.was_bookmarked = true;
    //        Ok(())
    //    }
    //}
    //
    //impl RetrieveBookmark for TestDatabase {
    //    fn get_bookmarked(&self, _manga_id: &str) -> Result<Option<ChapterBookmarked>, Box<dyn std::error::Error>> {
    //        Ok(self.chapter_bookmarked.clone())
    //    }
    //}
    //
    //#[tokio::test]
    //async fn it_sends_event_to_bookmark_currently_selected_chapter_on_key_press_if_auto_bookmark_is_false() {
    //    let mut manga_page: MangaPage<TrackerTest> = MangaPage::new(Manga::default(), None);
    //    manga_page.handle_events(Events::Key(KeyCode::Char('m').into()));
    //
    //    let result = timeout(Duration::from_millis(250), manga_page.local_action_rx.recv())
    //        .await
    //        .unwrap()
    //        .unwrap();
    //
    //    assert_eq!(MangaPageActions::BookMarkChapterSelected, result)
    //}
    //
    //#[tokio::test]
    //async fn it_does_not_send_event_bookmark_chapter_selected_if_auto_bookmark_is_true() {
    //    let mut manga_page: MangaPage<TrackerTest> = MangaPage::new(Manga::default(), None).auto_bookmark(true);
    //    manga_page.handle_events(Events::Key(KeyCode::Char('m').into()));
    //
    //    assert!(manga_page.local_action_rx.is_empty());
    //}
    //
    //#[test]
    //fn it_bookmarks_currently_selected_chapter() {
    //    let mut manga_page: MangaPage<TrackerTest> = MangaPage::new(Manga::default(), None);
    //
    //    let chapter_to_bookmark: ChapterItem = ChapterItem {
    //        id: "id_chapter_bookmarked".to_string(),
    //        ..Default::default()
    //    };
    //
    //    let mut list_state = tui_widget_list::ListState::default();
    //
    //    list_state.select(Some(0));
    //
    //    manga_page.chapters = Some(ChaptersData {
    //        widget: ChaptersListWidget {
    //            chapters: vec![chapter_to_bookmark, ChapterItem::default()],
    //        },
    //        state: list_state,
    //        ..Default::default()
    //    });
    //
    //    let mut test_database = TestDatabase::new();
    //
    //    manga_page.bookmark_current_chapter_selected(&mut test_database);
    //
    //    let bookmarked_chapter = manga_page
    //        .chapters
    //        .as_ref()
    //        .unwrap()
    //        .widget
    //        .chapters
    //        .iter()
    //        .find(|chap| chap.id == "id_chapter_bookmarked")
    //        .unwrap();
    //
    //    assert!(bookmarked_chapter.is_bookmarked);
    //}
    //
    //#[test]
    //fn it_only_bookmarks_one_chapter_at_a_time() {
    //    let mut manga_page: MangaPage<TrackerTest> = MangaPage::new(Manga::default(), None);
    //
    //    let mut list_state = tui_widget_list::ListState::default();
    //
    //    list_state.select(Some(0));
    //
    //    manga_page.chapters = Some(ChaptersData {
    //        widget: ChaptersListWidget {
    //            chapters: vec![
    //                ChapterItem {
    //                    is_bookmarked: true,
    //                    ..Default::default()
    //                },
    //                ChapterItem {
    //                    is_bookmarked: true,
    //                    ..Default::default()
    //                },
    //            ],
    //        },
    //        state: list_state,
    //        ..Default::default()
    //    });
    //
    //    let mut test_database = TestDatabase::new();
    //
    //    manga_page.bookmark_current_chapter_selected(&mut test_database);
    //
    //    let chapters = manga_page.get_chapter_data();
    //
    //    assert!(chapters.widget.chapters[0].is_bookmarked);
    //    assert!(!chapters.widget.chapters[1].is_bookmarked);
    //}
    //
    //// clear all the events from initialization
    //fn flush_events<T: MangaTracker>(manga_page: &mut MangaPage<T>) {
    //    while manga_page.local_event_rx.try_recv().is_ok() {}
    //}
    //
    //#[tokio::test]
    //async fn it_sends_event_to_fetch_chapter_bookmarked_if_there_is_any() {
    //    let mut manga_page: MangaPage<TrackerTest> = MangaPage::new(Manga::default(), None);
    //
    //    flush_events(&mut manga_page);
    //
    //    let expected = ChapterBookmarked {
    //        id: "chapter_bookmarked".to_string(),
    //        ..Default::default()
    //    };
    //
    //    let test_database = TestDatabase::with_bookmarked_chapter(expected.clone());
    //
    //    let expected = MangaPageEvents::FetchChapterBookmarked(expected);
    //
    //    manga_page.get_chapter_bookmarked_from_db(test_database);
    //
    //    let result = timeout(Duration::from_millis(250), manga_page.local_event_rx.recv())
    //        .await
    //        .unwrap()
    //        .unwrap();
    //
    //    assert_eq!(expected, result);
    //}
    //
    //#[test]
    //fn it_is_set_as_bookmark_not_found_when_no_chapter_is_bookmarked() {
    //    let mut manga_page: MangaPage<TrackerTest> = MangaPage::new(Manga::default(), None);
    //
    //    flush_events(&mut manga_page);
    //
    //    let test_database = TestDatabase::new();
    //
    //    manga_page.get_chapter_bookmarked_from_db(test_database);
    //
    //    assert_eq!(manga_page.bookmark_state.phase, BookmarkPhase::NotFoundDatabase);
    //}
    //
    //#[derive(Clone)]
    //struct TestApiClient {
    //    should_fail: bool,
    //    response: (ChapterToRead, MangaToRead),
    //}
    //
    //impl TestApiClient {
    //    pub fn with_response(response: (ChapterToRead, MangaToRead)) -> Self {
    //        Self {
    //            should_fail: false,
    //            response,
    //        }
    //    }
    //
    //    pub fn with_failing_response() -> Self {
    //        Self {
    //            should_fail: true,
    //            response: (Default::default(), Default::default()),
    //        }
    //    }
    //}
    //
    //impl FetchChapterBookmarked for TestApiClient {
    //    async fn fetch_chapter_bookmarked(
    //        &self,
    //        _chapter: ChapterBookmarked,
    //    ) -> Result<(ChapterToRead, MangaToRead), Box<dyn Error>> {
    //        if self.should_fail {
    //            return Err("should fail".into());
    //        }
    //        Ok(self.response.clone())
    //    }
    //}
    //
    //#[tokio::test]
    //async fn it_send_event_to_read_bookmark_chapter_by_pressing_tab() {
    //    let mut manga_page: MangaPage<TrackerTest> = MangaPage::new(Manga::default(), None);
    //    manga_page.handle_events(Events::Key(KeyCode::Tab.into()));
    //
    //    let expected = MangaPageActions::GoToReadBookmarkedChapter;
    //
    //    let result = timeout(Duration::from_millis(250), manga_page.local_action_rx.recv())
    //        .await
    //        .unwrap()
    //        .unwrap();
    //
    //    assert_eq!(expected, result)
    //}
    //
    //#[tokio::test]
    //async fn it_sends_event_to_go_reader_page_from_bookmarked_chapter() {
    //    let (tx, _) = unbounded_channel();
    //    let mut manga_page: MangaPage<TrackerTest> = MangaPage::new(Manga::default(), None).with_global_sender(tx);
    //
    //    flush_events(&mut manga_page);
    //    let chapter_bookmarked: ChapterBookmarked = ChapterBookmarked {
    //        id: "bookmarked".to_string(),
    //        ..Default::default()
    //    };
    //
    //    let response = (
    //        ChapterToRead {
    //            id: "bookmarked".to_string(),
    //            ..Default::default()
    //        },
    //        MangaToRead::default(),
    //    );
    //
    //    let expected = MangaPageEvents::ReadChapterBookmarked(response.0.clone(), response.1.clone());
    //
    //    let api_client = TestApiClient::with_response(response);
    //
    //    manga_page.fetch_chapter_bookmarked(chapter_bookmarked, api_client);
    //
    //    let result = timeout(Duration::from_millis(250), manga_page.local_event_rx.recv())
    //        .await
    //        .unwrap()
    //        .unwrap();
    //
    //    assert_eq!(manga_page.bookmark_state.phase, BookmarkPhase::SearchingFromApi);
    //    assert_eq!(expected, result)
    //}
    //
    //#[tokio::test]
    //async fn it_sends_event_chapter_bookmarked_failed_to_fetch() {
    //    let (tx, _) = unbounded_channel();
    //    let mut manga_page: MangaPage<TrackerTest> = MangaPage::new(Manga::default(), None).with_global_sender(tx);
    //
    //    flush_events(&mut manga_page);
    //
    //    let api_client = TestApiClient::with_failing_response();
    //
    //    manga_page.fetch_chapter_bookmarked(ChapterBookmarked::default(), api_client);
    //
    //    let expected = MangaPageEvents::FetchBookmarkFailed;
    //
    //    let result = timeout(Duration::from_millis(250), manga_page.local_event_rx.recv())
    //        .await
    //        .unwrap()
    //        .unwrap();
    //
    //    assert_eq!(expected, result);
    //}
    //
    //#[tokio::test]
    //async fn if_manga_tracking_fails_it_sends_event_to_write_error_to_error_log_file() -> Result<(), Box<dyn Error>> {
    //    let expected_error_message = "some_error_message";
    //    let failing_tracker = TrackerTest::failing_with_error_message(&expected_error_message);
    //
    //    let mut manga_page: MangaPage<TrackerTest> = MangaPage::new(Manga::default(), Some(Picker::new((1, 2))));
    //
    //    flush_events(&mut manga_page);
    //
    //    manga_page.track_manga(Some(failing_tracker), "manga-test".to_string(), 1, Some(3));
    //
    //    let expected = MangaPageEvents::TrackingFailed(expected_error_message.to_string());
    //
    //    let result = timeout(Duration::from_millis(500), manga_page.local_event_rx.recv()).await?.unwrap();
    //
    //    assert_eq!(expected, result);
    //
    //    Ok(())
    //}
}
