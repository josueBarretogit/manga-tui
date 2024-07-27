use crate::backend::error_log::write_to_error_log;
use crate::backend::fetch::MangadexClient;
use crate::backend::filter::Languages;
use crate::backend::Data;
use crate::common::{Artist, Author, Manga};
use crate::view::widgets::ImageHandler;
use image::io::Reader;
use ratatui::{prelude::*, widgets::*};
use std::io::Cursor;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinSet;
use tui_input::Input;

pub fn set_tags_style(tag: &str) -> Span<'_> {
    match tag.to_lowercase().as_str() {
        "suggestive" => format!("  {tag}  ").black().bg(Color::Yellow),
        "gore" | "sexual violence" | "pornographic" | "erotica" => {
            format!("  {tag}  ").black().bg(Color::Red)
        }
        "doujinshi" => format!("  {tag}  ").bg(Color::Blue),
        _ => format!("  {tag}  ").into(),
    }
}

pub fn set_status_style(status: &str) -> Span<'_> {
    match status.to_lowercase().as_str() {
        "completed" => format!(" 🔵 {status} ").into(),
        "ongoing" => format!(" 🟢 {status} ").into(),
        "hiatus" => format!(" 🟡 {status} ").into(),
        "cancelled" => format!(" 🟠 {status} ").into(),
        _ => format!(" {status} ").into(),
    }
}

pub fn search_manga_cover<IM: ImageHandler>(
    file_name: String,
    manga_id: String,
    join_set: &mut JoinSet<()>,
    tx: UnboundedSender<IM>,
) {
    join_set.spawn(async move {
        let response = MangadexClient::global()
            .get_cover_for_manga(&manga_id, &file_name)
            .await;

        match response {
            Ok(bytes) => {
                let dyn_img = Reader::new(Cursor::new(bytes))
                    .with_guessed_format()
                    .unwrap();

                let maybe_decoded = dyn_img.decode();

                match maybe_decoded {
                    Ok(image) => {
                        tx.send(IM::load(image, manga_id)).unwrap();
                    }
                    Err(e) => {
                        write_to_error_log(crate::backend::error_log::ErrorType::FromError(
                            Box::new(e),
                        ));
                        tx.send(IM::not_found(manga_id)).unwrap();
                    }
                };
            }
            Err(e) => {
                write_to_error_log(crate::backend::error_log::ErrorType::FromError(Box::new(e)));
                tx.send(IM::not_found(manga_id)).unwrap()
            }
        }
    });
}

pub fn from_manga_response(value: Data) -> Manga {
    let id = value.id.clone();
    // Todo! maybe there is a better way to do this
    // and use preferred language
    let title = value.attributes.title.en.unwrap_or(
        value.attributes.title.ja_ro.unwrap_or(
            value.attributes.title.ja.unwrap_or(
                value.attributes.title.jp.unwrap_or(
                    value.attributes.title.zh.unwrap_or(
                        value
                            .attributes
                            .title
                            .ko
                            .unwrap_or(value.attributes.title.ko_ro.unwrap_or_default()),
                    ),
                ),
            ),
        ),
    );

    let description = match value.attributes.description {
        Some(description) => description.en.unwrap_or("No description".to_string()),
        None => String::from("No description"),
    };

    let content_rating = value.attributes.content_rating;

    let tags: Vec<String> = value
        .attributes
        .tags
        .iter()
        .map(|tag| tag.attributes.name.en.to_string())
        .collect();

    let mut img_url: Option<String> = Option::default();
    let mut author = Author::default();
    let mut artist = Artist::default();

    for rel in &value.relationships {
        if let Some(attributes) = &rel.attributes {
            match rel.type_field.as_str() {
                "author" => {
                    author = Author {
                        id: rel.id.to_string(),
                        name: attributes.name.as_ref().cloned().unwrap_or_default(),
                    };
                }
                "artist" => {
                    artist = Artist {
                        id: rel.id.to_string(),
                        name: attributes.name.as_ref().cloned().unwrap_or_default(),
                    }
                }
                "cover_art" => img_url = Some(attributes.file_name.as_ref().unwrap().to_string()),
                _ => {}
            }
        }
    }

    let languages: Vec<Languages> = value
        .attributes
        .available_translated_languages
        .into_iter()
        .flatten()
        .flat_map(|lang| Languages::try_from_iso_code(&lang))
        .collect();

    let status = value.attributes.status;

    let publication_demographic = value.attributes.publication_demographic.unwrap_or_default();

    Manga {
        id,
        title,
        description,
        content_rating,
        tags,
        status,
        img_url,
        author,
        artist,
        publication_demographic,
        available_languages: languages,
    }
}

pub fn display_dates_since_publication(day: i64) -> String {
    let month = (day as f64 / 30.44) as i64;
    let year = (day as f64 / 364.0) as i64;
    if day <= 31 {
        format!("{} days ago", day.abs())
    } else if month <= 12 {
        return format!("{} months ago", month);
    } else {
        return format!("{} years ago", year);
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

pub fn render_search_bar(
    is_typing: bool,
    input_help: Line<'_>,
    input: &Input,
    frame: &mut Frame<'_>,
    area: Rect,
) {
    let style = if is_typing {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let input_bar = Paragraph::new(input.value())
        .block(Block::bordered().title(input_help).border_style(style));

    input_bar.render(area, frame.buffer_mut());

    let width = area.width.max(3) - 3;

    let scroll = input.visual_scroll(width as usize);

    match is_typing {
        true => frame.set_cursor(
            area.x + ((input.visual_cursor()).max(scroll) - scroll) as u16 + 1,
            area.y + 1,
        ),
        false => {}
    }
}
