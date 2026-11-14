#[cfg(test)]
use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style, Styled};
use ratatui::text::{Line, Span};
use ratatui::widgets::block::Title;
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Widget, WidgetRef, Wrap};

use crate::backend::manga_provider::LatestChapter;
use crate::backend::tui::Events;
use crate::global::INSTRUCTIONS_STYLE;
use crate::i18n::tr;
use crate::utils::centered_rect;

pub mod feed;
pub mod home;
pub mod manga;
pub mod reader;
pub mod search;

pub trait Component {
    type Actions;
    ///Handles the logic for drawing to the screen
    fn render(&mut self, area: Rect, frame: &mut Frame<'_>);
    fn handle_events(&mut self, events: Events);
    fn update(&mut self, action: Self::Actions);

    /// This is intended for stuff like aborting tasks and clearing vec's
    fn clean_up(&mut self);
}

pub trait StatefulWidgetFrame {
    type State;
    fn render(&mut self, area: Rect, frame: &mut Frame<'_>, state: &mut Self::State);
}

#[derive(Debug, Default)]
pub struct ModalDimensions {
    width: u16,
    height: u16,
}

#[derive(Debug)]
pub struct Modal<T: WidgetRef> {
    dimensions: ModalDimensions,
    child_widget: T,
}

#[derive(Debug, Default)]
pub struct ModalBuilder<T: WidgetRef> {
    dimensions: Option<ModalDimensions>,
    child_widget: T,
}

impl<T: WidgetRef> ModalBuilder<T> {
    #[inline]
    pub fn new(child_widget: T) -> Self {
        Self {
            dimensions: None,
            child_widget,
        }
    }

    #[inline]
    pub fn with_dimensions(mut self, dimensions: ModalDimensions) -> Self {
        self.dimensions = Some(dimensions);
        self
    }

    #[inline]
    pub fn build(self) -> Modal<T> {
        Modal::new(
            self.dimensions.unwrap_or(ModalDimensions {
                width: 50,
                height: 50,
            }),
            self.child_widget,
        )
    }
}

impl<T: WidgetRef> Modal<T> {
    pub fn new(dimensions: ModalDimensions, child_widget: T) -> Self {
        Self {
            dimensions,
            child_widget,
        }
    }
}

impl<T: WidgetRef> WidgetRef for Modal<T> {
    fn render_ref(&self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let area = centered_rect(area, self.dimensions.width, self.dimensions.height);
        Clear.render(area, buf);

        self.child_widget.render_ref(area, buf);
    }
}

impl<T: WidgetRef> Widget for Modal<T> {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        WidgetRef::render_ref(&self, area, buf);
    }
}

#[derive(Debug)]
struct ErrorModalBody<'a> {
    message: &'a str,
}

impl<'a> Widget for ErrorModalBody<'a> {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        self.render_ref(area, buf);
    }
}

impl<'a> WidgetRef for ErrorModalBody<'a> {
    fn render_ref(&self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Red))
            .title(Title::from(Line::from(vec![
                " ⚠ Error — press ".into(),
                "<q>".set_style(*INSTRUCTIONS_STYLE),
                " to close ".into(),
            ])))
            .render(area, buf);

        let inner = area.inner(Margin {
            horizontal: 2,
            vertical: 2,
        });

        Paragraph::new(self.message).wrap(Wrap { trim: true }).render(inner, buf);
    }
}

#[derive(Debug)]
pub struct ErrorModal<'a> {
    message: &'a str,
}

impl<'a> ErrorModal<'a> {
    #[inline]
    pub fn new(message: &'a str) -> Self {
        Self { message }
    }
}

impl<'a> Widget for ErrorModal<'a> {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let modal = ModalBuilder::new(ErrorModalBody {
            message: self.message,
        })
        .build();

