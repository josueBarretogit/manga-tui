use std::error::Error;

use bytes::Bytes;
use image::io::Reader;
use image::GenericImageView;
use tokio::sync::mpsc::UnboundedSender;

use crate::backend::error_log::{write_to_error_log, ErrorType};
use crate::backend::fetch::MangadexClient;
use crate::view::pages::reader::{MangaReaderEvents, PageData};

pub async fn get_manga_panel(endpoint: String, file_name: String, tx: UnboundedSender<MangaReaderEvents>, page_index: usize) {
    let image_response = MangadexClient::global().get_chapter_page(&endpoint, &file_name).await;
    #[cfg(not(test))]
    match image_response {
        Ok(bytes) => {
            let procces_image_task = convert_bytes_to_manga_panel(bytes, page_index);

            if let Ok(page_data) = procces_image_task {
                tx.send(MangaReaderEvents::LoadPage(Some(page_data))).ok();
            }
        },
        Err(e) => {
            write_to_error_log(ErrorType::FromError(Box::new(e)));
        },
    };
}

/// From the api response convert the bytes to pageData
fn convert_bytes_to_manga_panel(bytes: Bytes, page_index: usize) -> Result<PageData, Box<dyn Error>> {
    let dyn_img = Reader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    let decoded = dyn_img.decode()?;
    let page_data = PageData {
        dimensions: decoded.dimensions(),
        img: decoded,
        index: page_index,
    };
    Ok(page_data)
}
