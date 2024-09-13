use std::time::{Duration, Instant};

use tokio::sync::mpsc::UnboundedSender;

use crate::backend::download::DownloadChapter;
use crate::backend::error_log::{self, write_to_error_log, ErrorType};
use crate::backend::fetch::{ApiClient, MangadexClient};
use crate::backend::filter::Languages;
use crate::backend::{ChapterPagesResponse, ChapterResponse};
use crate::common::PageType;
use crate::config::{DownloadType, ImageQuality, MangaTuiConfig};
use crate::view::pages::manga::{ChapterOrder, MangaPageEvents};

#[cfg(not(test))]
pub async fn search_chapters_operation(
    manga_id: String,
    page: u32,
    language: Languages,
    chapter_order: ChapterOrder,
    tx: UnboundedSender<MangaPageEvents>,
) {
    use crate::backend::error_log::{write_to_error_log, ErrorType};
    use crate::backend::fetch::{ApiClient, MangadexClient};
    use crate::backend::ChapterResponse;

    let response = MangadexClient::global()
        .get_manga_chapters(&manga_id, page, language, chapter_order)
        .await;

    match response {
        Ok(chapters_response) => {
            let data: Result<ChapterResponse, reqwest::Error> = chapters_response.json().await;
            if let Ok(chapters) = data {
                tx.send(MangaPageEvents::LoadChapters(Some(chapters))).ok();
            }
        },
        Err(e) => {
            write_to_error_log(ErrorType::FromError(Box::new(e)));
            tx.send(MangaPageEvents::LoadChapters(None)).ok();
        },
    }
}

#[cfg(test)]
pub async fn search_chapters_operation(
    _manga_id: String,
    _page: u32,
    _language: Languages,
    _chapter_order: ChapterOrder,
    tx: UnboundedSender<MangaPageEvents>,
) {
    tx.send(MangaPageEvents::LoadChapters(None)).ok();
}

pub struct DownloadAllChaptersData {
    pub tx: UnboundedSender<MangaPageEvents>,
    pub manga_id: String,
    pub manga_title: String,
    pub lang: Languages,
}

pub async fn download_all_chapters_task(data: DownloadAllChaptersData) {
    let chapter_response = MangadexClient::global().get_all_chapters_for_manga(&data.manga_id, data.lang).await;

    match chapter_response {
        Ok(response) => {
            if let Ok(response) = response.json::<ChapterResponse>().await {
                let total_chapters = response.data.len();
                data.tx.send(MangaPageEvents::StartDownloadProgress(total_chapters as f64)).ok();

                let download_chapter_delay = if total_chapters < 40 {
                    1
                } else if (40..100).contains(&total_chapters) {
                    3
                } else if (100..200).contains(&total_chapters) {
                    6
                } else {
                    8
                };

                let config = MangaTuiConfig::get();

                for chapter_found in response.data.into_iter() {
                    let chapter_id = chapter_found.id;

                    let start_fetch_time = Instant::now();

                    let pages_response = MangadexClient::global().get_chapter_pages(&chapter_id).await;

                    let chapter_number = chapter_found.attributes.chapter.unwrap_or_default();

                    let scanlator = chapter_found
                        .relationships
                        .iter()
                        .find(|rel| rel.type_field == "scanlation_group")
                        .map(|rel| rel.attributes.as_ref().unwrap().name.to_string());

                    let chapter_title = chapter_found.attributes.title.unwrap_or_default();
                    let scanlator = scanlator.unwrap_or_default();
                    //
                    //    match pages_response {
                    //        Ok(response) => {
                    //            if let Ok(res) = response.json::<ChapterPagesResponse>().await {
                    //                let (files, quality) = match config.image_quality {
                    //                    ImageQuality::Low => (res.chapter.data_saver, PageType::LowQuality),
                    //                    ImageQuality::High => (res.chapter.data, PageType::HighQuality),
                    //                };
                    //
                    //                let endpoint = format!("{}/{}/{}", res.base_url, quality, res.chapter.hash);
                    //
                    //                let chapter_to_download = DownloadChapter::new(
                    //                    &chapter_id,
                    //                    &data.manga_id,
                    //                    &data.manga_title,
                    //                    &chapter_title,
                    //                    chapter_number.parse().unwrap_or_default(),
                    //                    &scanlator,
                    //                    &data.lang.as_human_readable(),
                    //                );
                    //
                    //                let download_proccess = match config.download_type {
                    //                    DownloadType::Cbz => {
                    //                        download_chapter_cbz(true, chapter_to_download, files, endpoint, data.tx.clone())
                    //                    },
                    //                    DownloadType::Raw => {
                    //                        download_chapter_raw_images(true, chapter_to_download, files, endpoint,
                    // data.tx.clone())                    },
                    //                    DownloadType::Epub => {
                    //                        download_chapter_epub(true, chapter_to_download, files, endpoint, data.tx.clone())
                    //                    },
                    //                };
                    //
                    //                if let Err(e) = download_proccess {
                    //                    let error_message =
                    //                        format!("Chapter: {} could not be downloaded, details: {}", chapter_title, e);
                    //
                    //                    data.tx.send(MangaPageEvents::SetDownloadAllChaptersProgress).ok();
                    //
                    //                    write_to_error_log(ErrorType::FromError(Box::from(error_message)));
                    //                    return;
                    //                }
                    //
                    //                data.tx
                    //                    .send(MangaPageEvents::SaveChapterDownloadStatus(chapter_id, chapter_title.to_string()))
                    //                    .ok();
                    //            }
                    //        },
                    //        Err(e) => {
                    //            let error_message = format!("Chapter: {} could not be downloaded, details: {}", chapter_title, e);
                    //
                    //            data.tx.send(MangaPageEvents::SetDownloadAllChaptersProgress).ok();
                    //            write_to_error_log(ErrorType::FromError(Box::from(error_message)));
                    //        },
                    //    }
                    //
                    //    let time_since = start_fetch_time.elapsed();
                    //    std::thread::sleep(Duration::from_secs(download_chapter_delay).saturating_sub(time_since));
                }
            }
        },
        Err(e) => {
            data.tx.send(MangaPageEvents::DownloadAllChaptersError).ok();
            write_to_error_log(error_log::ErrorType::FromError(Box::new(e)));
        },
    }
}
