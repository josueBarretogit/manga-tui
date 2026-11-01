use chrono::NaiveDate;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph, Widget};
use tui_input::Input;

use crate::i18n::tr;

/// Convert a `NaiveDate` to a user friendly date, like: "2 days ago" or "hace 2 días"
pub fn display_dates_since_publication(date: NaiveDate) -> String {
    let today = chrono::offset::Local::now().date_naive();

    let difference = today - date;
    let day = difference.num_days();

    if day <= 0 {
        return tr("hoy", "today").to_string();
    }

    let month = (day as f64 / 30.44) as i64;
    let year = (day as f64 / 364.0) as i64;

    if day == 1 {
        tr("hace 1 día", "1 day ago").to_string()
    } else if day <= 31 {
        if crate::i18n::is_es() { format!("hace {day} días") } else { format!("{day} days ago") }
    } else if month <= 1 {
        tr("hace 1 mes", "1 month ago").to_string()
    } else if month <= 12 {
        if crate::i18n::is_es() { format!("hace {month} meses") } else { format!("{month} months ago") }
    } else if year <= 1 {
        tr("hace 1 año", "1 year ago").to_string()
    } else if crate::i18n::is_es() {
        format!("hace {year} años")
    } else {
        format!("{year} years ago")
    }
}

pub fn centered_rect(r: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let percent_x = percent_x.min(100);
    let percent_y = percent_y.min(100);

    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn render_search_bar(is_typing: bool, input_help: Line<'_>, input: &Input, frame: &mut Frame<'_>, area: Rect) {
    let style = if is_typing { Style::default().fg(Color::Yellow) } else { Style::default() };

    let input_bar = Paragraph::new(input.value()).block(Block::bordered().title(input_help).border_style(style));

    input_bar.render(area, frame.buffer_mut());

    let width = area.width.max(3) - 3;

    let scroll = input.visual_scroll(width as usize);

    if is_typing {
        frame.set_cursor_position((area.x + ((input.visual_cursor()).max(scroll) - scroll) as u16 + 1, area.y + 1));
    }
}
