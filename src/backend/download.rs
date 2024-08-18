use manga_tui::exists;
use std::fs::{create_dir, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use zip::write::{FileOptions, SimpleFileOptions};
use zip::ZipWriter;

use crate::common::PageType;
use crate::config::{DownloadType, ImageQuality, MangaTuiConfig};
use crate::view::pages::manga::MangaPageEvents;

use super::error_log::{write_to_error_log, ErrorType};
use super::fetch::MangadexClient;
use super::filter::Languages;
use super::{ChapterPagesResponse, ChapterResponse, APP_DATA_DIR};

pub struct DownloadChapter<'a> {
    pub id_chapter: &'a str,
    pub manga_id: &'a str,
    pub manga_title: &'a str,
    pub title: &'a str,
    pub number: &'a str,
    pub scanlator: &'a str,
    pub lang: &'a str,
}

fn create_manga_directory(chapter: &DownloadChapter<'_>) -> Result<(PathBuf), std::io::Error> {
    // need directory with the manga's title, and its id to make it unique
    let chapter_id = chapter.id_chapter.to_string();

    let dir_manga_downloads = APP_DATA_DIR.as_ref().unwrap().join("mangaDownloads");

    let dir_manga = dir_manga_downloads.join(format!(
        "{} {}",
        chapter.manga_title.trim(),
        chapter.manga_id
    ));

    if !exists!(&dir_manga) {
        create_dir(&dir_manga)?;
    }

    // need directory to store the language the chapter is in
    let chapter_language_dir = dir_manga.join(chapter.lang);

    if !exists!(&chapter_language_dir) {
        create_dir(&chapter_language_dir)?;
    }

    // need directory with chapter's title, number and scanlator

    Ok(chapter_language_dir)
}

pub fn download_chapter_raw_images(
    is_downloading_all_chapters: bool,
    chapter: DownloadChapter<'_>,
    files: Vec<String>,
    endpoint: String,
    tx: UnboundedSender<MangaPageEvents>,
) -> Result<(), std::io::Error> {
    let chapter_language_dir = create_manga_directory(&chapter)?;

    let chapter_dir = chapter_language_dir.join(format!(
        "Ch. {} {} {} {}",
        chapter.number,
        chapter.title.trim().replace('/', "-"),
        chapter.scanlator.trim().replace('/', "-"),
        chapter.id_chapter,
    ));

    if !exists!(&chapter_dir) {
        create_dir(&chapter_dir)?;
    }
    let chapter_id = chapter.id_chapter.to_string();

    tokio::spawn(async move {
        let total_pages = files.len();
        for (index, file_name) in files.into_iter().enumerate() {
            let image_response = MangadexClient::global()
                .get_chapter_page(&endpoint, &file_name)
                .await;

            let file_name = Path::new(&file_name);

            match image_response {
                Ok(bytes) => {
                    let image_name = format!(
                        "{}.{}",
                        index + 1,
                        file_name.extension().unwrap().to_str().unwrap()
                    );
                    let mut image_created = File::create(chapter_dir.join(image_name)).unwrap();
                    image_created.write_all(&bytes).unwrap();

                    if !is_downloading_all_chapters {
                        tx.send(MangaPageEvents::SetDownloadProgress(
                            (index as f64) / (total_pages as f64),
                            chapter_id.clone(),
                        ))
                        .ok();
                    }
                }
                Err(e) => write_to_error_log(ErrorType::FromError(Box::new(e))),
            }
        }

        if is_downloading_all_chapters {
            tx.send(MangaPageEvents::SetDownloadAllChaptersProgress)
                .ok();
        } else {
            tx.send(MangaPageEvents::ChapterFinishedDownloading(chapter_id))
                .ok();
        }
    });

    Ok(())
}

