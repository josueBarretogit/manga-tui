use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;

use crate::backend::error_log::write_to_error_log;
use crate::backend::manga_downloader::cbz_downloader::CbzDownloader;
use crate::backend::manga_downloader::epub_downloader::EpubDownloader;
use crate::backend::manga_downloader::pdf_downloader::PdfDownloader;
use crate::backend::manga_downloader::raw_images::RawImagesDownloader;
use crate::backend::manga_downloader::{ChapterToDownloadSanitized, MangaDownloader};
use crate::backend::manga_provider::{Chapter, Languages, MangaPageProvider};
use crate::backend::tracker::{track_manga, MangaTracker};
use crate::backend::AppDirectories;
use crate::common::format_error_message_tracking_reading_history;
use crate::config::{DownloadType, MangaTuiConfig};
use crate::view::pages::manga::MangaPageEvents;

#[derive(Debug)]
pub struct DownloadAllChapters<T: MangaPageProvider> {
    pub client: Arc<T>,
    pub manga_id: String,
    pub manga_id_safe_for_download: String,
    pub manga_title: String,
    pub lang: Languages,
    pub config: MangaTuiConfig,
    pub tx: UnboundedSender<MangaPageEvents>,
}

// This is needed in order to avoid any api request limits
fn get_download_delay(total_chapters: usize) -> u64 {
    if total_chapters < 40 {
        2
    } else if (40..100).contains(&total_chapters) {
        4
    } else {
        8
    }
}

pub async fn download_all_chapters<T: MangaPageProvider>(args: DownloadAllChapters<T>) {
    let get_all_chapters_response = args.client.get_all_chapters(&args.manga_id, args.lang).await;

    match get_all_chapters_response {
        Ok(chapters) => {
            let total_chapters = chapters.len();

            args.tx.send(MangaPageEvents::StartDownloadProgress(total_chapters as f64)).ok();

            let download_chapter_delay = get_download_delay(total_chapters);

            for chapter in chapters {
                let manga_id = args.manga_id.clone();
                let manga_id_safe_for_download = args.manga_id_safe_for_download.clone();
                let manga_title = args.manga_title.clone();
                let client = Arc::clone(&args.client);
                let inner_tx = args.tx.clone();

                sleep(Duration::from_secs(download_chapter_delay));
                tokio::spawn(async move {
                    let chapter_pages_response = client
                        .get_chapter_pages(&chapter.id, &manga_id, args.config.image_quality, |_, _| {})
                        .await;
                    match chapter_pages_response {
                        Ok(pages) => {
                            let original_chapter_title = chapter.title.clone();
                            let chapter_id = chapter.id.clone();
                            let chapter_to_download: ChapterToDownloadSanitized = ChapterToDownloadSanitized {
                                chapter_id: chapter.id_safe_for_download,
                                manga_id: manga_id_safe_for_download,
                                manga_title: manga_title.into(),
                                chapter_title: chapter.title.into(),
                                chapter_number: chapter.chapter_number,
                                volume_number: chapter.volume_number,
                                language: args.lang,
                                scanlator: chapter.scanlator.unwrap_or_default().into(),
                                download_type: args.config.download_type,
                                pages,
                            };

                            let downloader: &dyn MangaDownloader = match args.config.download_type {
                                DownloadType::Cbz => &CbzDownloader::new(),
                                DownloadType::Raw => &RawImagesDownloader::new(),
                                DownloadType::Epub => &EpubDownloader::new(),
                                DownloadType::Pdf => &PdfDownloader::new(),
                            };

                            let download_result = downloader
                                .save_chapter_in_file_system(&AppDirectories::MangaDownloads.get_full_path(), chapter_to_download);

                            if let Err(e) = download_result {
                                write_to_error_log(
                                    format!(
                                        "failed to download chapter : {}, details about the error : {e}",
                                        original_chapter_title
                                    )
                                    .into(),
                                );
                            }

                            inner_tx.send(MangaPageEvents::SetDownloadAllChaptersProgress).ok();
                            inner_tx
                                .send(MangaPageEvents::SaveChapterDownloadStatus(chapter_id, original_chapter_title))
                                .ok();
                        },
                        Err(e) => {
                            write_to_error_log(
                                format!("failed to download chapter : {}, details about the error : {e}", chapter.title).into(),
                            );
                            inner_tx.send(MangaPageEvents::SetDownloadAllChaptersProgress).ok();
                        },
                    }
                });
            }
        },
        Err(e) => {
            args.tx.send(MangaPageEvents::DownloadAllChaptersError).ok();
            write_to_error_log(
                format!(
                    "could not get all chapter for manga {} with id {} more details about the error: \n{e}",
                    args.manga_title, args.manga_id
                )
                .into(),
            );
        },
    }
}

