use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tokio::sync::mpsc::UnboundedSender;

use crate::backend::api_responses::{ChapterPagesResponse, ChapterResponse};
use crate::backend::download::{DownloadChapter, ImageMetada};
use crate::backend::error_log::{self, write_to_error_log, ErrorType};
#[cfg(test)]
use crate::backend::fetch::MockMangadexClient;
use crate::backend::fetch::{ApiClient, MangadexClient};
use crate::backend::filter::Languages;
use crate::common::PageType;
use crate::config::{DownloadType, ImageQuality, MangaTuiConfig};
use crate::view::pages::manga::{ChapterOrder, MangaPageEvents};

pub async fn search_chapters_operation(
    manga_id: String,
    page: u32,
    language: Languages,
    chapter_order: ChapterOrder,
    tx: UnboundedSender<MangaPageEvents>,
) {
    #[cfg(test)]
    let api_client = MockMangadexClient::new();

    #[cfg(not(test))]
    let api_client = MangadexClient::global();

    let response = api_client.get_manga_chapters(&manga_id, page, language, chapter_order).await;

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

//pub fn download_chapter_raw_images(
//    is_downloading_all_chapters: bool,
//    chapter: DownloadChapter,
//    files: Vec<String>,
//    endpoint: String,
//    tx: UnboundedSender<MangaPageEvents>,
//) -> Result<(), std::io::Error> {
//    let chapter_language_dir = chapter.create_manga_directory(AppDirectories::get_base_directory())?;
//
//    let chapter_dir = chapter_language_dir
//        .join(format!("Ch. {} {} {} {}", chapter.number, chapter.chapter_title, chapter.scanlator, chapter.id_chapter,));
//
//    if !exists!(&chapter_dir) {
//        create_dir(&chapter_dir)?;
//    }
//    let chapter_id = chapter.id_chapter.to_string();
//
//    tokio::spawn(async move {
//        let total_pages = files.len();
//        for (index, file_name) in files.into_iter().enumerate() {
//            let image_response = MangadexClient::global().get_chapter_page(&endpoint, &file_name).await;
//
//            let file_name = Path::new(&file_name);
//
//            match image_response {
//                Ok(response) => {
//                    if let Ok(bytes) = response.bytes().await {
//                        let image_name = format!("{}.{}", index + 1, file_name.extension().unwrap().to_str().unwrap());
//                        let mut image_created = File::create(chapter_dir.join(image_name)).unwrap();
//                        image_created.write_all(&bytes).unwrap();
//
//                        if !is_downloading_all_chapters {
//                            tx.send(MangaPageEvents::SetDownloadProgress(
//                                (index as f64) / (total_pages as f64),
//                                chapter_id.clone(),
//                            ))
//                            .ok();
//                        }
//                    }
//                },
//                Err(e) => write_to_error_log(ErrorType::FromError(Box::new(e))),
//            }
//        }
//
//        if is_downloading_all_chapters {
//            tx.send(MangaPageEvents::SetDownloadAllChaptersProgress).ok();
//        } else {
//            tx.send(MangaPageEvents::ChapterFinishedDownloading(chapter_id)).ok();
//        }
//    });
//
//    Ok(())
//}
//
//pub fn download_chapter_epub(
//    is_downloading_all_chapters: bool,
//    chapter: DownloadChapter,
//    files: Vec<String>,
//    endpoint: String,
//    tx: UnboundedSender<MangaPageEvents>,
//) -> Result<(), std::io::Error> {
//    let chapter_dir_language = chapter.create_manga_directory(AppDirectories::get_base_directory())?;
//
//    let chapter_id = chapter.id_chapter.to_string();
//    let chapter_name = format!("Ch. {} {} {} {}", chapter.number, chapter.chapter_title, chapter.scanlator, chapter.id_chapter);
//
//    tokio::spawn(async move {
//        let total_pages = files.len();
//
//        let mut epub_output = File::create(chapter_dir_language.join(format!("{}.epub", chapter_name))).unwrap();
//
//        let mut epub = epub_builder::EpubBuilder::new(epub_builder::ZipLibrary::new().unwrap()).unwrap();
//
//        epub.epub_version(epub_builder::EpubVersion::V30);
//
//        let _ = epub.metadata("title", chapter_name);
//
//        for (index, file_name) in files.into_iter().enumerate() {
//            let image_response = MangadexClient::global().get_chapter_page(&endpoint, &file_name).await;
//
//            match image_response {
//                Ok(response) => {
//                    if let Ok(bytes) = response.bytes().await {
//                        let image_path = format!("data/{}", file_name);
//
//                        let file_name = Path::new(&file_name);
//
//                        let mime_type = format!("image/{}", file_name.extension().unwrap().to_str().unwrap());
//
//                        if index == 0 {
//                            epub.add_cover_image(&image_path, bytes.as_ref(), &mime_type).unwrap();
//                        }
//
//                        epub.add_resource(&image_path, bytes.as_ref(), &mime_type).unwrap();
//
//                        epub.add_content(epub_builder::EpubContent::new(
//                            format!("{}.xhtml", index + 1),
//                            format!(
//                                r#"
//                            <?xml version='1.0' encoding='utf-8'?>
//                            <!DOCTYPE html>
//                            <html xmlns="http://www.w3.org/1999/xhtml">
//                              <head>
//                                <title>Panel</title>
//                                <meta http-equiv="Content-Type" content="text/html; charset=utf-8"/>
//                              </head>
//                              <body>
//                                <div class="centered_image">
//                                    <img src="{}" alt="Panel" />
//                                </div>
//                              </body>
//                            </html>
//                        "#,
//                                image_path
//                            )
//                            .as_bytes(),
//                        ))
//                        .unwrap();
//
//                        if !is_downloading_all_chapters {
//                            tx.send(MangaPageEvents::SetDownloadProgress(
//                                (index as f64) / (total_pages as f64),
//                                chapter_id.to_string(),
//                            ))
//                            .ok();
//                        }
//                    }
//                },
//                Err(e) => write_to_error_log(ErrorType::FromError(Box::new(e))),
//            }
//        }
//
//        epub.generate(&mut epub_output).unwrap();
//
//        if is_downloading_all_chapters {
//            tx.send(MangaPageEvents::SetDownloadAllChaptersProgress).ok();
//        } else {
//            tx.send(MangaPageEvents::ChapterFinishedDownloading(chapter_id)).ok();
//        }
//    });
//
//    Ok(())
//}
//
//pub fn download_chapter_cbz(
//    is_downloading_all_chapters: bool,
//    chapter: DownloadChapter,
//    files: Vec<String>,
//    endpoint: String,
//    tx: UnboundedSender<MangaPageEvents>,
//) -> Result<(), std::io::Error> {
//    let chapter_dir_language = create_manga_directory(&chapter, AppDirectories::get_base_directory())?;
//
//    let chapter_id = chapter.id_chapter.to_string();
//    let chapter_name = format!("Ch. {} {} {} {}", chapter.number, chapter.chapter_title, chapter.scanlator, chapter.id_chapter);
//
//    let chapter_name = format!("{}.cbz", chapter_name);
//
//    let chapter_zip_file = File::create(chapter_dir_language.join(chapter_name))?;
//
//    tokio::spawn(async move {
//        let mut zip = ZipWriter::new(chapter_zip_file);
//        let total_pages = files.len();
//
//        let options = SimpleFileOptions::default()
//            .compression_method(zip::CompressionMethod::Deflated)
//            .unix_permissions(0o755);
//
//        for (index, file_name) in files.into_iter().enumerate() {
//            let image_response = MangadexClient::global().get_chapter_page(&endpoint, &file_name).await;
//
//            let file_name = Path::new(&file_name);
//
//            match image_response {
//                Ok(response) => {
//                    if let Ok(bytes) = response.bytes().await {
//                        let image_name = format!("{}.{}", index + 1, file_name.extension().unwrap().to_str().unwrap());
//
//                        let _ = zip.start_file(chapter_dir_language.join(image_name).to_str().unwrap(), options);
//
//                        let _ = zip.write_all(&bytes);
//
//                        if !is_downloading_all_chapters {
//                            tx.send(MangaPageEvents::SetDownloadProgress(
//                                (index as f64) / (total_pages as f64),
//                                chapter_id.to_string(),
//                            ))
//                            .ok();
//                        }
//                    }
//                },
//                Err(e) => write_to_error_log(ErrorType::FromError(Box::new(e))),
//            }
//        }
//        zip.finish().unwrap();
//
//        if is_downloading_all_chapters {
//            tx.send(MangaPageEvents::SetDownloadAllChaptersProgress).ok();
//        } else {
//            tx.send(MangaPageEvents::ChapterFinishedDownloading(chapter_id)).ok();
//        }
//    });
//
//    Ok(())
//}
//

//async fn get_chapter_pages_response(
//    api_client: impl ApiClient,
//    id_manga: String,
//    file_format: DownloadType,
//) -> Result<(), Box<dyn std::error::Error>> {
//    let response = api_client.get_chapter_pages(&id_manga).await?.json().await;
//    Ok(())
//}

struct DownloadArgs<'a> {
    chapter_to_download: DownloadChapter,
    files: Vec<String>,
    directory_to_download: &'a Path,
    endpoint: &'a str,
}