        modal.render_ref(area, buf);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewChapterItem {
    pub manga_id: String,
    pub manga_title: String,
    pub chapter_numbers: String,
    pub chapters: Vec<LatestChapter>,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewChaptersModalState {
    pub items: Vec<NewChapterItem>,
    pub cursor: usize,
    pub status_message: Option<String>,
}

impl NewChaptersModalState {
    pub fn new(items: Vec<NewChapterItem>) -> Self {
        Self {
            items,
            cursor: 0,
            status_message: None,
        }
    }
}

#[derive(Debug)]
pub struct NewChaptersModal<'a> {
    state: &'a NewChaptersModalState,
}

impl<'a> NewChaptersModal<'a> {
    #[inline]
    pub fn new(state: &'a NewChaptersModalState) -> Self {
        Self { state }
    }
}

impl<'a> Widget for NewChaptersModal<'a> {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let area = centered_rect(area, 75, 65);
        Clear.render(area, buf);

        if let Some(status) = &self.state.status_message {
            let block = Block::bordered()
                .title(Line::from(vec![Span::styled(" New chapters ", Style::default().add_modifier(Modifier::BOLD))]))
                .title_bottom(Line::from(vec![Span::styled(
                    " Press <q> or <Esc> to close ",
                    Style::default().add_modifier(Modifier::ITALIC),
                )]));
            let inner = block.inner(area);
            block.render(area, buf);
            Paragraph::new(status.as_str()).wrap(Wrap { trim: true }).render(inner, buf);
            return;
        }

        let selected_count = self.state.items.iter().filter(|i| i.selected).count();
        let total_count = self.state.items.len();

        let block = Block::bordered()
            .title(Line::from(vec![Span::styled(
                format!(" New chapters available ({}/{}) ", selected_count, total_count),
                Style::default().add_modifier(Modifier::BOLD),
            )]))
            .title_bottom(Line::from(vec![Span::styled(
                " <j/k> Navigate | <Space> Toggle | <a> All | <y>/<d> Download | <q> Close ",
                Style::default().fg(Color::Yellow),
            )]));

        let inner = area.inner(Margin {
            horizontal: 2,
            vertical: 1,
        });
        block.render(area, buf);

        if inner.height < 3 || inner.width < 10 {
            return;
        }

        let header = Line::from(vec![Span::styled(
            crate::i18n::tr("Selecciona mangas para descargar nuevos capítulos:", "Select manga to download new chapters:"),
            Style::default().add_modifier(Modifier::BOLD),
        )]);

        let max_items_visible = (inner.height.saturating_sub(2)) as usize;
        let cursor = self.state.cursor;
        let scroll = if max_items_visible == 0 || total_count <= max_items_visible || cursor < max_items_visible {
            0
        } else {
            cursor + 1 - max_items_visible
        };

        let mut lines = vec![header, Line::raw("")];

        for (idx, item) in self.state.items.iter().enumerate().skip(scroll).take(max_items_visible) {
            let is_focused = idx == cursor;
            let cursor_span = if is_focused {
                Span::styled("> ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            } else {
                Span::raw("  ")
            };

            let check_span = if item.selected {
                Span::styled("[✓] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
            } else {
                Span::styled("[ ] ", Style::default().fg(Color::DarkGray))
            };

            let title_style = if is_focused {
                if item.selected {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)
                }
            } else if item.selected {
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let ch_style = if item.selected { Style::default().fg(Color::Yellow) } else { Style::default().fg(Color::DarkGray) };

            let status_extra = if !item.selected { " (skipped)" } else { "" };

            lines.push(Line::from(vec![
                cursor_span,
                check_span,
                Span::styled(item.manga_title.as_str(), title_style),
                Span::styled(format!(": Ch. {}{}", item.chapter_numbers, status_extra), ch_style),
            ]));
        }

        Paragraph::new(lines).render(inner, buf);
    }
}

#[derive(Debug)]
pub struct DownloadQueueModal<'a> {
    progress: Option<&'a crate::backend::download_queue::DownloadQueueProgress>,
}

impl<'a> DownloadQueueModal<'a> {
    pub fn new(progress: Option<&'a crate::backend::download_queue::DownloadQueueProgress>) -> Self {
        Self { progress }
    }
}

impl<'a> Widget for DownloadQueueModal<'a> {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let area = centered_rect(area, 60, 40);
        Clear.render(area, buf);

        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .title(Line::from(vec![Span::styled(
                tr(" ⬇ Cola de Descargas ", " ⬇ Download Queue "),
                Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan),
            )]))
            .title_bottom(Line::from(vec![Span::styled(
                tr(" <c> Cancelar todo | <q>, <Esc> o <D> Cerrar ", " <c> Cancel All | <q> or <Esc> or <D> Close "),
                Style::default().fg(Color::Yellow),
            )]));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 3 || inner.width < 10 {
            return;
        }