#[derive(Debug)]
pub struct DownloadSingleChapter<T: MangaPageProvider, S: MangaTracker> {
    pub client: Arc<T>,
    pub manga_tracker: Option<S>,
    pub manga_id: String,
    pub manga_id_safe_for_download: String,
    pub manga_title: String,
    pub chapter: Chapter,
    pub config: MangaTuiConfig,
    pub tx: UnboundedSender<MangaPageEvents>,
}

pub async fn download_single_chapter<T: MangaPageProvider, S: MangaTracker>(args: DownloadSingleChapter<T, S>) {
    let sender_report_progress = args.tx.clone();
    let pages_bytes = args
        .client
        .get_chapter_pages(&args.chapter.id, &args.manga_id, args.config.image_quality, move |percentage, chapter_id| {
            sender_report_progress
                .send(MangaPageEvents::SetDownloadProgress(percentage, chapter_id.to_string()))
                .ok();
        })
        .await;

    let tx = args.tx;
    match pages_bytes {
        Ok(pages) => {
            let original_chapter_title = args.chapter.title.clone();
            let chapter_id = args.chapter.id.clone();
            let chapter_number = args.chapter.chapter_number.clone();
            let volume_number = args.chapter.volume_number.clone();
            let manga_title_copy = args.manga_title.clone();
            let chapter_to_download: ChapterToDownloadSanitized = ChapterToDownloadSanitized {
                chapter_id: args.chapter.id_safe_for_download,
                manga_id: args.manga_id_safe_for_download,
                manga_title: args.manga_title.into(),
                chapter_title: args.chapter.title.into(),
                chapter_number: args.chapter.chapter_number,
                volume_number: args.chapter.volume_number,
                language: args.chapter.language,
                scanlator: args.chapter.scanlator.unwrap_or_default().into(),
                download_type: args.config.download_type,
                pages,
            };
            if args.config.track_reading_when_download {
                // clone chapter title so that it can be used inside `track_manga` error
                // closure
                let chapter_title_error = original_chapter_title.clone();
                track_manga(
                    args.manga_tracker,
                    manga_title_copy.clone(),
                    // This conversion is needed so that we take into account chapters
                    // like 1.2, 10.1 etc
                    chapter_number.parse::<f64>().unwrap_or(0.0) as u32,
                    volume_number.and_then(|vol| vol.parse().ok()),
                    move |error| {
                        write_to_error_log(
                            format_error_message_tracking_reading_history(
                                chapter_title_error.clone(),
                                manga_title_copy.clone(),
                                error,
                            )
                            .into(),
                        );
                    },
                );
            }

            let downloader: &dyn MangaDownloader = match args.config.download_type {
                DownloadType::Cbz => &CbzDownloader::new(),
                DownloadType::Raw => &RawImagesDownloader::new(),
                DownloadType::Epub => &EpubDownloader::new(),
                DownloadType::Pdf => &PdfDownloader::new(),
            };

            match downloader.save_chapter_in_file_system(&AppDirectories::MangaDownloads.get_full_path(), chapter_to_download) {
                Ok(()) => {
                    tx.send(MangaPageEvents::SaveChapterDownloadStatus(chapter_id.clone(), original_chapter_title))
                        .ok();
                    tx.send(MangaPageEvents::ChapterFinishedDownloading(chapter_id)).ok();
                },
                Err(e) => {
                    write_to_error_log(e.into());
                    tx.send(MangaPageEvents::DownloadError(chapter_id)).ok();
                },
            };
        },
        Err(e) => {
            write_to_error_log(e.into());
            tx.send(MangaPageEvents::DownloadError(args.chapter.id)).ok();
        },
    }
}

