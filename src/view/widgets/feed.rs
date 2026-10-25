use std::fmt::Display;

use chrono::NaiveDate;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, StatefulWidget, Widget, WidgetRef, Wrap};
use tui_widget_list::PreRender;

use crate::backend::database::MangaHistoryResponse;
use crate::backend::manga_provider::{Languages, LatestChapter, MangaProviders};
use crate::global::CURRENT_LIST_ITEM_STYLE;
use crate::utils::display_dates_since_publication;
use crate::view::widgets::ModalBuilder;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum FeedTabs {
    #[default]
    Timeline,
    ContinueReading,
    Library,
}

impl Display for FeedTabs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeline => write!(f, "Novedades"),
            Self::ContinueReading => write!(f, "Continuar leyendo"),
            Self::Library => write!(f, "Biblioteca"),
        }
    }
}

impl FeedTabs {
    #[inline]
    pub fn cycle(self) -> Self {
        match self {
            Self::Timeline => Self::ContinueReading,
            Self::ContinueReading => Self::Library,
            Self::Library => Self::Timeline,
        }
    }
}

pub fn provider_badge_style(provider: MangaProviders) -> Style {
    match provider {
        MangaProviders::Mangadex => Style::default().fg(Color::Rgb(255, 103, 64)),
        MangaProviders::Leercapitulo => Style::default().fg(Color::Rgb(0, 200, 255)),
        MangaProviders::Mangaoni => Style::default().fg(Color::Rgb(230, 80, 180)),
        MangaProviders::Inmanga => Style::default().fg(Color::Rgb(65, 105, 225)),
        MangaProviders::Weebcentral => Style::default().fg(Color::Rgb(46, 204, 113)),
        MangaProviders::Mangapill => Style::default().fg(Color::Rgb(241, 196, 15)),
        MangaProviders::Comizy => Style::default().fg(Color::Rgb(231, 76, 60)),
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TimelineChapterItem {
    pub manga_id: String,
    pub manga_title: String,
    pub provider: MangaProviders,
    pub chapter_id: String,
    pub chapter_number: String,
    pub chapter_title: String,
    pub publication_date: Option<NaiveDate>,
    pub language: Languages,
    pub is_read: bool,
    pub style: Style,
}

impl PreRender for TimelineChapterItem {
    fn pre_render(&mut self, context: &tui_widget_list::PreRenderContext) -> u16 {
        if context.is_selected {
            self.style = *CURRENT_LIST_ITEM_STYLE;
        }
        4
    }
}

impl Widget for TimelineChapterItem {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered().border_style(self.style);
        let inner = block.inner(area);
        block.render(area, buf);

        let provider_span = Span::styled(format!(" [{}] ", self.provider), provider_badge_style(self.provider).bold());
        let title_span = Span::styled(format!("{} ", self.manga_title), Style::default().bold());
        let ch_num_span = Span::styled(format!("Cap. {} ", self.chapter_number), Style::default().fg(Color::Yellow).bold());
        let lang_span = Span::styled(
            format!("| {} {}", self.language.as_emoji(), self.language.as_human_readable()),
            Style::default().fg(Color::DarkGray),
        );

        let line1 = Line::from(vec![provider_span, title_span, ch_num_span, lang_span]);

        let status_span = if self.is_read {
            Span::styled(" [LEÍDO] ", Style::default().fg(Color::DarkGray))
        } else {
            Span::styled(" [NUEVO] ", Style::default().fg(Color::Green).bold())
        };

        let ch_desc = if !self.chapter_title.is_empty() {
            Span::styled(format!("\"{}\" ", self.chapter_title), Style::default().italic())
        } else {
            Span::raw("")
        };

        let date_str = match self.publication_date {
            Some(d) => format!("• {}", display_dates_since_publication(d)),
            None => "".to_string(),
        };
        let date_span = Span::styled(date_str, Style::default().fg(Color::Gray));

        let line2 = Line::from(vec![status_span, ch_desc, date_span]);

        let layout = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(inner);
        Paragraph::new(line1).render(layout[0], buf);
        Paragraph::new(line2).render(layout[1], buf);
    }
}

#[derive(Clone, Debug, Default)]
pub struct TimelineWidget {
    pub items: Vec<TimelineChapterItem>,
    pub state: tui_widget_list::ListState,
    pub is_loading: bool,
}

impl TimelineWidget {
    #[inline]
    pub fn select_next(&mut self) {
        self.state.next();
    }

    #[inline]
    pub fn select_previous(&mut self) {
        self.state.previous();
    }

    #[inline]
    pub fn get_current_selected(&self) -> Option<&TimelineChapterItem> {
        self.state.selected.and_then(|index| self.items.get(index))
    }
}

impl StatefulWidget for TimelineWidget {
    type State = tui_widget_list::ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let layout = Layout::vertical([Constraint::Length(2), Constraint::Min(5)]);
        let [header_area, list_area] = layout.areas(area);

        let count_unread = self.items.iter().filter(|i| !i.is_read).count();
        let header_line = Line::from(vec![
            " Novedades recientes: ".into(),
            format!("{} capítulos ", self.items.len()).bold(),
            format!("({count_unread} sin leer) ").fg(Color::Green).bold(),
            "| Seleccionar: ".into(),
            "<j/k>".bold().fg(Color::Yellow),
            " | Abrir manga: ".into(),
            "<Enter / r>".bold().fg(Color::Yellow),
            " | Actualizar: ".into(),
            "<u>".bold().fg(Color::Yellow),
        ]);
        Paragraph::new(header_line).render(header_area, buf);

        if self.items.is_empty() {
            if self.is_loading {
                Paragraph::new(" Buscando novedades en tus mangas favoritos... Por favor, espera.")
                    .fg(Color::Yellow)
                    .render(list_area, buf);
            } else {
                Paragraph::new(" No hay novedades recientes.\n Consejo: Pulsa <f> en cualquier ficha de manga para añadirlo a Favoritos y ver sus nuevos capítulos aquí.")
                    .fg(Color::Gray)
                    .render(list_area, buf);
            }
        } else {
            let list = tui_widget_list::List::new(self.items);
            StatefulWidget::render(list, list_area, buf, state);
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecentChapters {
    pub id: String,
    pub title: String,
    pub number: String,
    pub translated_language: Languages,
    pub readeable_at: Option<NaiveDate>,
}

impl From<RecentChapters> for ListItem<'_> {
    fn from(value: RecentChapters) -> Self {
        let mut lines = vec![
            format!("Ch. {} ", value.number).into(),
            value.title.bold(),
            " | ".into(),
            value.translated_language.as_emoji().into(),
            " ".into(),
            value.translated_language.as_human_readable().into(),
        ];

        if let Some(date) = value.readeable_at {
            lines.push(" ".into());
            lines.push(display_dates_since_publication(date).into());
        }

        let line = Line::from(lines);

        ListItem::new(line)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MangasRead {
    pub id: String,
    pub title: String,
    pub provider: MangaProviders,
    pub style: Style,
    pub recent_chapters: Vec<RecentChapters>,
    pub last_read_date: Option<String>,
    pub last_read_chapter: Option<String>,
    pub total_read_count: u32,
    pub bookmark_chapter: Option<String>,
    pub bookmark_page: Option<u32>,
    pub is_favorite: bool,
    pub is_plan_to_read: bool,
    pub downloaded_count: u32,
}

impl From<LatestChapter> for RecentChapters {
    fn from(value: LatestChapter) -> Self {
        let id = value.id;

        let translated_language = value.language;

        Self {
            id,
            title: value.title,
            number: value.chapter_number,
            readeable_at: value.publication_date,
            translated_language,
        }
    }
}

impl Widget for MangasRead {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::horizontal([Constraint::Percentage(52), Constraint::Percentage(48)]);
        let [info_area, recent_chapters_area] = layout.margin(1).areas(area);

        Block::bordered().border_style(self.style).render(area, buf);

        let mut info_lines = Vec::new();

        // 1. Header: [Provider] Title
        let provider_badge = Span::styled(format!("[{}] ", self.provider), provider_badge_style(self.provider).bold());
        let title_span = Span::styled(self.title.clone(), Style::default().bold());
        info_lines.push(Line::from(vec![provider_badge, title_span]));

        // 2. Badges: [★ FAVORITO] [📋 PLAN TO READ] [💾 X caps]
        let mut badges = Vec::new();
        if self.is_favorite {
            badges.push(Span::styled("[★ FAVORITO] ", Style::default().fg(Color::Yellow).bold()));
        }
        if self.is_plan_to_read {
            badges.push(Span::styled("[📋 PENDIENTE] ", Style::default().fg(Color::Cyan)));
        }
        if self.downloaded_count > 0 {
            badges.push(Span::styled(format!("[💾 {} caps] ", self.downloaded_count), Style::default().fg(Color::Magenta)));
        }
        if !badges.is_empty() {
            info_lines.push(Line::from(badges));
        }

        // 3. Last read chapter
        if let Some(ch) = &self.last_read_chapter {
            info_lines
                .push(Line::from(vec![Span::styled("📖 Último leído: ", Style::default().fg(Color::Cyan)), Span::raw(ch.clone())]));
        } else if self.is_plan_to_read {
            info_lines.push(Line::from(vec![
                Span::styled("📌 Estado: ", Style::default().fg(Color::DarkGray)),
                Span::styled("En lista para leer más tarde", Style::default().fg(Color::Gray)),
            ]));
        }

        // 4. Progress / Bookmark
        let mut progress_spans = Vec::new();
        if self.total_read_count > 0 {
            progress_spans
                .push(Span::styled(format!("📊 {} caps leídos ", self.total_read_count), Style::default().fg(Color::Gray)));
        }
        if let Some(page) = self.bookmark_page {
            progress_spans.push(Span::styled(format!("🔖 Marcador en pág. {page}"), Style::default().fg(Color::Yellow)));
        }
        if !progress_spans.is_empty() {
            info_lines.push(Line::from(progress_spans));
        }

        // 5. Action hint
        info_lines.push(Line::from(vec![
            Span::styled("<Enter / r> ", Style::default().fg(Color::Yellow).bold()),
            Span::styled("Abrir manga", Style::default().fg(Color::DarkGray)),
        ]));

        Paragraph::new(info_lines)
            .block(Block::default().borders(Borders::RIGHT))
            .wrap(Wrap { trim: true })
            .render(info_area, buf);

        if !self.recent_chapters.is_empty() {
            Widget::render(
                List::new(self.recent_chapters).block(Block::bordered().title("Capítulos recientes")),
                recent_chapters_area.inner(Margin {
                    horizontal: 1,
                    vertical: 0,
                }),
                buf,
            );
        } else {
            Paragraph::new(" Sin capítulos recientes cargados")
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::bordered().title("Capítulos recientes"))
                .render(recent_chapters_area, buf);
        }
    }
}

impl PreRender for MangasRead {
    fn pre_render(&mut self, context: &tui_widget_list::PreRenderContext) -> u16 {
        if context.is_selected {
            self.style = *CURRENT_LIST_ITEM_STYLE;
        }
        8
    }
}

#[derive(Clone, Debug, Default)]
pub struct HistoryWidget {
    pub page: u32,
    pub total_results: u32,
    pub mangas: Vec<MangasRead>,
    pub state: tui_widget_list::ListState,
}

impl HistoryWidget {
    #[inline]
    pub fn select_next(&mut self) {
        self.state.next();
    }

    #[inline]
    pub fn select_previous(&mut self) {
        self.state.previous();
    }

    #[inline]
    pub fn get_current_manga_selected(&self) -> Option<&MangasRead> {
        self.state.selected.and_then(|index| self.mangas.get(index))
    }

    #[inline]
    pub fn next_page(&mut self) {
        self.page += 1
    }

    #[inline]
    pub fn previous_page(&mut self) {
        self.page -= 1;
    }

    pub fn set_chapter(&mut self, manga_id: String, response: Vec<LatestChapter>) {
        if let Some(manga) = self.mangas.iter_mut().find(|manga| manga.id == manga_id) {
            for chapter in response {
                manga.recent_chapters.push(RecentChapters::from(chapter));
            }
        }
    }

    pub fn can_search_next_page(&self, total_items: f64) -> bool {
        self.page as f64 != (self.total_results as f64 / total_items).ceil() && !self.mangas.is_empty()
    }

    pub fn can_search_previous_page(&self) -> bool {
        !self.mangas.is_empty() && self.page != 1
    }

    pub fn from_database_response(response: MangaHistoryResponse) -> Self {
        Self {
            page: response.page,
            total_results: response.total_items,
            mangas: response
                .mangas
                .into_iter()
                .map(|history| MangasRead {
                    id: history.id,
                    title: history.title,
                    provider: history.provider,
                    recent_chapters: vec![],
                    style: Style::default(),
                    last_read_date: history.last_read,
                    last_read_chapter: history.last_read_chapter,
                    total_read_count: history.total_read_count,
                    bookmark_chapter: history.bookmark_chapter,
                    bookmark_page: history.bookmark_page,
                    is_favorite: history.is_favorite,
                    is_plan_to_read: history.is_plan_to_read,
                    downloaded_count: history.downloaded_count,
                })
                .collect(),
            state: tui_widget_list::ListState::default(),
        }
    }

    fn render_pagination_data_and_instructions(&mut self, area: Rect, buf: &mut Buffer) {
        let amount_pages = (self.total_results as f64 / 5_f64).ceil().max(1.0);
        Paragraph::new(Line::from(vec![
            "Mangas: ".into(),
            self.total_results.to_string().bold(),
            format!(" | Pág: {} de {} | ", self.page, amount_pages).into(),
            "Página: ".into(),
            "<w/b> ".bold().fg(Color::Yellow),
            "| Ficha: ".into(),
            "<Enter / r> ".bold().fg(Color::Yellow),
            "| Quitar: ".into(),
            "<d> ".bold().fg(Color::Red),
            "| Borrar todo: ".into(),
            "<D> ".bold().fg(Color::Red),
        ]))
        .render(area, buf);
    }
}

impl StatefulWidget for HistoryWidget {
    type State = tui_widget_list::ListState;

    fn render(mut self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let layout = Layout::vertical([Constraint::Length(2), Constraint::Min(5)]);
        let [total_results_area, list_area] = layout.areas(area);

        self.render_pagination_data_and_instructions(total_results_area, buf);
        let list = tui_widget_list::List::new(self.mangas);
        StatefulWidget::render(list, list_area, buf, state);
    }
}

#[derive(Debug)]
struct AskConfirmationDeleteAllModalBody {
    manga_provider: MangaProviders,
    tab: FeedTabs,
}

impl WidgetRef for AskConfirmationDeleteAllModalBody {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Block::bordered().render(area, buf);

        let inner = area.inner(Margin {
            horizontal: 2,
            vertical: 2,
        });

        let [warning_area, options_area] = Layout::vertical([Constraint::Percentage(20), Constraint::Percentage(80)]).areas(inner);

        let warning = format!(
            "¿Estás seguro de que quieres eliminar TODOS los mangas del proveedor {} en la sección {}?",
            self.manga_provider, self.tab
        );

        Paragraph::new(warning).wrap(Wrap { trim: true }).render(warning_area, buf);

        let as_list = List::new(Line::from(vec![
            "Tu historial de lectura y descargas se mantendrán pero no se mostrarán en esta página.".into(),
            "Sí, eliminar: <w>".bold().fg(Color::Red),
            "No, cancelar: <q>".bold().fg(Color::Green),
        ]));

        Widget::render(as_list, options_area, buf);
    }
}

#[derive(Debug)]
pub struct AskConfirmationDeleteAllModal {
    manga_provider: MangaProviders,
    tab: FeedTabs,
}

impl AskConfirmationDeleteAllModal {
    pub fn new(tab: FeedTabs, provider: MangaProviders) -> Self {
        Self {
            manga_provider: provider,
            tab,
        }
    }
}

impl Widget for AskConfirmationDeleteAllModal {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let modal = ModalBuilder::new(AskConfirmationDeleteAllModalBody {
            manga_provider: self.manga_provider,
            tab: self.tab,
        })
        .with_dimensions(super::ModalDimensions {
            width: 70,
            height: 60,
        })
        .build();

        modal.render(area, buf);
    }
}
