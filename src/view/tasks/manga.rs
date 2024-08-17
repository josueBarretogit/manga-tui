use tokio::sync::mpsc::UnboundedSender;

use crate::backend::error_log::{write_to_error_log, ErrorType};
use crate::backend::fetch::MangadexClient;
use crate::backend::filter::Languages;
use crate::view::pages::manga::{ChapterOrder, MangaPageEvents};

#[cfg(not(test))]
pub async fn search_chapters_operation(
    manga_id: String,
    page: u32,
    language: Languages,
    chapter_order: ChapterOrder,
    tx: UnboundedSender<MangaPageEvents>,
) {
    let response = MangadexClient::global()
        .get_manga_chapters(manga_id, page, language, chapter_order)
        .await;

    match response {
        Ok(chapters_response) => {
            tx.send(MangaPageEvents::LoadChapters(Some(chapters_response)))
                .ok();
        }
        Err(e) => {
            write_to_error_log(ErrorType::FromError(Box::new(e)));
            tx.send(MangaPageEvents::LoadChapters(None)).ok();
        }
    }
}

#[cfg(test)]
pub async fn search_chapters_operation(
    manga_id: String,
    page: u32,
    language: Languages,
    chapter_order: ChapterOrder,
    tx: UnboundedSender<MangaPageEvents>,
) {
    tx.send(MangaPageEvents::LoadChapters(None));
}
