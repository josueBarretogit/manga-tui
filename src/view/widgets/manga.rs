use std::path::PathBuf;

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::line::THICK;
use ratatui::text::{Line, ToSpan};
use ratatui::widgets::{Block, LineGauge, Paragraph, StatefulWidget, Widget, Wrap};
use throbber_widgets_tui::{Throbber, ThrobberState};
use tokio::sync::mpsc::UnboundedSender;
use tui_widget_list::PreRender;

use crate::backend::manga_provider::Chapter;
use crate::global::{CURRENT_LIST_ITEM_STYLE, ERROR_STYLE, INSTRUCTIONS_STYLE};
use crate::utils::display_dates_since_publication;
use crate::view::pages::manga::MangaPageEvents;

#[derive(Clone, Debug, Default)]
pub enum ChapterItemState {
    #[default]
    Normal,
    /// When the user tried to download a chapter and there was an error
    DownloadError,
    /// When the user tried to read a chapter and there was an error
    ReadError,
}

#[derive(Clone, Debug, Default)]
pub struct ChapterItem {
    pub chapter: Chapter,
    pub is_read: bool,
    pub is_downloaded: bool,
    pub is_bookmarked: bool,
    pub state: ChapterItemState,
    pub download_loading_state: Option<f64>,
    pub style: Style,
}

impl Widget for ChapterItem {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::horizontal([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Fill(50),
            Constraint::Fill(30),
            Constraint::Fill(20),
        ]);

        let [is_read_area, is_downloaded_area, title_area, scanlator_area, readable_at_area] = layout.areas(area);

        let is_read_icon = if self.is_read { "👀" } else { " " };

        let is_downloaded_icon = if self.is_downloaded { "📥" } else { " " };

        Line::from(is_read_icon).style(self.style).render(is_read_area, buf);
        Line::from(is_downloaded_icon).style(self.style).render(is_downloaded_area, buf);

        let information = if self.is_bookmarked {
            "Bookmarked | ".to_string()
        } else {
            format!("Vol. {} Ch. {} | ", self.chapter.volume_number.unwrap_or_default(), self.chapter.chapter_number)
        };

        Paragraph::new(Line::from(vec![information.into(), self.chapter.title.into()]))
            .wrap(Wrap { trim: true })
            .style(self.style)
            .render(title_area, buf);

        match self.download_loading_state.as_ref() {
            Some(progress) => {
                LineGauge::default()
                    .block(Block::bordered().title("Downloading please wait a moment"))
                    .filled_style(Style::default().fg(Color::Blue).bg(Color::Black).add_modifier(Modifier::BOLD))
                    .line_set(THICK)
                    .ratio(*progress)
                    .render(scanlator_area, buf);
            },
            None => match self.state {
                ChapterItemState::Normal => {
                    Paragraph::new(self.chapter.scanlator.unwrap_or_default())
                        .style(self.style)
                        .wrap(Wrap { trim: true })
                        .render(scanlator_area, buf);

                    Paragraph::new(display_dates_since_publication(self.chapter.publication_date))
                        .style(self.style)
                        .wrap(Wrap { trim: true })
                        .render(readable_at_area, buf);
                },
                ChapterItemState::DownloadError => {
                    Paragraph::new(
                        "Cannot download this chapter due to an error, please try again"
                            .to_span()
                            .style(*ERROR_STYLE),
                    )
                    .render(
                        Rect::new(
                            scanlator_area.x,
                            scanlator_area.y,
                            scanlator_area.width + readable_at_area.width,
                            scanlator_area.height,
                        ),
                        buf,
                    );
                },
                ChapterItemState::ReadError => {
                    Paragraph::new("Cannot read this chapter due to an error, please try again".to_span().style(*ERROR_STYLE))
                        .render(
                            Rect::new(
                                scanlator_area.x,
                                scanlator_area.y,
                                scanlator_area.width + readable_at_area.width,
                                scanlator_area.height,
                            ),
                            buf,
                        );
                },
            },
        }
    }
}

