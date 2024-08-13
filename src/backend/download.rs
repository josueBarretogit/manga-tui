use manga_tui::exists;
use std::fs::{create_dir, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

use crate::common::PageType;
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

fn create_manga_directory(
    chapter: &DownloadChapter<'_>,
) -> Result<(PathBuf, String), std::io::Error> {
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

    let chapter_dir = chapter_language_dir.join(format!(
        "Ch. {} {} {} {}",
        chapter.number,
        chapter.title.trim().replace('/', "-"),
        chapter.scanlator.trim().replace('/', "-"),
        chapter_id
    ));

    if !exists!(&chapter_dir) {
        create_dir(&chapter_dir)?;
    }

    Ok((chapter_dir, chapter_id))
}

pub fn download_single_chaper(
    chapter: DownloadChapter<'_>,
    chapter_data: ChapterPagesResponse,
    tx: UnboundedSender<MangaPageEvents>,
) -> Result<(), std::io::Error> {
    let (chapter_dir, chapter_id) = create_manga_directory(&chapter)?;

    let total_chapters = chapter_data.chapter.data.len();

    tokio::spawn(async move {
        for (index, file_name) in chapter_data.chapter.data.iter().enumerate() {
            let endpoint = format!(
                "{}/data/{}",
                chapter_data.base_url, chapter_data.chapter.hash
            );

            let image_response = MangadexClient::global()
                .get_chapter_page(&endpoint, file_name)
                .await;

            let file_name = Path::new(&file_name);

            match image_response {
                Ok(bytes) => {
                    let mut image_created = File::create(chapter_dir.join(format!(
                        "{}.{}",
                        index + 1,
                        file_name.extension().unwrap().to_str().unwrap()
                    )))
                    .unwrap();
                    image_created.write_all(&bytes).unwrap();
                    tx.send(MangaPageEvents::SetDownloadProgress(
                        (index as f64) / (total_chapters as f64),
                        chapter_id.clone(),
                    ))
                    .ok();
                }
                Err(e) => write_to_error_log(ErrorType::FromError(Box::new(e))),
            }
        }
        tx.send(MangaPageEvents::ChapterFinishedDownloading(chapter_id))
            .ok();
    });

    Ok(())
}

pub fn download_chapter(
    chapter: DownloadChapter<'_>,
    chapter_data: ChapterPagesResponse,
    chapter_quality: PageType,
    tx: UnboundedSender<MangaPageEvents>,
) -> Result<(), std::io::Error> {
    let (chapter_dir, chapter_id) = create_manga_directory(&chapter)?;

    let total_chapters = chapter_data.chapter.data.len();

    let files = match chapter_quality {
        PageType::HighQuality => chapter_data.chapter.data,
        PageType::LowQuality => chapter_data.chapter.data_saver,
    };

    tokio::spawn(async move {
        for (index, file_name) in files.into_iter().enumerate() {
            let endpoint = format!(
                "{}/{}/{}",
                chapter_data.base_url, chapter_quality, chapter_data.chapter.hash
            );

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
                    if exists!(&chapter_dir.join(&image_name)) {
                        return;
                    }

                    let mut image_created = File::create(chapter_dir.join(image_name)).unwrap();
                    image_created.write_all(&bytes).unwrap();

                    // tx.send(MangaPageEvents::SetDownloadProgress(
                    //     (index as f64) / (total_chapters as f64),
                    //     chapter_id.clone(),
                    // ))
                    // .ok();
                }
                Err(e) => write_to_error_log(ErrorType::FromError(Box::new(e))),
            }
        }
        // tx.send(MangaPageEvents::ChapterFinishedDownloading(chapter_id))
        // .ok();
    });

    Ok(())
}

pub struct DownloadAllChapters {
    pub manga_id: String,
    pub manga_title: String,
}

pub fn download_all_chapters(
    chapter_data: ChapterResponse,
    manga_details: DownloadAllChapters,
    tx: UnboundedSender<MangaPageEvents>,
) {
    for chapter in chapter_data.data {
        let id = chapter.id.clone();
        let manga_id = manga_details.manga_id.clone();
        let manga_title = manga_details.manga_title.clone();
        let tx = tx.clone();

        let scanlator = chapter
            .relationships
            .iter()
            .find(|rel| rel.type_field == "scanlation_group")
            .map(|rel| rel.attributes.as_ref().unwrap().name.to_string());

        tokio::spawn(async move {
            let pages_response = MangadexClient::global().get_chapter_pages(&id).await;

            match pages_response {
                Ok(res) => {
                    download_chapter(
                        DownloadChapter {
                            id_chapter: &chapter.id,
                            manga_id: &manga_id,
                            manga_title: &manga_title,
                            title: chapter.attributes.title.unwrap_or_default().as_str(),
                            number: chapter.attributes.chapter.unwrap_or_default().as_str(),
                            scanlator: &scanlator.unwrap_or_default(),
                            lang: &Languages::default().as_human_readable(),
                        },
                        res,
                        PageType::LowQuality,
                        tx,
                    )
                    .unwrap();
                }
                Err(e) => {
                    write_to_error_log(ErrorType::FromError(Box::new(e)));
                }
            }
        });
        std::thread::sleep(Duration::from_secs(3));
    }
}
