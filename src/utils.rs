use chrono::NaiveDate;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};
use tui_input::Input;

use crate::backend::manga_provider::mangadex::filter::{TagListItem, TagListItemState};

pub fn set_filter_tags_style(tag: &TagListItem) -> Span<'_> {
    match tag.state {
        TagListItemState::Included => format!(" {} ", tag.name).black().on_green(),
        TagListItemState::Excluded => format!(" {} ", tag.name).black().on_red(),
        TagListItemState::NotSelected => Span::from(tag.name.clone()),
    }
}

/// Convert a `NaiveDate` to a user friendly date, like: "2 days ago"
pub fn display_dates_since_publication(date: NaiveDate) -> String {
    let today = chrono::offset::Local::now().date_naive();

    let difference = today - date;
    let day = difference.num_days();
    let month = (day as f64 / 30.44) as i64;
    let year = (day as f64 / 364.0) as i64;
    if day <= 31 {
        format!("{} days ago", day.abs())
    } else if month <= 12 {
        return format!("{month} months ago");
    } else {
        return format!("{year} years ago");
    }
}

pub fn centered_rect(r: Rect, percent_x: u16, percent_y: u16) -> Rect {
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

    match is_typing {
        true => frame.set_cursor(area.x + ((input.visual_cursor()).max(scroll) - scroll) as u16 + 1, area.y + 1),
        false => {},
    }
}