impl PreRender for ChapterItem {
    fn pre_render(&mut self, context: &tui_widget_list::PreRenderContext) -> u16 {
        if context.is_selected {
            self.style = *CURRENT_LIST_ITEM_STYLE;
        }

        if self.download_loading_state.is_some() { 3 } else { 1 }
    }
}

impl ChapterItem {
    pub fn new(chapter: Chapter) -> Self {
        Self {
            chapter,
            is_read: false,
            is_downloaded: false,
            is_bookmarked: false,
            download_loading_state: None,
            style: Style::default(),
            state: ChapterItemState::Normal,
        }
    }

    pub fn set_download_error(&mut self) {
        self.download_loading_state = None;
        self.state = ChapterItemState::DownloadError;
    }

    pub fn set_read_error(&mut self) {
        self.state = ChapterItemState::ReadError;
    }

    pub fn set_normal_state(&mut self) {
        self.state = ChapterItemState::Normal;
    }
}

#[derive(Clone, Debug, Default)]
pub struct ChaptersListWidget {
    pub chapters: Vec<ChapterItem>,
}

impl ChaptersListWidget {
    pub fn from_response(response: Vec<Chapter>) -> Self {
        let mut chapters: Vec<ChapterItem> = vec![];

        for chap in response {
            chapters.push(ChapterItem::new(chap));
        }

        Self { chapters }
    }
}

impl StatefulWidget for ChaptersListWidget {
    type State = tui_widget_list::ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let chapters_list = tui_widget_list::List::new(self.chapters);
        StatefulWidget::render(chapters_list, area, buf, state);
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DownloadPhase {
    #[default]
    ProccessNotStarted,
    Asking,
    FetchingChaptersData,
    DownloadingChapters,
    AskAbortProcess,
    ErrorChaptersData,
}

#[derive(Debug)]
pub struct DownloadAllChaptersState {
    pub phase: DownloadPhase,
    pub total_chapters: f64,
    pub loader_state: ThrobberState,
    pub download_progress: f64,
    pub download_location: PathBuf,
    pub tx: UnboundedSender<MangaPageEvents>,
}

impl DownloadAllChaptersState {
    pub fn new(tx: UnboundedSender<MangaPageEvents>) -> Self {
        Self {
            phase: DownloadPhase::default(),
            total_chapters: 0.0,
            loader_state: ThrobberState::default(),
            download_progress: 0.0,
            download_location: PathBuf::default(),
            tx,
        }
    }

    pub fn is_downloading(&self) -> bool {
        self.phase == DownloadPhase::DownloadingChapters || self.phase == DownloadPhase::AskAbortProcess
    }

    pub fn process_started(&self) -> bool {
        self.phase != DownloadPhase::ProccessNotStarted
    }

    /// Either phase can start download
    pub fn is_ready_to_fetch_data(&self) -> bool {
        self.phase == DownloadPhase::Asking || self.phase == DownloadPhase::ErrorChaptersData
    }

    pub fn set_download_progress(&mut self) {
        self.download_progress += 1.0;
    }

    pub fn ask_for_confirmation(&mut self) {
        if !self.is_downloading() {
            self.phase = DownloadPhase::Asking;
        }
    }

    pub fn fetch_chapters_data(&mut self) {
        if !self.is_downloading() {
            self.phase = DownloadPhase::FetchingChaptersData;
        }
    }

    pub fn start_download(&mut self) {
        if !self.is_downloading() {
            self.phase = DownloadPhase::DownloadingChapters;
            self.total_chapters = 0.0;
            self.download_progress = 0.0;
        }
    }

    pub fn cancel(&mut self) {
        if !self.is_downloading() {
            self.phase = DownloadPhase::ProccessNotStarted;
        }
    }