#[cfg(test)]
mod tests {
    //use std::fs;
    //use std::ops::AddAssign;
    //use std::path::{Path, PathBuf};
    //
    //use fake::faker::name::en::Name;
    //use fake::Fake;
    //use manga_tui::exists;
    //use pretty_assertions::assert_eq;
    //use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
    //use uuid::Uuid;
    //
    //use super::*;
    //use crate::backend::api_responses::{ChapterAttribute, ChapterData};
    //use crate::backend::fetch::fake_api_client::MockMangadexClient;
    //
    //async fn validate_progress_sent(
    //    mut rx: UnboundedReceiver<MangaPageEvents>,
    //    expected_amount_files: f64,
    //    expected_id_sent: String,
    //) {
    //    let mut iterations = 0.0;
    //    for _ in 0..(expected_amount_files as usize) {
    //        let event = rx.recv().await.expect("no event was sent");
    //        match event {
    //            MangaPageEvents::SetDownloadProgress(ratio_progress, manga_id) => {
    //                assert_eq!(manga_id, expected_id_sent);
    //                assert_eq!(iterations / expected_amount_files, ratio_progress);
    //                iterations.add_assign(1.0);
    //            },
    //            _ => panic!("wrong event was sent"),
    //        }
    //    }
    //}
    //
    //async fn validate_download_all_chapter_progress(mut rx: UnboundedReceiver<MangaPageEvents>, total_chapters: f64) {
    //    for _ in 0..(total_chapters as usize) {
    //        let event = rx.recv().await.expect("no event was sent");
    //        match event {
    //            MangaPageEvents::SetDownloadAllChaptersProgress => {},
    //            MangaPageEvents::SaveChapterDownloadStatus(_, _) => {},
    //            _ => panic!("wrong event was sent"),
    //        }
    //    }
    //}
    //
    //fn create_tests_directory() -> Result<PathBuf, std::io::Error> {
    //    let base_directory = Path::new("./test_results/manga_page_tasks");
    //
    //    if !exists!(&base_directory) {
    //        fs::create_dir_all(base_directory)?;
    //    }
    //
    //    Ok(base_directory.to_path_buf())
    //}
    //
    //fn get_chapter_for_testing() -> DownloadChapter {
    //    DownloadChapter::new(
    //        &Uuid::new_v4().to_string(),
    //        &Uuid::new_v4().to_string(),
    //        &Name().fake::<String>(),
    //        &Name().fake::<String>(),
    //        "1",
    //        &Name().fake::<String>(),
    //        &Languages::default().as_human_readable(),
    //    )
    //}
    //
    //#[tokio::test]
    //#[ignore]
    //async fn download_a_chapter_given_a_api_response_raw_images_reporting_pages_progress() -> Result<(), Box<dyn Error>> {
    //    let chapter_to_download = get_chapter_for_testing();
    //    let directory_to_download = create_tests_directory()?;
    //
    //    let (sender_progress, receiver_progress) = unbounded_channel::<MangaPageEvents>();
    //    let expected_amount_files = 3;
    //    let chapter_id = Uuid::new_v4().to_string();
    //    let report_progress = true;
    //
    //    download_chapter_task(
    //        chapter_to_download.clone(),
    //        MockMangadexClient::new().with_amount_returning_items(expected_amount_files),
    //        ImageQuality::Low,
    //        directory_to_download.clone(),
    //        DownloadType::Raw,
    //        chapter_id.clone(),
    //        report_progress,
    //        sender_progress,
    //    )
    //    .await?;
    //
    //    validate_progress_sent(receiver_progress, expected_amount_files as f64, chapter_id).await;
    //
    //    Ok(())
    //}
    //
    //#[tokio::test]
    //#[ignore]
    //async fn download_a_chapter_given_a_api_response_cbz() -> Result<(), Box<dyn std::error::Error>> {
    //    let (sender_progress, receiver_progress) = unbounded_channel::<MangaPageEvents>();
    //    let expected_amount_files = 3;
    //
    //    let chapter_id = Uuid::new_v4().to_string();
    //    let report_progress = true;
    //
    //    download_chapter_task(
    //        get_chapter_for_testing(),
    //        MockMangadexClient::new().with_amount_returning_items(expected_amount_files),
    //        ImageQuality::Low,
    //        create_tests_directory()?,
    //        DownloadType::Cbz,
    //        chapter_id.clone(),
    //        report_progress,
    //        sender_progress,
    //    )
    //    .await?;
    //
    //    validate_progress_sent(receiver_progress, expected_amount_files as f64, chapter_id).await;
    //
    //    Ok(())
    //}
    //
    //#[tokio::test]
    //#[ignore]
    //async fn download_a_chapter_given_a_api_response_epub_with_progress() -> Result<(), Box<dyn std::error::Error>> {
    //    let (sender_progress, receiver_progress) = unbounded_channel::<MangaPageEvents>();
    //
    //    let expected_amount_files = 3;
    //    let chapter_id = Uuid::new_v4().to_string();
    //    let should_report_progress = true;
    //
    //    download_chapter_task(
    //        get_chapter_for_testing(),
    //        MockMangadexClient::new().with_amount_returning_items(expected_amount_files),
    //        ImageQuality::Low,
    //        create_tests_directory()?,
    //        DownloadType::Epub,
    //        chapter_id.clone(),
    //        should_report_progress,
    //        sender_progress,
    //    )
    //    .await?;
    //
    //    validate_progress_sent(receiver_progress, expected_amount_files as f64, chapter_id).await;
    //
    //    Ok(())
    //}
    //
    //#[tokio::test]
    //#[ignore]
    //async fn download_all_chapters_expected_events() -> Result<(), Box<dyn std::error::Error>> {
    //    let directory_to_download = create_tests_directory()?;
    //    let (sender, mut rx) = unbounded_channel::<MangaPageEvents>();
    //    let total_chapters = 3;
    //
    //    let mut chapters: Vec<ChapterData> = vec![];
    //    for index in 0..total_chapters {
    //        chapters.push(ChapterData {
    //            id: Uuid::new_v4().into(),
    //            type_field: "chapter".into(),
    //            attributes: ChapterAttribute {
    //                chapter: Some(index.to_string()),
    //                ..Default::default()
    //            },
    //            ..Default::default()
    //        })
    //    }
    //
    //    let response = ChapterResponse {
    //        data: chapters,
    //
    //        ..Default::default()
    //    };
    //
    //    let api_client = MockMangadexClient::new().with_amount_returning_items(2).with_chapter_response(response);
    //
    //    let manga_id = Uuid::new_v4().to_string();
    //    let manga_title = Uuid::new_v4().to_string();
    //    let language = Languages::default();
    //    let file_format = DownloadType::Cbz;
    //    let image_quality = ImageQuality::Low;
    //
    //    download_all_chapters(api_client, DownloadAllChapters {
    //        sender,
    //        manga_id,
    //        manga_title,
    //        image_quality,
    //        directory_to_download: directory_to_download.clone(),
    //        file_format,
    //        language,
    //    })
    //    .await?;
    //
    //    let expected_event = rx.recv().await.expect("no event was sent");
    //
    //    assert_eq!(MangaPageEvents::StartDownloadProgress(total_chapters as f64), expected_event);
    //
    //    validate_download_all_chapter_progress(rx, total_chapters as f64).await;
    //
    //    Ok(())
    //}
}
