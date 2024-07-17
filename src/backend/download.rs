use manga_tui::exists;
use std::fs::{create_dir, File};
use std::io::Write;
use std::path::Path;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

use crate::view::pages::manga::MangaPageEvents;

use super::error_log::{write_to_error_log, ErrorType};
use super::fetch::MangadexClient;
use super::{ChapterPagesResponse, APP_DATA_DIR};

pub struct DownloadChapter<'a> {
    pub id_chapter: &'a str,
    pub manga_id: &'a str,
    pub manga_title: &'a str,
    pub title: &'a str,
    pub number: &'a str,
    pub scanlator: &'a str,
}

pub fn download_chapter(
    chapter: DownloadChapter<'_>,
    chapter_data: ChapterPagesResponse,
    tx: UnboundedSender<MangaPageEvents>,
) -> Result<(), std::io::Error> {
    // need directory with the manga's title, and its id to make it unique
    let chapter_id = chapter.id_chapter.to_string();

    let dir_manga_downloads = APP_DATA_DIR.as_ref().unwrap().join("mangaDownloads");

    let dir_manga =
        dir_manga_downloads.join(format!("{} {}", chapter.manga_title, chapter.manga_id));

    if !exists!(&dir_manga) {
        create_dir(&dir_manga)?;
    }

    // need directory with chapter's title, number and scanlator

    let chapter_dir = dir_manga.join(format!(
        "Ch. {} {} {} {}",
        chapter.number, chapter.title, chapter.scanlator, chapter_id
    ));

    if !exists!(&chapter_dir) {
        create_dir(&chapter_dir)?;
    }

    // create images and store them in the directory

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
                }
                Err(e) => write_to_error_log(ErrorType::FromError(Box::new(e))),
            }
        }
        tx.send(MangaPageEvents::ChapterFinishedDownloading(chapter_id))
            .ok();
    });

    Ok(())
}
