use std::fs::{create_dir, create_dir_all, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use bytes::Bytes;
use manga_tui::{exists, SanitizedFilename};
use tokio::sync::mpsc::UnboundedSender;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use super::error_log::{write_to_error_log, ErrorType};
use super::fetch::{ApiClient, MangadexClient, MockMangadexClient};
use super::AppDirectories;
use crate::view::pages::manga::MangaPageEvents;

pub struct DownloadChapter {
    pub id_chapter: SanitizedFilename,
    pub manga_id: SanitizedFilename,
    pub manga_title: SanitizedFilename,
    pub chapter_title: SanitizedFilename,
    pub number: u32,
    pub scanlator: SanitizedFilename,
    pub lang: SanitizedFilename,
}

impl<'a> DownloadChapter {
    pub fn new(
        id_chapter: &'a str,
        manga_id: &'a str,
        manga_title: &'a str,
        chapter_title: &'a str,
        number: u32,
        scanlator: &'a str,
        lang: &'a str,
    ) -> Self {
        Self {
            id_chapter: SanitizedFilename::new(id_chapter),
            manga_id: SanitizedFilename::new(manga_id),
            manga_title: SanitizedFilename::new(manga_title),
            chapter_title: SanitizedFilename::new(chapter_title),
            number,
            scanlator: SanitizedFilename::new(scanlator),
            lang: SanitizedFilename::new(lang),
        }
    }

    pub fn make_chapter_name(&'a self) -> PathBuf {
        let file_name = format!("Ch. {} {} {} {}", self.number, self.chapter_title, self.scanlator, self.id_chapter);
        PathBuf::from(file_name)
    }
}

fn create_manga_directory(chapter: &DownloadChapter, base_directory: &Path) -> Result<PathBuf, std::io::Error> {
    let dir_manga_downloads = base_directory.join("mangaDownloads");

    if !exists!(&dir_manga_downloads) {
        create_dir_all(&dir_manga_downloads)?;
    }

    let dir_manga = dir_manga_downloads.join(format!("{} {}", chapter.manga_title, chapter.manga_id));

    if !exists!(&dir_manga) {
        create_dir(&dir_manga)?;
    }

    // need directory to store the language the chapter is in
    let chapter_language_dir = dir_manga.join(chapter.lang.as_path());

    if !exists!(&chapter_language_dir) {
        create_dir(&chapter_language_dir)?;
    }

    Ok(chapter_language_dir)
}

async fn fetch_page(client: impl ApiClient, endpoint: String, filename: String) -> Result<Bytes, reqwest::Error> {
    let response = client.get_chapter_page(&endpoint, &filename).await?;
    response.bytes().await
}

