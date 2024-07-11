use std::io::Cursor;

use image::io::Reader;
use ratatui::style::{Color, Stylize};
use ratatui::text::Span;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinSet;

use crate::backend::fetch::MangadexClient;
use crate::view::pages::search::SearchPageEvents;
use crate::view::widgets::ImageHandler;

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
                    Err(_) => {
                        tx.send(IM::not_found(manga_id)).unwrap();
                    }
                };
            }
            Err(_) => tx.send(IM::not_found(manga_id)).unwrap(),
        }
    });
}
