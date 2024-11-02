use manga_tui::SearchTerm;
use tokio::sync::mpsc::UnboundedSender;

use crate::backend::error_log::{write_to_error_log, ErrorType};
use crate::backend::fetch::ApiClient;
use crate::backend::filter::Filters;
use crate::utils::decode_bytes_to_image;
use crate::view::pages::search::SearchPageEvents;

/// This function searchs for mangas and send a `SearchPageEvents::LoadMangasFound` event
pub async fn search_mangas_operation(
    api_client: impl ApiClient,
    search_by_manga_title: Option<SearchTerm>,
    page: u32,
    filters: Filters,
    tx: UnboundedSender<SearchPageEvents>,
) {
    let search_response = api_client.search_mangas(search_by_manga_title, page, filters).await;
    match search_response {
        Ok(mangas_found) => {
            if let Ok(data) = mangas_found.json().await {
                tx.send(SearchPageEvents::LoadMangasFound(Some(data))).ok();
            }
        },
        Err(e) => {
            write_to_error_log(ErrorType::Error(Box::new(e)));
            tx.send(SearchPageEvents::LoadMangasFound(None)).ok();
        },
    }
}

pub async fn search_manga_covers(
    api_client: impl ApiClient,
    manga_id: String,
    file_name: String,
    tx: UnboundedSender<SearchPageEvents>,
) {
    let search_cover_response = api_client.get_cover_for_manga_lower_quality(&manga_id, &file_name).await;
    if let Ok(response) = search_cover_response {
        if let Ok(bytes) = response.bytes().await {
            let decoding_operation = decode_bytes_to_image(bytes);
            tx.send(SearchPageEvents::LoadCover(decoding_operation.ok(), manga_id)).ok();
        }
    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc::unbounded_channel;

    use super::*;
    use crate::backend::api_responses::SearchMangaResponse;
    use crate::backend::fetch::fake_api_client::MockMangadexClient;

    #[tokio::test]
    async fn search_mangas_task() {
        let (tx, mut rx) = unbounded_channel::<SearchPageEvents>();

        let expected = SearchMangaResponse::default();

        search_mangas_operation(MockMangadexClient::new(), None, 1, Filters::default(), tx).await;

        let event = rx.recv().await.expect("LoadMangasFound event not sent");

        assert_eq!(SearchPageEvents::LoadMangasFound(Some(expected)), event);
    }

    #[tokio::test]
    async fn search_mangas_cover() {
        let (tx, mut rx) = unbounded_channel::<SearchPageEvents>();

        let manga_id = String::from("manga_id");

        search_manga_covers(MockMangadexClient::new(), manga_id.clone(), String::default(), tx).await;

        let event = rx.recv().await.expect("LoadCover event not sent");

        match event {
            SearchPageEvents::LoadCover(_image, id) => {
                assert_eq!(id, manga_id);
            },
            _ => panic!("wrong event was sent"),
        }
    }
}