pub fn download_chapter_cbz(
    is_downloading_all_chapters: bool,
    chapter: DownloadChapter<'_>,
    files: Vec<String>,
    endpoint: String,
    tx: UnboundedSender<MangaPageEvents>,
) -> Result<(), std::io::Error> {
    let (chapter_dir_language) = create_manga_directory(&chapter)?;

    let chapter_id = chapter.id_chapter.to_string();
    let chapter_name = format!(
        "Ch. {} {} | {} | {}",
        chapter.number,
        chapter.title.trim().replace('/', "-"),
        chapter.scanlator.trim().replace('/', "-"),
        chapter.id_chapter,
    );

    let chapter_name = format!("{}.cbz", chapter_name);

    let chapter_zip_file = File::create(chapter_dir_language.join(chapter_name))?;

    tokio::spawn(async move {
        let mut zip = ZipWriter::new(chapter_zip_file);
        let total_pages = files.len();

        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        for (index, file_name) in files.into_iter().enumerate() {
            let image_response = MangadexClient::global()
                .get_chapter_page(&endpoint, &file_name)
                .await;

            let file_name = Path::new(&file_name);

            match image_response {
                Ok(bytes) => {
                    let image_name = format!(
                        "{}.{}",
                        index + 1,
                        file_name.extension().unwrap().to_str().unwrap()
                    );

                    zip.start_file(
                        chapter_dir_language.join(image_name).to_str().unwrap(),
                        options,
                    );
                    zip.write_all(&bytes).unwrap();

                    if !is_downloading_all_chapters {
                        tx.send(MangaPageEvents::SetDownloadProgress(
                            (index as f64) / (total_pages as f64),
                            chapter_id.to_string(),
                        ))
                        .ok();
                    }
                }
                Err(e) => write_to_error_log(ErrorType::FromError(Box::new(e))),
            }
        }
        zip.finish().unwrap();

        if is_downloading_all_chapters {
            tx.send(MangaPageEvents::SetDownloadAllChaptersProgress)
                .ok();
        } else {
            tx.send(MangaPageEvents::ChapterFinishedDownloading(chapter_id))
                .ok();
        }
    });

    Ok(())
}

#[derive(Default)]
pub struct DownloadAllChapters {
    pub manga_id: String,
    pub manga_title: String,
    pub lang: Languages,
}

pub fn download_all_chapters(
    chapter_data: ChapterResponse,
    manga_details: DownloadAllChapters,
    tx: UnboundedSender<MangaPageEvents>,
) {
    let total_chapters = chapter_data.data.len();

    let download_chapter_delay = if total_chapters <= 20 {
        1
    } else if (40..100).contains(&total_chapters) {
        3
    } else if (100..200).contains(&total_chapters) {
        6
    } else {
        8
    };
    let config = MangaTuiConfig::get();

    for (index, chapter) in chapter_data.data.into_iter().enumerate() {
        let id = chapter.id.clone();
        let chapter_number = chapter.attributes.chapter.unwrap_or_default();
        let manga_id = manga_details.manga_id.clone();
        let manga_title = manga_details.manga_title.clone();
        let lang = manga_details.lang;

        let tx = tx.clone();

        let scanlator = chapter
            .relationships
            .iter()
            .find(|rel| rel.type_field == "scanlation_group")
            .map(|rel| rel.attributes.as_ref().unwrap().name.to_string());

        tokio::spawn(async move {
            let pages_response = MangadexClient::global().get_chapter_pages(&id).await;

            let chapter_title = chapter.attributes.title.unwrap_or_default();
            match pages_response {
                Ok(response) => {
                    let (files, quality) = match config.image_quality {
                        ImageQuality::Low => (response.chapter.data_saver, PageType::LowQuality),
                        ImageQuality::High => (response.chapter.data, PageType::HighQuality),
                    };

                    let endpoint = format!(
                        "{}/{}/{}",
                        response.base_url, quality, response.chapter.hash
                    );

                    let chapter_to_download = DownloadChapter {
                        id_chapter: &chapter.id,
                        manga_id: &manga_id,
                        manga_title: &manga_title,
                        title: &chapter_title,
                        number: &chapter_number,
                        scanlator: &scanlator.unwrap_or_default(),
                        lang: &lang.as_human_readable(),
                    };

                    let download_proccess = match config.download_type {
                        DownloadType::Cbz => download_chapter_cbz(
                            true,
                            chapter_to_download,
                            files,
                            endpoint,
                            tx.clone(),
                        ),
                        DownloadType::Raw => download_chapter_raw_images(
                            true,
                            chapter_to_download,
                            files,
                            endpoint,
                            tx.clone(),
                        ),
                        DownloadType::Pdf => Ok(()),
                    };

                    if let Err(e) = download_proccess {
                        let error_message = format!(
                            "Chapter: {} could not be downloaded, details: {}",
                            chapter_title, e
                        );

                        tx.send(MangaPageEvents::SetDownloadAllChaptersProgress)
                            .ok();

                        write_to_error_log(ErrorType::FromError(Box::from(error_message)));
                        return;
                    }

                    tx.send(MangaPageEvents::SaveChapterDownloadStatus(
                        chapter.id,
                        chapter_title,
                    ))
                    .ok();
                }
                Err(e) => {
                    let error_message = format!(
                        "Chapter: {} could not be downloaded, details: {}",
                        chapter_title, e
                    );

                    tx.send(MangaPageEvents::SetDownloadAllChaptersProgress)
                        .ok();
                    write_to_error_log(ErrorType::FromError(Box::from(error_message)));
                }
            }
        });
        std::thread::sleep(Duration::from_secs(download_chapter_delay));
    }
}