pub fn download_chapter_raw_images(
    is_downloading_all_chapters: bool,
    chapter: DownloadChapter,
    files: Vec<String>,
    endpoint: String,
    tx: UnboundedSender<MangaPageEvents>,
) -> Result<(), std::io::Error> {
    let chapter_language_dir = create_manga_directory(&chapter, AppDirectories::get_base_directory())?;

    let chapter_dir = chapter_language_dir
        .join(format!("Ch. {} {} {} {}", chapter.number, chapter.chapter_title, chapter.scanlator, chapter.id_chapter,));

    if !exists!(&chapter_dir) {
        create_dir(&chapter_dir)?;
    }
    let chapter_id = chapter.id_chapter.to_string();

    tokio::spawn(async move {
        let total_pages = files.len();
        for (index, file_name) in files.into_iter().enumerate() {
            let image_response = MockMangadexClient::new().get_chapter_page(&endpoint, &file_name).await;

            let file_name = Path::new(&file_name);

            match image_response {
                Ok(response) => {
                    if let Ok(bytes) = response.bytes().await {
                        let image_name = format!("{}.{}", index + 1, file_name.extension().unwrap().to_str().unwrap());
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
                },
                Err(e) => write_to_error_log(ErrorType::FromError(Box::new(e))),
            }
        }

        if is_downloading_all_chapters {
            tx.send(MangaPageEvents::SetDownloadAllChaptersProgress).ok();
        } else {
            tx.send(MangaPageEvents::ChapterFinishedDownloading(chapter_id)).ok();
        }
    });

    Ok(())
}

pub fn download_chapter_epub(
    is_downloading_all_chapters: bool,
    chapter: DownloadChapter,
    files: Vec<String>,
    endpoint: String,
    tx: UnboundedSender<MangaPageEvents>,
) -> Result<(), std::io::Error> {
    let chapter_dir_language = create_manga_directory(&chapter, AppDirectories::get_base_directory())?;

    let chapter_id = chapter.id_chapter.to_string();
    let chapter_name = format!("Ch. {} {} {} {}", chapter.number, chapter.chapter_title, chapter.scanlator, chapter.id_chapter);

    tokio::spawn(async move {
        let total_pages = files.len();

        let mut epub_output = File::create(chapter_dir_language.join(format!("{}.epub", chapter_name))).unwrap();

        let mut epub = epub_builder::EpubBuilder::new(epub_builder::ZipLibrary::new().unwrap()).unwrap();

        epub.epub_version(epub_builder::EpubVersion::V30);

        let _ = epub.metadata("title", chapter_name);

        for (index, file_name) in files.into_iter().enumerate() {
            let image_response = MangadexClient::global().get_chapter_page(&endpoint, &file_name).await;

            match image_response {
                Ok(response) => {
                    if let Ok(bytes) = response.bytes().await {
                        let image_path = format!("data/{}", file_name);

                        let file_name = Path::new(&file_name);

                        let mime_type = format!("image/{}", file_name.extension().unwrap().to_str().unwrap());

                        if index == 0 {
                            epub.add_cover_image(&image_path, bytes.as_ref(), &mime_type).unwrap();
                        }

                        epub.add_resource(&image_path, bytes.as_ref(), &mime_type).unwrap();

                        epub.add_content(epub_builder::EpubContent::new(
                            format!("{}.xhtml", index + 1),
                            format!(
                                r#" 
                            <?xml version='1.0' encoding='utf-8'?>
                            <!DOCTYPE html>
                            <html xmlns="http://www.w3.org/1999/xhtml">
                              <head>
                                <title>Panel</title>
                                <meta http-equiv="Content-Type" content="text/html; charset=utf-8"/>
                              </head>
                              <body>
                                <div class="centered_image">
                                    <img src="{}" alt="Panel" />
                                </div>
                              </body>
                            </html>
                        "#,
                                image_path
                            )
                            .as_bytes(),
                        ))
                        .unwrap();

                        if !is_downloading_all_chapters {
                            tx.send(MangaPageEvents::SetDownloadProgress(
                                (index as f64) / (total_pages as f64),
                                chapter_id.to_string(),
                            ))
                            .ok();
                        }
                    }
                },
                Err(e) => write_to_error_log(ErrorType::FromError(Box::new(e))),
            }
        }

        epub.generate(&mut epub_output).unwrap();

        if is_downloading_all_chapters {
            tx.send(MangaPageEvents::SetDownloadAllChaptersProgress).ok();
        } else {
            tx.send(MangaPageEvents::ChapterFinishedDownloading(chapter_id)).ok();
        }
    });

    Ok(())
}

pub fn download_chapter_cbz(
    is_downloading_all_chapters: bool,
    chapter: DownloadChapter,
    files: Vec<String>,
    endpoint: String,
    tx: UnboundedSender<MangaPageEvents>,
) -> Result<(), std::io::Error> {
    let chapter_dir_language = create_manga_directory(&chapter, AppDirectories::get_base_directory())?;

    let chapter_id = chapter.id_chapter.to_string();
    let chapter_name = format!("Ch. {} {} {} {}", chapter.number, chapter.chapter_title, chapter.scanlator, chapter.id_chapter);

    let chapter_name = format!("{}.cbz", chapter_name);

    let chapter_zip_file = File::create(chapter_dir_language.join(chapter_name))?;

    tokio::spawn(async move {
        let mut zip = ZipWriter::new(chapter_zip_file);
        let total_pages = files.len();

        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        for (index, file_name) in files.into_iter().enumerate() {
            let image_response = MangadexClient::global().get_chapter_page(&endpoint, &file_name).await;

            let file_name = Path::new(&file_name);

            match image_response {
                Ok(response) => {
                    if let Ok(bytes) = response.bytes().await {
                        let image_name = format!("{}.{}", index + 1, file_name.extension().unwrap().to_str().unwrap());

                        let _ = zip.start_file(chapter_dir_language.join(image_name).to_str().unwrap(), options);

                        let _ = zip.write_all(&bytes);

                        if !is_downloading_all_chapters {
                            tx.send(MangaPageEvents::SetDownloadProgress(
                                (index as f64) / (total_pages as f64),
                                chapter_id.to_string(),
                            ))
                            .ok();
                        }
                    }
                },
                Err(e) => write_to_error_log(ErrorType::FromError(Box::new(e))),
            }
        }
        zip.finish().unwrap();

        if is_downloading_all_chapters {
            tx.send(MangaPageEvents::SetDownloadAllChaptersProgress).ok();
        } else {
            tx.send(MangaPageEvents::ChapterFinishedDownloading(chapter_id)).ok();
        }
    });

    Ok(())
}

// fetch pages
// make directory
// create file with the page
// notify back that proccess is succesful

#[cfg(test)]
mod tests {
    use std::fs;

    use fake::*;
    use pretty_assertions::assert_eq;
    use uuid::Uuid;

    use self::faker::name::en::{FirstName, Title};
    use super::*;
    use crate::backend::filter::Languages;

    #[test]
    fn it_should_make_a_directory_for_a_manga() -> Result<(), std::io::Error> {
        let chapter_to_download = DownloadChapter::new(
            &Uuid::new_v4().to_string(),
            &Uuid::new_v4().to_string(),
            Title().fake(),
            Title().fake(),
            1,
            FirstName().fake(),
            &Languages::default().as_human_readable(),
        );

        let directory_path = create_manga_directory(&chapter_to_download, Path::new("./"))?;

        fs::read_dir(&directory_path)?;

        Ok(())
    }

    //#[tokio::test]
    //async fn download_as_cbz() {
    //    let id = Uuid::new_v4().to_string();
    //
    //    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<MangaPageEvents>();
    //
    //    let chapter_to_download = DownloadChapter {
    //        id_chapter: &id,
    //        manga_id: "some_manga_id",
    //        manga_title: "some_title",
    //        chapter_title: "some_title",
    //        number: "1",
    //        scanlator: "some_scanlator",
    //        lang: Languages::default().as_iso_code(),
    //    };
    //
    //    let resullt = download_chapter_raw_images(
    //        false,
    //        chapter_to_download,
    //        vec!["file1.png".to_string(), "file2.png".to_string()],
    //        "some_endpoint".to_string(),
    //        tx,
    //    );
    //
    //    if let Err(e) = resullt {
    //        panic!("{e}");
    //    }
    //}
}
