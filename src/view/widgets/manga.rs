use crate::backend::filter::Languages;
use crate::backend::{AppDirectories, ChapterResponse};
use crate::common::PageType;
use crate::global::{CURRENT_LIST_ITEM_STYLE, ERROR_STYLE, INSTRUCTIONS_STYLE};
use crate::utils::display_dates_since_publication;
use ratatui::{prelude::*, widgets::*};
use tui_widget_list::PreRender;

use self::text::ToSpan;

#[derive(Clone)]
pub enum ChapterItemState {
    Normal,
    /// When the user tried to download a chapter and there was an error
    DownloadError,
    /// When the user tried to read a chapter and there was an error
    ReadError,
}

#[derive(Clone)]
pub struct ChapterItem {
    pub id: String,
    pub title: String,
    pub readable_at: String,
    pub scanlator: String,
    pub chapter_number: String,
    pub is_read: bool,
    pub is_downloaded: bool,
    pub state: ChapterItemState,
    pub download_loading_state: Option<f64>,
    pub translated_language: Languages,
    style: Style,
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

        let [is_read_area, is_downloaded_area, title_area, scanlator_area, readable_at_area] =
            layout.areas(area);

        let is_read_icon = if self.is_read { "👀" } else { " " };

        let is_downloaded_icon = if self.is_downloaded { "📥" } else { " " };

        Line::from(is_read_icon)
            .style(self.style)
            .render(is_read_area, buf);
        Line::from(is_downloaded_icon)
            .style(self.style)
            .render(is_downloaded_area, buf);

        Paragraph::new(Line::from(vec![
            format!(" Ch. {} ", self.chapter_number).into(),
            self.title.into(),
        ]))
        .wrap(Wrap { trim: true })
        .style(self.style)
        .render(title_area, buf);

        match self.download_loading_state.as_ref() {
            Some(progress) => {
                LineGauge::default()
                    .block(Block::bordered().title("Downloading please wait a moment"))
                    .filled_style(
                        Style::default()
                            .fg(Color::Blue)
                            .bg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    )
                    .line_set(symbols::line::THICK)
                    .ratio(*progress)
                    .render(scanlator_area, buf);
            }
            None => match self.state {
                ChapterItemState::Normal => {
                    Paragraph::new(self.scanlator)
                        .style(self.style)
                        .wrap(Wrap { trim: true })
                        .render(scanlator_area, buf);

                    Paragraph::new(self.readable_at)
                        .style(self.style)
                        .wrap(Wrap { trim: true })
                        .render(readable_at_area, buf);
                }
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
                }
                ChapterItemState::ReadError => {
                    Paragraph::new(
                        "Cannot read this chapter due to an error, please try again"
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
                }
            },
        }
    }
}

impl PreRender for ChapterItem {
    fn pre_render(&mut self, context: &tui_widget_list::PreRenderContext) -> u16 {
        if context.is_selected {
            self.style = *CURRENT_LIST_ITEM_STYLE;
        }

        if self.download_loading_state.is_some() {
            3
        } else {
            1
        }
    }
}

