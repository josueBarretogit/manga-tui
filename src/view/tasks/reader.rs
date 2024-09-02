use std::error::Error;

use bytes::Bytes;
use image::io::Reader;
use image::GenericImageView;
use tokio::sync::mpsc::UnboundedSender;

use crate::backend::error_log::{write_to_error_log, ErrorType};
use crate::backend::fetch::ApiClient;
use crate::view::pages::reader::{MangaReaderEvents, PageData};

pub async fn get_manga_panel(
    client: impl ApiClient,
    endpoint: String,
    file_name: String,
    tx: UnboundedSender<MangaReaderEvents>,
    page_index: usize,
) {
    let image_response = client.get_chapter_page(&endpoint, &file_name).await;
    match image_response {
        Ok(response) => {
            if let Ok(bytes) = response.bytes().await {
                let procces_image_task = convert_bytes_to_manga_panel(bytes, page_index);

                if let Ok(page_data) = procces_image_task {
                    tx.send(MangaReaderEvents::LoadPage(Some(page_data))).ok();
                }
            }
        },
        Err(e) => {
            write_to_error_log(ErrorType::FromError(Box::new(e)));
        },
    };
}

/// From the api response convert the bytes to pageData
fn convert_bytes_to_manga_panel(bytes: Bytes, page_index: usize) -> Result<PageData, Box<dyn Error>> {
    let decoded = Reader::new(std::io::Cursor::new(bytes)).with_guessed_format()?.decode()?;
    Ok(PageData {
        dimensions: decoded.dimensions(),
        img: decoded,
        index: page_index,
    })
}

#[cfg(test)]
mod test {
    use httpmock::Method::GET;
    use httpmock::MockServer;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::backend::fetch::MockApiClient;

    #[test]
    fn convert_bytes_to_page_data() {
        let some_data = include_bytes!("../../../public/mangadex_support.jpg");
        let page_data = convert_bytes_to_manga_panel(some_data.to_vec().into(), 1).expect("operation was not succesful");
        assert_eq!(1, page_data.index);
        assert_eq!((1024, 804), page_data.dimensions)
    }

    #[tokio::test]
    async fn get_manga_panel_works() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<MangaReaderEvents>();

        let server = MockServer::start_async().await;
        let expect_response = include_bytes!("../../../public/mangadex_support.jpg");

        let request = server
            .mock_async(|when, then| {
                when.method(GET).path_contains("filename.png");
                then.status(200).body(expect_response);
            })
            .await;

        get_manga_panel(MockApiClient::new(), server.base_url(), "filename.png".to_string(), tx, 1).await;

        request.assert_async().await;

        let event = rx.recv().await.expect("could not get manga panel");

        let page_data = match event {
            MangaReaderEvents::FetchPages => panic!("wrong event was sent"),
            MangaReaderEvents::LoadPage(page_data) => page_data.expect("should load a page"),
        };

        assert_eq!(1, page_data.index)
    }
}
