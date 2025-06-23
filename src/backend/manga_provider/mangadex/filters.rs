use filter_provider::{TagListItem, TagListItemState};
use ratatui::style::Stylize;
use ratatui::text::Span;

pub mod api_parameter;
pub mod filter_provider;

pub fn set_filter_tags_style(tag: &TagListItem) -> Span<'_> {
    match tag.state {
        TagListItemState::Included => format!(" {} ", tag.name).black().on_green(),
        TagListItemState::Excluded => format!(" {} ", tag.name).black().on_red(),
        TagListItemState::NotSelected => Span::from(tag.name.clone()),
    }
}