    pub fn reset(&mut self) {
        if self.is_downloading() || self.phase == DownloadPhase::AskAbortProcess {
            self.phase = DownloadPhase::ProccessNotStarted;
            self.total_chapters = 0.0;
            self.download_progress = 0.0;
        }
    }

    pub fn ask_abort_proccess(&mut self) {
        if self.is_downloading() {
            self.phase = DownloadPhase::AskAbortProcess;
        }
    }

    pub fn abort_proccess(&mut self) {
        if self.is_downloading() || self.phase == DownloadPhase::AskAbortProcess {
            self.reset();
        }
    }

    pub fn continue_download(&mut self) {
        if self.phase == DownloadPhase::AskAbortProcess {
            self.phase = DownloadPhase::DownloadingChapters;
        }
    }

    pub fn set_total_chapters(&mut self, total_chapters: f64) {
        self.total_chapters = total_chapters;
    }

    pub fn finished_downloading(&self) -> bool {
        self.download_progress == self.total_chapters
    }

    pub fn set_download_error(&mut self) {
        self.phase = DownloadPhase::ErrorChaptersData;
    }

    pub fn set_download_location(&mut self, location: PathBuf) {
        self.download_location = location
    }

    pub fn tick(&mut self) {
        self.loader_state.calc_next();
    }
}

pub struct DownloadAllChaptersWidget<'a> {
    pub manga_title: &'a str,
}

impl<'a> DownloadAllChaptersWidget<'a> {
    pub fn new(manga_title: &'a str) -> Self {
        Self { manga_title }
    }
}

impl<'a> DownloadAllChaptersWidget<'a> {
    fn render_download_information(&mut self, area: Rect, buf: &mut Buffer, state: &mut DownloadAllChaptersState) {
        let [information_area, loader_area] = Layout::horizontal([Constraint::Fill(2), Constraint::Fill(1)]).areas(area);

        let download_location = format!("Download location : {}", state.download_location.as_path().display(),);

        Paragraph::new(Line::from(vec![
            "Downloading all chapters, this will take a while, ".into(),
            download_location.into(),
            " ".into(),
        ]))
        .wrap(Wrap { trim: true })
        .render(information_area, buf);

        let loader = Throbber::default()
            .label("Download in progress")
            .style(Style::default().fg(Color::Yellow))
            .throbber_set(throbber_widgets_tui::BRAILLE_SIX)
            .use_type(throbber_widgets_tui::WhichUse::Spin);
        StatefulWidget::render(loader, loader_area, buf, &mut state.loader_state);
    }
}

impl<'a> StatefulWidget for DownloadAllChaptersWidget<'a> {
    type State = DownloadAllChaptersState;

    fn render(mut self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        Block::bordered().render(area, buf);

        let download_information_area = area.inner(Margin {
            horizontal: 2,
            vertical: 2,
        });

        match state.phase {
            DownloadPhase::ProccessNotStarted => {},
            DownloadPhase::Asking => {
                let instructions = vec![
                    "Do you want to download all chapters? Yes: ".into(),
                    "<Enter>".to_span().style(*INSTRUCTIONS_STYLE),
                    " no ".into(),
                    "<Esc>".to_span().style(*INSTRUCTIONS_STYLE),
                ];

                Paragraph::new(Line::from(instructions)).render(download_information_area, buf);
            },
            DownloadPhase::AskAbortProcess => {
                let instructions = vec![
                    "Are you sure you want to cancel? yes: ".into(),
                    "<Enter>".to_span().style(*INSTRUCTIONS_STYLE),
                    " no: ".into(),
                    "<Esc>".to_span().style(*INSTRUCTIONS_STYLE),
                ];

                Paragraph::new(Line::from(instructions)).render(download_information_area, buf);
            },
            DownloadPhase::FetchingChaptersData => {
                let loader = Throbber::default()
                    .label("fetching manga data after this each chapter will begin to be downloaded")
                    .style(Style::default().fg(Color::Yellow))
                    .throbber_set(throbber_widgets_tui::BRAILLE_SIX)
                    .use_type(throbber_widgets_tui::WhichUse::Spin);

                StatefulWidget::render(loader, download_information_area, buf, &mut state.loader_state);
            },
            DownloadPhase::ErrorChaptersData => {
                "Could not get chapters data, press <Enter> to try again"
                    .to_span()
                    .style(*ERROR_STYLE)
                    .render(download_information_area, buf);
            },
            DownloadPhase::DownloadingChapters => {
                if state.finished_downloading() {
                    state.tx.send(MangaPageEvents::FinishedDownloadingAllChapters).ok();
                    return;
                }

                let [information_area, progress_area] =
                    Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)]).areas(download_information_area);