impl<'a> DownloadArgs<'a> {
    fn new(chapter_to_download: DownloadChapter, files: Vec<String>, directory_to_download: &'a Path, endpoint: &'a str) -> Self {
        Self {
            chapter_to_download,
            files,
            directory_to_download,
            endpoint,
        }
    }
}

async fn download_chapter_raw_images(api_client: impl ApiClient, data: DownloadArgs<'_>) -> Result<PathBuf, Box<dyn Error>> {
    let manga_directory = data.chapter_to_download.create_manga_directory(data.directory_to_download)?;
    let chapter_directory = data.chapter_to_download.make_raw_images_directory(&manga_directory)?;

    for (index, chapter_page_file_name) in data.files.into_iter().enumerate() {
        let extension = Path::new(&chapter_page_file_name).extension().unwrap().to_str().unwrap();

        let image_bytes = api_client.get_chapter_page(data.endpoint, &chapter_page_file_name).await?.bytes().await?;

        data.chapter_to_download.create_image_file(
            &image_bytes,
            &chapter_directory,
            format!("{}.{}", index + 1, extension).into(),
        )?;
    }

    Ok(chapter_directory)
}

async fn collect_chapter_pages(
    api_client: impl ApiClient,
    files: Vec<String>,
    image_endpoint: &str,
) -> Result<Vec<ImageMetada>, Box<dyn Error>> {
    let mut images: Vec<ImageMetada> = vec![];

    for file_name in files {
        let extension = Path::new(&file_name).extension().unwrap().to_str().unwrap();
        let image_bytes = api_client.get_chapter_page(image_endpoint, &file_name).await?.bytes().await?;
        images.push(ImageMetada::new(extension, image_bytes));
    }

    Ok(images)
}