impl ChapterItem {
    pub fn new(
        id: String,
        title: String,
        chapter_number: String,
        readable_at: String,
        scanlator: String,
        translated_language: Languages,
    ) -> Self {
        Self {
            id,
            title,
            readable_at,
            scanlator,
            chapter_number,
            is_read: false,
            is_downloaded: false,
            download_loading_state: None,
            translated_language,
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

#[derive(Clone)]
pub struct ChaptersListWidget {
    pub chapters: Vec<ChapterItem>,
}

impl ChaptersListWidget {
    pub fn from_response(response: &ChapterResponse) -> Self {
        let mut chapters: Vec<ChapterItem> = vec![];

        let today = chrono::offset::Local::now().date_naive();
        for chapter in response.data.iter() {
            let id = chapter.id.clone();
            let title = chapter
                .attributes
                .title
                .clone()
                .unwrap_or("No title".to_string());

            let chapter_number = chapter
                .attributes
                .chapter
                .clone()
                .unwrap_or("0".to_string());

            let translated_language: Languages =
                Languages::try_from_iso_code(&chapter.attributes.translated_language)
                    .unwrap_or(*Languages::get_preferred_lang());

            let parse_date = chrono::DateTime::parse_from_rfc3339(&chapter.attributes.readable_at)
                .unwrap_or_default();

            let difference = today - parse_date.date_naive();

            let scanlator = chapter
                .relationships
                .iter()
                .find(|rel| rel.type_field == "scanlation_group")
                .map(|rel| rel.attributes.as_ref().unwrap().name.to_string());

            chapters.push(ChapterItem::new(
                id,
                title,
                chapter_number,
                display_dates_since_publication(difference.num_days()),
                scanlator.unwrap_or_default(),
                translated_language,
            ))
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
    SettingQuality,
    FetchingChaptersData,
    DownloadingChapters,
}

#[derive(Default)]
pub struct DownloadAllChaptersState {
    pub phase: DownloadPhase,
    pub image_quality: PageType,
    pub total_chapters: f64,
    pub download_progress: f64,
}

impl DownloadAllChaptersState {
    pub fn is_downloading_chapters(&self) -> bool {
        self.phase == DownloadPhase::DownloadingChapters
    }

    pub fn process_started(&self) -> bool {
        self.phase != DownloadPhase::ProccessNotStarted
    }

    pub fn is_ready_to_download(&self) -> bool {
        self.phase == DownloadPhase::SettingQuality
    }

    pub fn set_download_progress(&mut self) {
        self.download_progress += 1.0;
    }

    pub fn ask_for_confirmation(&mut self) {
        self.phase = DownloadPhase::Asking;
    }

    pub fn confirm(&mut self) {
        self.phase = DownloadPhase::SettingQuality;
    }

    pub fn start_download(&mut self) {
        self.phase = DownloadPhase::DownloadingChapters;
    }

    pub fn cancel(&mut self) {
        self.phase = DownloadPhase::ProccessNotStarted;
    }

    pub fn set_total_chapters(&mut self, total_chapters : f64) {
        self.total_chapters = total_chapters;
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

impl<'a> StatefulWidget for DownloadAllChaptersWidget<'a> {
    type State = DownloadAllChaptersState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        Block::bordered().render(area, buf);

        let download_information_area = area.inner(Margin {
            horizontal: 2,
            vertical: 2,
        });

        match state.phase {
            DownloadPhase::ProccessNotStarted => {}
            DownloadPhase::Asking => {
                let instructions = vec![
                    "Do you want to download all chapters? Yes: ".into(),
                    "<Enter>".to_span().style(*INSTRUCTIONS_STYLE),
                    " no ".into(),
                    "<Esc>".to_span().style(*INSTRUCTIONS_STYLE),
                ];

                Paragraph::new(Line::from(instructions)).render(download_information_area, buf);
            }
            DownloadPhase::SettingQuality => {
                Widget::render(
                    List::new([
                        Line::from(vec![
                            "Choose image quality ".into(),
                            "<t>".to_span().style(*INSTRUCTIONS_STYLE),
                        ]),
                        "Lower image quality is recommended for slow internet".into(),
                        state.image_quality.as_human_readable().into(),
                        Line::from(vec![
                            "Start download: ".into(),
                            "<Spacebar>".to_span().style(*INSTRUCTIONS_STYLE),
                        ]),
                    ]),
                    download_information_area,
                    buf,
                );
            }
            DownloadPhase::FetchingChaptersData => {
                // add throbber
                "fetching manga data after this each chapter will be downloaded"
                    .render(download_information_area, buf);
            }
            DownloadPhase::DownloadingChapters => {
                let [information_area, progress_area] =
                    Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)])
                        .areas(download_information_area);
                Paragraph::new("Downloading all chapters, this will take a while")
                    .render(information_area, buf);

                LineGauge::default()
                    .block(Block::bordered().title(format!(
                                    "Total chapters: {}, you can check the download at : {}",
                                    state.total_chapters,
                                    AppDirectories::MangaDownloads
                                        .into_path_buf()
                                        .join(self.manga_title)
                                        .to_str()
                                        .unwrap()
                                )))
                    .filled_style(
                        Style::default()
                            .fg(Color::Blue)
                            .bg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    )
                    .line_set(symbols::line::THICK)
                    .ratio(state.download_progress / state.total_chapters)
                    .render(progress_area, buf);
            }
        }
    }
}
