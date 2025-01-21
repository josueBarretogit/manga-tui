use reqwest::Url;
use tokio::sync::mpsc::UnboundedSender;

use crate::backend::error_log::{write_to_error_log, ErrorType};
use crate::view::pages::reader::{MangaReaderEvents, PageData, SearchMangaPanel};

pub async fn get_manga_panel(
    client: impl SearchMangaPanel,
    endpoint: Url,
    tx: UnboundedSender<MangaReaderEvents>,
    page_index: usize,
) {
    let response = client.search_manga_panel(endpoint).await;

    match response {
        Ok(panel) => {
            let page = PageData {
                panel,
                index: page_index,
            };
            tx.send(MangaReaderEvents::LoadPage(page)).ok();
        },
        Err(e) => {
            tx.send(MangaReaderEvents::FailedPage(page_index)).ok();
            write_to_error_log(ErrorType::Error(e));
        },
    }
}

#[cfg(test)]
mod test {
    //use httpmock::Method::GET;
    //use httpmock::MockServer;
    //use pretty_assertions::assert_eq;
    //use reqwest::Url;
    //
    //use super::*;
    //use crate::backend::fetch::MangadexClient;
    //
    //#[tokio::test]
    //async fn get_manga_panel_works() {
    //    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<MangaReaderEvents>();
    //
    //    let server = MockServer::start_async().await;
    //    let expect_response = include_bytes!("../../../public/mangadex_support.jpg");
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET).path_contains("filename.png");
    //            then.status(200).body(expect_response);
    //        })
    //        .await;
    //
    //    let base_url: Url = format!("{}/{}", server.base_url(), "filename.png").parse().unwrap();
    //
    //    get_manga_panel(MangadexClient::new(base_url.clone(), base_url.clone()), base_url, tx, 1).await;
    //
    //    request.assert_async().await;
    //
    //    let event = rx.recv().await.expect("could not get manga panel");
    //
    //    let page_data = match event {
    //        MangaReaderEvents::FailedPage(_) => panic!("wrong event was sent"),
    //        MangaReaderEvents::FetchPages => panic!("wrong event was sent"),
    //        MangaReaderEvents::LoadPage(page_data) => page_data,
    //        _ => panic!("wrong event was sent"),
    //    };
    //
    //    assert_eq!(1, page_data.index)
    //}
}