async fn download_chapter_cbz(api_client: impl ApiClient, data: DownloadArgs<'_>) -> Result<PathBuf, Box<dyn Error>> {
    let pages_to_save_in_cbz = collect_chapter_pages(api_client, data.files, data.endpoint).await?;
    let cbz_created = data.chapter_to_download.create_cbz(data.directory_to_download, pages_to_save_in_cbz)?;

    Ok(cbz_created)
}

async fn download_chapter_epub(api_client: impl ApiClient, data: DownloadArgs<'_>) -> Result<PathBuf, Box<dyn Error>> {
    let pages_to_save_in_cbz = collect_chapter_pages(api_client, data.files, data.endpoint).await?;
    let epub_created = data.chapter_to_download.create_epub(data.directory_to_download, pages_to_save_in_cbz)?;

    Ok(epub_created)
}

async fn download_chapter(
    chapter_to_download: DownloadChapter,
    api_client: impl ApiClient,
    image_quality: ImageQuality,
    directory_to_download: PathBuf,
    file_format: DownloadType,
) -> Result<PathBuf, Box<dyn Error>> {
    let pages_response: ChapterPagesResponse =
        api_client.get_chapter_pages(&chapter_to_download.get_chapter_id()).await?.json().await?;

    let image_endpoint = pages_response.get_image_url_endpoint(image_quality);

    let files = pages_response.get_files_based_on_quality(image_quality);

    let file_created = match file_format {
        DownloadType::Cbz => {
            download_chapter_cbz(api_client, DownloadArgs::new(chapter_to_download, files, &directory_to_download, &image_endpoint))
                .await?
        },
        DownloadType::Raw => {
            download_chapter_raw_images(
                api_client,
                DownloadArgs::new(chapter_to_download, files, &directory_to_download, &image_endpoint),
            )
            .await?
        },
        DownloadType::Epub => {
            download_chapter_epub(
                api_client,
                DownloadArgs::new(chapter_to_download, files, &directory_to_download, &image_endpoint),
            )
            .await?
        },
    };

    Ok(file_created)
}