                self.render_download_information(information_area, buf, state);

                let download_progress_title = vec![
                    format!("Total chapters: {}, chapters downloaded : {} ", state.total_chapters, state.download_progress).into(),
                    "Cancel download: ".into(),
                    "<Esc>".to_span().style(*INSTRUCTIONS_STYLE),
                ];

                LineGauge::default()
                    .block(Block::bordered().title(download_progress_title))
                    .filled_style(Style::default().fg(Color::Blue).bg(Color::Black).add_modifier(Modifier::BOLD))
                    .line_set(THICK)
                    .ratio(state.download_progress / state.total_chapters)
                    .render(progress_area, buf);
            },
        }
    }
}

#[cfg(test)]
mod test {
    use tokio::sync::mpsc;

    use super::*;

    #[tokio::test]
    async fn download_state_works() {
        let (tx, mut rx) = mpsc::unbounded_channel::<MangaPageEvents>();
        let mut download_all_chapters_state = DownloadAllChaptersState::new(tx);

        assert_eq!(DownloadPhase::ProccessNotStarted, download_all_chapters_state.phase);
        assert!(!download_all_chapters_state.process_started());
        assert!(!download_all_chapters_state.is_downloading());

        assert_eq!(0.0, download_all_chapters_state.download_progress);

        download_all_chapters_state.ask_for_confirmation();

        assert_eq!(DownloadPhase::Asking, download_all_chapters_state.phase);

        // The user cancelled
        download_all_chapters_state.cancel();

        assert_eq!(DownloadPhase::ProccessNotStarted, download_all_chapters_state.phase);

        download_all_chapters_state.ask_for_confirmation();

        // The user confirmed
        download_all_chapters_state.fetch_chapters_data();

        assert_eq!(DownloadPhase::FetchingChaptersData, download_all_chapters_state.phase);

        download_all_chapters_state.start_download();

        assert_eq!(DownloadPhase::DownloadingChapters, download_all_chapters_state.phase);

        download_all_chapters_state.set_download_error();

        assert_eq!(DownloadPhase::ErrorChaptersData, download_all_chapters_state.phase);

        download_all_chapters_state.fetch_chapters_data();

        assert_eq!(DownloadPhase::FetchingChaptersData, download_all_chapters_state.phase);

        download_all_chapters_state.start_download();

        download_all_chapters_state.set_total_chapters(3.0);

        assert_eq!(DownloadPhase::DownloadingChapters, download_all_chapters_state.phase);
        assert_eq!(3.0, download_all_chapters_state.total_chapters);

        download_all_chapters_state.set_download_progress();
        download_all_chapters_state.set_download_progress();
        download_all_chapters_state.set_download_progress();

        let area = Rect::new(0, 0, 50, 50);
        let mut buf = Buffer::empty(area);

        StatefulWidget::render(DownloadAllChaptersWidget::new("some_title"), area, &mut buf, &mut download_all_chapters_state);

        let download_finished = rx.recv().await.unwrap();

        assert_eq!(MangaPageEvents::FinishedDownloadingAllChapters, download_finished);
    }
}