        let mut lines = Vec::new();
        match self.progress {
            Some(p) if p.is_downloading => {
                lines.push(Line::from(vec![
                    Span::styled(
                        tr("Descargando actualmente: ", "Currently downloading: "),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(&p.current_manga_title, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                ]));
                lines.push(Line::from(vec![Span::raw(format!(
                    "{} {}  |  {} {:.0}%",
                    tr("Capítulo:", "Chapter:"),
                    p.current_chapter_number,
                    tr("Progreso pág.:", "Page progress:"),
                    p.current_page_percentage
                ))]));

                let mut batch_text = format!(
                    "{} {} {} {} {} ({} {})",
                    tr("Lote:", "Batch:"),
                    p.completed_in_batch,
                    tr("de", "of"),
                    p.total_in_batch,
                    tr("completados", "completed"),
                    p.remaining_items,
                    tr("restantes en cola", "remaining in queue")
                );
                if p.failed_in_batch > 0 {
                    batch_text.push_str(&format!(" | {}: {}", tr("Fallidos", "Failed"), p.failed_in_batch));
                }
                lines.push(Line::from(vec![Span::styled(
                    batch_text,
                    Style::default().fg(if p.failed_in_batch > 0 { Color::LightRed } else { Color::Cyan }),
                )]));
                lines.push(Line::raw(""));

                // Gauge representation
                let gauge_width = (inner.width.saturating_sub(10)) as usize;
                let filled = ((p.current_page_percentage / 100.0) * gauge_width as f64).round() as usize;
                let filled = filled.min(gauge_width);
                let empty = gauge_width.saturating_sub(filled);
                let bar = format!("[{}{}] {:.0}%", "█".repeat(filled), "░".repeat(empty), p.current_page_percentage);
                lines.push(Line::from(vec![Span::styled(bar, Style::default().fg(Color::Cyan))]));
            },
            Some(p) if p.remaining_items > 0 => {
                lines.push(Line::from(vec![Span::styled(
                    format!(
                        "{} ({} {})",
                        tr("Cola en espera", "Queue waiting"),
                        p.remaining_items,
                        tr("elementos restantes", "items remaining")
                    ),
                    Style::default().fg(Color::Yellow),
                )]));
            },
            _ => {
                lines.push(Line::raw(tr("No hay descargas activas en la cola.", "No active downloads in queue.")));
                lines.push(Line::raw(""));
                lines.push(Line::from(vec![
                    Span::styled(tr("Consejo: ", "Tip: "), Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(tr(
                        "¡Selecciona capítulos con <Espacio> y pulsa <d> para añadirlos a la cola!",
                        "Select chapters with <Space> and press <d> to add them to the queue!",
                    )),
                ]));
            },
        }

        Paragraph::new(lines).render(inner, buf);
    }
}

#[derive(Debug, Default)]
pub struct HelpModal;

impl HelpModal {
    pub fn new() -> Self {
        Self
    }

    fn key_line<'a>(key: &'a str, desc: &'a str) -> Line<'a> {
        Line::from(vec![
            Span::styled(format!(" {:<10} ", key), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(desc, Style::default().fg(Color::White)),
        ])
    }

    fn section_title<'a>(title: &'a str) -> Line<'a> {
        Line::from(vec![Span::styled(format!(" ▸ {title}"), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))])
    }
}