async fn download_multiple_chapters() {}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use fake::faker::name::en::Name;
    use fake::Fake;
    use manga_tui::exists;
    use pretty_assertions::assert_eq;
    use uuid::Uuid;

    use super::*;
    use crate::backend::fetch::MockMangadexClient;

    fn create_tests_directory() -> Result<PathBuf, std::io::Error> {
        let base_directory = Path::new("./test_results/manga_page_tasks");

        if !exists!(&base_directory) {
            fs::create_dir_all(base_directory)?;
        }

        Ok(base_directory.to_path_buf())
    }

    fn get_chapter_for_testing() -> DownloadChapter {
        DownloadChapter::new(
            &Uuid::new_v4().to_string(),
            &Uuid::new_v4().to_string(),
            &Name().fake::<String>(),
            &Name().fake::<String>(),
            1,
            &Name().fake::<String>(),
            &Languages::default().as_human_readable(),
        )
    }

    #[tokio::test]
    async fn download_a_chapter_given_a_api_response_raw_images() -> Result<(), Box<dyn std::error::Error>> {
        let chapter_to_download = get_chapter_for_testing();
        let directory_to_download = create_tests_directory()?;

        let chapter_directory_path = download_chapter(
            chapter_to_download.clone(),
            MockMangadexClient::new(),
            ImageQuality::Low,
            directory_to_download.clone(),
            DownloadType::Raw,
        )
        .await?;

        let chapter_directory = fs::read_dir(&chapter_directory_path)?;

        for file in chapter_directory {
            dbg!(file?);
        }

        Ok(())
    }

    #[tokio::test]
    async fn download_a_chapter_given_a_api_response_cbz() -> Result<(), Box<dyn std::error::Error>> {
        let cbz_created = download_chapter(
            get_chapter_for_testing(),
            MockMangadexClient::new(),
            ImageQuality::Low,
            create_tests_directory()?,
            DownloadType::Cbz,
        )
        .await?;

        assert_eq!(cbz_created.extension().unwrap(), "cbz");

        Ok(())
    }

    #[tokio::test]
    async fn download_a_chapter_given_a_api_response_epub() -> Result<(), Box<dyn std::error::Error>> {
        let epub_created = download_chapter(
            get_chapter_for_testing(),
            MockMangadexClient::new(),
            ImageQuality::Low,
            create_tests_directory()?,
            DownloadType::Epub,
        )
        .await?;

        assert_eq!(epub_created.extension().unwrap(), "epub");

        Ok(())
    }
}
