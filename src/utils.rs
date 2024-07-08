use ratatui::style::{Color, Stylize};
use ratatui::text::Span;

pub fn set_tags_style(tag: &str) -> Span<'_> {
    match tag.to_lowercase().as_str() {
        "suggestive" => format!(" {tag} ").black().bg(Color::Yellow),
        "gore" | "sexual violence" | "pornographic" | "erotica" => {
            format!(" {tag} ").black().bg(Color::Red)
        }
        "doujinshi" => format!(" {tag} ").bg(Color::Blue),
        _ => format!(" {tag} ").into(),
    }
}