impl Widget for HelpModal {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let modal_area = centered_rect(area, 78, 76);
        Clear.render(modal_area, buf);

        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .title(Line::from(vec![Span::styled(
                tr(" ⌨ Atajos de Teclado y Ayuda Rápida ", " ⌨ Keyboard Shortcuts & Quick Help "),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )]))
            .title_bottom(
                Line::from(vec![Span::styled(
                    tr(" Pulsa <n>, <q> o <Esc> para cerrar ", " Press <n>, <q> or <Esc> to close "),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                )])
                .alignment(Alignment::Center),
            );

        let inner = block.inner(modal_area).inner(Margin {
            horizontal: 1,
            vertical: 1,
        });
        block.render(modal_area, buf);

        if inner.height < 4 || inner.width < 20 {
            return;
        }

        let [left_col, right_col] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .areas(inner);

        let left_lines = vec![
            Self::section_title(tr("Navegación Global", "Global Navigation")),
            Self::key_line("u / F1", tr("Ir a Inicio", "Go to Home")),
            Self::key_line("i / F2", tr("Ir a Buscar", "Go to Search")),
            Self::key_line("o / F3", tr("Ir a Seguimiento (Biblioteca)", "Go to Feed (Library)")),
            Self::key_line("D", tr("Abrir cola de descargas", "Open download queue")),
            Self::key_line("n / F4", tr("Mostrar/ocultar esta ayuda", "Toggle this help modal")),
            Self::key_line("Ctrl+C", tr("Salir de manga-tui", "Quit manga-tui")),
            Line::raw(""),
            Self::section_title(tr("Buscador (Search)", "Search Page")),
            Self::key_line("s", tr("Escribir en barra de búsqueda", "Focus search input")),
            Self::key_line("Enter", tr("Ejecutar búsqueda / Abrir", "Run search / Open")),
            Self::key_line("j/k/h/l", tr("Moverse por resultados", "Navigate results")),
            Self::key_line("w / b", tr("Página siguiente / anterior", "Next / previous page")),
            Self::key_line("p", tr("Alternar Todos / Proveedor actual", "Toggle All / Current provider")),
            Self::key_line("L", tr("Filtrar idioma (Todos/ES/EN)", "Filter language (All/ES/EN)")),
            Self::key_line("f", tr("Abrir filtros avanzados", "Open advanced filters")),
            Self::key_line("r", tr("Abrir ficha del manga", "Open manga page")),
        ];

        let right_lines = vec![
            Self::section_title(tr("Ficha del Manga", "Manga Page")),
            Self::key_line("j / k", tr("Subir / bajar en capítulos", "Scroll chapters up / down")),
            Self::key_line("r", tr("Leer capítulo seleccionado", "Read selected chapter")),
            Self::key_line("Space", tr("Seleccionar capítulo para lote", "Toggle select chapter")),
            Self::key_line("v / c", tr("Seleccionar página / Limpiar", "Select page / Clear selection")),
            Self::key_line("d / a", tr("Descargar selección / todos", "Download selected / all")),
            Self::key_line("f", tr("Marcar/desmarcar Favorito ★", "Toggle Favorite ★")),
            Self::key_line("P", tr("Migrar a otro proveedor", "Switch manga provider")),
            Self::key_line("l / o", tr("Cambiar idioma / orden", "Change language / order")),
            Self::key_line("Tab / m", tr("Ir a marcador / Guardar marcador", "Read / Save bookmark")),
            Line::raw(""),
            Self::section_title(tr("Biblioteca y Lector", "Library & Reader")),
            Self::key_line("Tab / 1-3", tr("Cambiar pestaña en Seguimiento", "Switch tab in Feed")),
            Self::key_line("j / k", tr("Pasar página en el lector", "Next / prev page in reader")),
            Self::key_line("w / b", tr("Siguiente / anterior capítulo", "Next / prev chapter in reader")),
            Self::key_line("Backspace", tr("Volver atrás", "Go back")),
        ];

        Paragraph::new(left_lines).wrap(Wrap { trim: false }).render(left_col, buf);
        Paragraph::new(right_lines).wrap(Wrap { trim: false }).render(right_col, buf);
    }
}

#[cfg(test)]
// Use in testing
pub fn press_key<T>(page: &mut dyn Component<Actions = T>, key: KeyCode) {
    page.handle_events(Events::Key(key.into()));
}
