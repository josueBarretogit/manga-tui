use tokio::sync::mpsc::UnboundedSender;

use crate::backend::error_log::{write_to_error_log, ErrorType};
use crate::backend::manga_provider::mangadex::api_responses::feed::OneMangaResponse;
use crate::backend::manga_provider::mangadex::ApiClient;
use crate::backend::tui::Events;
use crate::utils::from_manga_response;
use crate::view::pages::feed::FeedEvents;

pub async fn search_manga<T: ApiClient>(
    api_client: T,
    manga_id: String,
    sender: UnboundedSender<Events>,
    feed_page_sender: UnboundedSender<FeedEvents>,
) {
    let response = api_client.get_one_manga(&manga_id).await;

    match response {
        Ok(res) => {
            if let Ok(manga) = res.json::<OneMangaResponse>().await {
                let manga_found = from_manga_response(manga.data);
                //sender.send(Events::GoToMangaPage(MangaItem::new(manga_found))).ok();
            }
        },
        Err(e) => {
            write_to_error_log(ErrorType::Error(Box::new(e)));
            feed_page_sender.send(FeedEvents::ErrorSearchingMangaData).ok();
        },
    }
}

pub async fn search_latest_chapters<T: ApiClient>(api_client: T, manga_id: String, sender: UnboundedSender<FeedEvents>) {
    let latest_chapter_response = api_client.get_latest_chapters(&manga_id).await;
    match latest_chapter_response {
        Ok(res) => {
            if let Ok(chapter_data) = res.json().await {
                sender.send(FeedEvents::LoadRecentChapters(manga_id, Some(chapter_data))).ok();
            }
        },
        Err(e) => {
            write_to_error_log(ErrorType::Error(Box::new(e)));
            sender.send(FeedEvents::LoadRecentChapters(manga_id, None)).ok();
        },
    }
}
