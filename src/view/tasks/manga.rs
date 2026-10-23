use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;

use crate::backend::AppDirectories;
use crate::backend::database::{Database, SetChapterDownloaded};
use crate::backend::error_log::write_to_error_log;
use crate::backend::manga_downloader::cbz_downloader::CbzDownloader;
use crate::backend::manga_downloader::epub_downloader::EpubDownloader;
use crate::backend::manga_downloader::pdf_downloader::PdfDownloader;
use crate::backend::manga_downloader::raw_images::RawImagesDownloader;
use crate::backend::manga_downloader::{ChapterToDownloadSanitized, MangaDownloader};
use crate::backend::manga_provider::{Chapter, Languages, LatestChapter, MangaPageProvider, MangaProvider, MangaProviders};
use crate::backend::tracker::{MangaTracker, track_manga};
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

async fn download_chapters_batch<T: MangaPageProvider>(
    chapters: Vec<Chapter>,
    client: Arc<T>,
    manga_id: String,
    manga_id_safe_for_download: String,
    manga_title: String,
    lang: Languages,
    config: MangaTuiConfig,
    tx: UnboundedSender<MangaPageEvents>,
) {
    let total_chapters = chapters.len();
    tx.send(MangaPageEvents::StartDownloadProgress(total_chapters as f64)).ok();

    let download_chapter_delay = get_download_delay(total_chapters);

    for chapter in chapters {
        let manga_id = manga_id.clone();
        let manga_id_safe_for_download = manga_id_safe_for_download.clone();
        let manga_title = manga_title.clone();
        let client = Arc::clone(&client);
        let inner_tx = tx.clone();

        tokio::time::sleep(Duration::from_secs(download_chapter_delay)).await;
        let chapter_pages_response = client.get_chapter_pages(&chapter.id, &manga_id, config.image_quality, |_, _| {}).await;
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
                    language: lang,
                    scanlator: chapter.scanlator.unwrap_or_default().into(),
                    download_type: config.download_type,
                    pages,
                    provider: Some(client.name()),
                };

                let downloader: &dyn MangaDownloader = match config.download_type {
                    DownloadType::Cbz => &CbzDownloader::new(),
                    DownloadType::Raw => &RawImagesDownloader::new(),
                    DownloadType::Epub => &EpubDownloader::new(),
                    DownloadType::Pdf => &PdfDownloader::new(),
                };

                let download_result =
                    downloader.save_chapter_in_file_system(&AppDirectories::MangaDownloads.get_full_path(), chapter_to_download);

                if let Err(e) = download_result {
                    write_to_error_log(
                        format!("failed to download chapter : {original_chapter_title}, details about the error : {e}").into(),
                    );
                }

                inner_tx.send(MangaPageEvents::SetDownloadAllChaptersProgress).ok();
                inner_tx
                    .send(MangaPageEvents::SaveChapterDownloadStatus(chapter_id, original_chapter_title))
                    .ok();
            },
            Err(e) => {
                write_to_error_log(format!("failed to download chapter : {}, details about the error : {e}", chapter.title).into());
                inner_tx.send(MangaPageEvents::SetDownloadAllChaptersProgress).ok();
            },
        }
    }
}

pub async fn download_all_chapters<T: MangaPageProvider>(args: DownloadAllChapters<T>) {
    let chapters = match args.client.get_all_chapters(&args.manga_id, args.lang).await {
        Ok(chapters) => chapters,
        Err(e) => {
            args.tx.send(MangaPageEvents::DownloadAllChaptersError).ok();
            write_to_error_log(
                format!(
                    "could not get all chapter for manga {} with id {} more details about the error: \n{e}",
                    args.manga_title, args.manga_id
                )
                .into(),
            );
            return;
        },
    };

    download_chapters_batch(
        chapters,
        args.client,
        args.manga_id,
        args.manga_id_safe_for_download,
        args.manga_title,
        args.lang,
        args.config,
        args.tx,
    )
    .await;
}

#[derive(Debug)]
pub struct DownloadSelectedChapters<T: MangaPageProvider> {
    pub client: Arc<T>,
    pub manga_id: String,
    pub manga_id_safe_for_download: String,
    pub manga_title: String,
    pub lang: Languages,
    pub chapters: Vec<Chapter>,
    pub config: MangaTuiConfig,
    pub tx: UnboundedSender<MangaPageEvents>,
}

pub async fn download_selected_chapters<T: MangaPageProvider>(args: DownloadSelectedChapters<T>) {
    download_chapters_batch(
        args.chapters,
        args.client,
        args.manga_id,
        args.manga_id_safe_for_download,
        args.manga_title,
        args.lang,
        args.config,
        args.tx,
    )
    .await;
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
                provider: Some(args.client.name()),
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

#[derive(Debug, Clone)]
pub struct AutoDownloadItem {
    pub provider: MangaProviders,
    pub manga_id: String,
    pub chapter: LatestChapter,
}

pub async fn download_new_favorite_chapters(
    items: Vec<AutoDownloadItem>,
    config: MangaTuiConfig,
) -> Result<(usize, Vec<String>), String> {
    let mut downloaded_count = 0;
    let mut downloaded_details = Vec::new();

    let mut grouped = std::collections::BTreeMap::<(MangaProviders, String), Vec<LatestChapter>>::new();
    for item in items {
        grouped.entry((item.provider, item.manga_id)).or_default().push(item.chapter);
    }

    for ((provider, manga_id), latest_chapters) in grouped {
        let manga = match crate::backend::multi_provider::get_manga_by_id_for_provider(provider, &manga_id).await {
            Ok(m) => m,
            Err(e) => {
                write_to_error_log(format!("Could not load manga details for {manga_id} on {provider}: {e}").into());
                continue;
            },
        };

        let latest_ids: std::collections::HashSet<&str> = latest_chapters.iter().map(|c| c.id.as_str()).collect();
        let mut chapters_to_download = Vec::new();
        let mut checked_languages = Vec::new();

        for latest in &latest_chapters {
            if checked_languages.contains(&latest.language) {
                continue;
            }
            checked_languages.push(latest.language);

            let all_chapters =
                match crate::backend::multi_provider::get_all_chapters_for_provider(provider, &manga_id, latest.language).await {
                    Ok(chaps) => chaps,
                    Err(e) => {
                        write_to_error_log(format!("Could not load chapters for {} on {provider}: {e}", manga.title).into());
                        continue;
                    },
                };
            chapters_to_download.extend(all_chapters.into_iter().filter(|c| latest_ids.contains(c.id.as_str())));
        }

        let mut is_first_chapter = true;
        for chapter in chapters_to_download {
            if !is_first_chapter {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            is_first_chapter = false;

            let pages = match crate::backend::multi_provider::get_chapter_pages_for_provider(
                provider,
                &chapter.id,
                &manga_id,
                config.image_quality,
                |_, _| {},
            )
            .await
            {
                Ok(p) => p,
                Err(e) => {
                    write_to_error_log(
                        format!("Could not fetch chapter {} for {} on {provider}: {e}", chapter.chapter_number, manga.title).into(),
                    );
                    continue;
                },
            };

            let chapter_title = chapter.title.clone();
            let chapter_id = chapter.id.clone();
            let chapter_to_download = ChapterToDownloadSanitized {
                chapter_id: chapter.id_safe_for_download,
                manga_id: manga.id_safe_for_download.clone(),
                manga_title: manga.title.clone().into(),
                chapter_title: chapter.title.into(),
                chapter_number: chapter.chapter_number.clone(),
                volume_number: chapter.volume_number,
                language: chapter.language,
                scanlator: chapter.scanlator.unwrap_or_default().into(),
                download_type: config.download_type,
                pages,
                provider: Some(provider),
            };

            let downloader: &dyn MangaDownloader = match config.download_type {
                DownloadType::Cbz => &CbzDownloader::new(),
                DownloadType::Raw => &RawImagesDownloader::new(),
                DownloadType::Epub => &EpubDownloader::new(),
                DownloadType::Pdf => &PdfDownloader::new(),
            };

            if let Err(e) =
                downloader.save_chapter_in_file_system(&AppDirectories::MangaDownloads.get_full_path(), chapter_to_download)
            {
                write_to_error_log(format!("Could not save chapter {chapter_title}: {e}").into());
                continue;
            }

            if let Ok(connection) = Database::get_connection() {
                let _ = Database::new(&connection).set_chapter_downloaded(SetChapterDownloaded {
                    id: &chapter_id,
                    title: &chapter_title,
                    manga_id: &manga_id,
                    manga_title: &manga.title,
                    img_url: Some(&manga.cover_img_url),
                    provider,
                });
            }

            downloaded_count += 1;
            downloaded_details.push(format!("{} c.{}", manga.title, chapter.chapter_number));
        }
    }

    Ok((downloaded_count, downloaded_details))
}

pub async fn download_new_chapters<T: MangaProvider>(
    client: Arc<T>,
    latest_chapters: Vec<LatestChapter>,
    config: MangaTuiConfig,
) -> Result<usize, String> {
    let mut chapters_by_manga = std::collections::BTreeMap::<String, Vec<LatestChapter>>::new();
    for chapter in latest_chapters {
        chapters_by_manga.entry(chapter.manga_id.clone()).or_default().push(chapter);
    }

    let mut downloaded_count = 0;
    for (manga_id, latest_chapters) in chapters_by_manga {
        let manga = client
            .get_manga_by_id(&manga_id)
            .await
            .map_err(|error| format!("Could not load manga details for {manga_id}: {error}"))?;

        let latest_ids: std::collections::HashSet<&str> = latest_chapters.iter().map(|chapter| chapter.id.as_str()).collect();
        let mut chapters_to_download = Vec::new();
        let mut checked_languages = Vec::new();
        for latest in &latest_chapters {
            if checked_languages.contains(&latest.language) {
                continue;
            }
            checked_languages.push(latest.language);

            let chapters = client
                .get_all_chapters(&manga_id, latest.language)
                .await
                .map_err(|error| format!("Could not load chapters for {}: {error}", manga.title))?;
            chapters_to_download.extend(chapters.into_iter().filter(|chapter| latest_ids.contains(chapter.id.as_str())));
        }

        for latest in &latest_chapters {
            if !chapters_to_download.iter().any(|chapter| chapter.id == latest.id) {
                return Err(format!("Could not resolve chapter {} for {}", latest.chapter_number, manga.title));
            }
        }

        let mut is_first_chapter = true;
        for chapter in chapters_to_download {
            if !is_first_chapter {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            is_first_chapter = false;

            let pages = client
                .get_chapter_pages(&chapter.id, &manga_id, config.image_quality, |_, _| {})
                .await
                .map_err(|error| format!("Could not fetch chapter {}: {error}", chapter.chapter_number))?;

            let chapter_title = chapter.title.clone();
            let chapter_id = chapter.id.clone();
            let chapter_to_download = ChapterToDownloadSanitized {
                chapter_id: chapter.id_safe_for_download,
                manga_id: manga.id_safe_for_download.clone(),
                manga_title: manga.title.clone().into(),
                chapter_title: chapter.title.into(),
                chapter_number: chapter.chapter_number,
                volume_number: chapter.volume_number,
                language: chapter.language,
                scanlator: chapter.scanlator.unwrap_or_default().into(),
                download_type: config.download_type,
                pages,
                provider: Some(client.name()),
            };
            let downloader: &dyn MangaDownloader = match config.download_type {
                DownloadType::Cbz => &CbzDownloader::new(),
                DownloadType::Raw => &RawImagesDownloader::new(),
                DownloadType::Epub => &EpubDownloader::new(),
                DownloadType::Pdf => &PdfDownloader::new(),
            };
            downloader
                .save_chapter_in_file_system(&AppDirectories::MangaDownloads.get_full_path(), chapter_to_download)
                .map_err(|error| format!("Could not save chapter {chapter_title}: {error}"))?;

            let connection = Database::get_connection().map_err(|error| error.to_string())?;
            Database::new(&connection)
                .set_chapter_downloaded(SetChapterDownloaded {
                    id: &chapter_id,
                    title: &chapter_title,
                    manga_id: &manga_id,
                    manga_title: &manga.title,
                    img_url: Some(&manga.cover_img_url),
                    provider: client.name(),
                })
                .map_err(|error| error.to_string())?;
            downloaded_count += 1;
        }
    }

    Ok(downloaded_count)
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
