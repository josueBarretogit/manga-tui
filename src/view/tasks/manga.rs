use std::error::Error;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, Instant};

use reqwest::Url;
use tokio::sync::mpsc::UnboundedSender;

use crate::backend::api_responses::{AggregateChapterResponse, ChapterPagesResponse, ChapterResponse};
use crate::backend::database::{save_history, ChapterToSaveHistory, Database, MangaReadingHistorySave};
use crate::backend::download::DownloadChapter;
use crate::backend::error_log::{write_to_error_log, ErrorType};
#[cfg(test)]
use crate::backend::fetch::fake_api_client::MockMangadexClient;
use crate::backend::fetch::ApiClient;
#[cfg(not(test))]
use crate::backend::fetch::MangadexClient;
use crate::backend::filter::Languages;
use crate::config::{DownloadType, ImageQuality, MangaTuiConfig};
use crate::view::app::MangaToRead;
use crate::view::pages::manga::{ChapterOrder, MangaPageEvents};
use crate::view::pages::reader::{ChapterToRead, ListOfChapters};

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
            write_to_error_log(ErrorType::Error(Box::new(e)));
            tx.send(MangaPageEvents::LoadChapters(None)).ok();
        },
    }
}

pub struct DownloadArgs<'a> {
    chapter_to_download: DownloadChapter,
    files: Vec<String>,
    directory_to_download: &'a Path,
    endpoint: &'a str,
    should_report_progress: bool,
    sender_report_download_progress: UnboundedSender<MangaPageEvents>,
}

impl<'a> DownloadArgs<'a> {
    fn new(
        chapter_to_download: DownloadChapter,
        files: Vec<String>,
        directory_to_download: &'a Path,
        endpoint: &'a str,
        should_report_progress: bool,
        sender_report_download_progress: UnboundedSender<MangaPageEvents>,
    ) -> Self {
        Self {
            chapter_to_download,
            files,
            directory_to_download,
            endpoint,
            should_report_progress,
            sender_report_download_progress,
        }
    }
}

async fn download_chapter_raw_images(
    api_client: impl ApiClient,
    chapter_id: String,
    data: DownloadArgs<'_>,
) -> Result<PathBuf, Box<dyn Error>> {
    let chapter_directory = data.chapter_to_download.make_chapter_directory(data.directory_to_download)?;
    let total_pages = data.files.len();

    for (index, chapter_page_file_name) in data.files.into_iter().enumerate() {
        let extension = Path::new(&chapter_page_file_name).extension().unwrap().to_str().unwrap();

        let endpoint: Url = format!("{}/{}", data.endpoint, chapter_page_file_name)
            .parse()
            .unwrap_or("http://localhost".parse().unwrap());

        if let Ok(response) = api_client.get_chapter_page(endpoint).await {
            if let Ok(bytes) = response.bytes().await {
                data.chapter_to_download.create_image_file(
                    &bytes,
                    &chapter_directory,
                    format!("{}.{}", index + 1, extension).into(),
                )?;
            }
        }
        if data.should_report_progress {
            data.sender_report_download_progress
                .send(MangaPageEvents::SetDownloadProgress(index as f64 / total_pages as f64, chapter_id.clone()))
                .ok();
        }
    }

    Ok(chapter_directory)
}

async fn download_chapter_cbz(
    api_client: impl ApiClient,
    chapter_id: String,
    data: DownloadArgs<'_>,
) -> Result<PathBuf, Box<dyn Error>> {
    let (mut zip_writer, cbz_path) = data.chapter_to_download.create_cbz_file(data.directory_to_download)?;
    let total_pages = data.files.len();

    for (index, file_name) in data.files.into_iter().enumerate() {
        let extension = Path::new(&file_name).extension().unwrap().to_str().unwrap();

        let endpoint: Url = format!("{}/{}", data.endpoint, file_name)
            .parse()
            .unwrap_or("http://localhost".parse().unwrap());

        if let Ok(response) = api_client.get_chapter_page(endpoint).await {
            if let Ok(bytes) = response.bytes().await {
                let file_name = format!("{}.{}", index + 1, extension);
                data.chapter_to_download.insert_into_cbz(&mut zip_writer, &file_name, &bytes);
            }
        }

        if data.should_report_progress {
            data.sender_report_download_progress
                .send(MangaPageEvents::SetDownloadProgress(index as f64 / total_pages as f64, chapter_id.clone()))
                .ok();
        }
    }

    zip_writer.finish()?;

    Ok(cbz_path)
}

async fn download_chapter_epub(
    api_client: impl ApiClient,
    chapter_id: String,
    data: DownloadArgs<'_>,
) -> Result<PathBuf, Box<dyn Error>> {
    let (mut epub_builder, mut epub_file, epub_path) = data.chapter_to_download.create_epub_file(data.directory_to_download)?;
    let total_pages = data.files.len();

    for (index, file_name) in data.files.into_iter().enumerate() {
        let extension = Path::new(&file_name).extension().unwrap().to_str().unwrap();

        let endpoint: Url = format!("{}/{}", data.endpoint, file_name)
            .parse()
            .unwrap_or("http://localhost".parse().unwrap());

        if let Ok(response) = api_client.get_chapter_page(endpoint).await {
            if let Ok(bytes) = response.bytes().await {
                let file_name = format!("{}.{}", index + 1, extension);
                data.chapter_to_download
                    .insert_into_epub(&mut epub_builder, &file_name, extension, index, &bytes);
            }
        }

        if data.should_report_progress {
            data.sender_report_download_progress
                .send(MangaPageEvents::SetDownloadProgress(index as f64 / total_pages as f64, chapter_id.clone()))
                .ok();
        }
    }

    epub_builder.generate(&mut epub_file)?;

    Ok(epub_path)
}

#[allow(clippy::too_many_arguments)]
pub async fn download_chapter_task(
    chapter_to_download: DownloadChapter,
    api_client: impl ApiClient,
    image_quality: ImageQuality,
    directory_to_download: PathBuf,
    file_format: DownloadType,
    chapter_id: String,
    should_report_progress: bool,
    sender: UnboundedSender<MangaPageEvents>,
) -> Result<PathBuf, Box<dyn Error>> {
    let manga_base_directory = chapter_to_download.make_base_manga_directory(&directory_to_download)?;

    let pages_response: ChapterPagesResponse = api_client.get_chapter_pages(&chapter_id).await?.json().await?;

    let image_endpoint = pages_response.get_image_url_endpoint(image_quality);

    let files = pages_response.get_files_based_on_quality(image_quality);

    let file_created = match file_format {
        DownloadType::Cbz => {
            download_chapter_cbz(
                api_client,
                chapter_id,
                DownloadArgs::new(
                    chapter_to_download,
                    files,
                    &manga_base_directory,
                    &image_endpoint,
                    should_report_progress,
                    sender,
                ),
            )
            .await?
        },
        DownloadType::Raw => {
            download_chapter_raw_images(
                api_client,
                chapter_id,
                DownloadArgs::new(
                    chapter_to_download,
                    files,
                    &manga_base_directory,
                    &image_endpoint,
                    should_report_progress,
                    sender,
                ),
            )
            .await?
        },
        DownloadType::Epub => {
            download_chapter_epub(
                api_client,
                chapter_id,
                DownloadArgs::new(
                    chapter_to_download,
                    files,
                    &manga_base_directory,
                    &image_endpoint,
                    should_report_progress,
                    sender,
                ),
            )
            .await?
        },
    };

    Ok(file_created)
}

#[derive(Debug, Clone)]
pub struct DownloadAllChapters {
    pub sender: UnboundedSender<MangaPageEvents>,
    pub manga_id: String,
    pub manga_title: String,
    pub image_quality: ImageQuality,
    pub directory_to_download: PathBuf,
    pub file_format: DownloadType,
    pub language: Languages,
}

pub async fn download_all_chapters(
    api_client: impl ApiClient + 'static,
    download_data: DownloadAllChapters,
) -> Result<(), Box<dyn Error>> {
    let all_chapters_response: ChapterResponse = api_client
        .get_all_chapters_for_manga(&download_data.manga_id, download_data.language)
        .await?
        .json()
        .await?;

    let total_chapters = all_chapters_response.data.len();

    download_data
        .sender
        .send(MangaPageEvents::StartDownloadProgress(total_chapters as f64))
        .ok();

    let download_chapter_delay = if total_chapters < 40 {
        1
    } else if (40..100).contains(&total_chapters) {
        3
    } else if (100..200).contains(&total_chapters) {
        6
    } else {
        8
    };

    for chapter in all_chapters_response.data {
        let scanlator = chapter
            .relationships
            .iter()
            .find(|rel| rel.type_field == "scanlation_group")
            .map(|rel| rel.attributes.as_ref().unwrap().name.to_string());

        let chapter_title = chapter.attributes.title.unwrap_or_default();
        let scanlator = scanlator.unwrap_or_default();

        let chapter_to_download = DownloadChapter::new(
            &chapter.id,
            &download_data.manga_id,
            &download_data.manga_title,
            &chapter_title,
            &chapter.attributes.chapter.unwrap_or_default(),
            &scanlator,
            &download_data.language.as_human_readable(),
        );

        let start_fetch_time = Instant::now();
        let api_client = api_client.clone();

        let download_data = download_data.clone();

        tokio::spawn(async move {
            let download_proccess = download_chapter_task(
                chapter_to_download,
                api_client,
                download_data.image_quality,
                download_data.directory_to_download.to_path_buf(),
                download_data.file_format,
                chapter.id.clone(),
                false,
                download_data.sender.clone(),
            )
            .await;

            if let Err(e) = download_proccess {
                write_to_error_log(ErrorType::Error(e));
            }

            download_data.sender.send(MangaPageEvents::SetDownloadAllChaptersProgress).ok();

            download_data
                .sender
                .send(MangaPageEvents::SaveChapterDownloadStatus(chapter.id, chapter_title))
                .ok();
        });

        let time_since = start_fetch_time.elapsed();
        sleep(Duration::from_secs(download_chapter_delay).saturating_sub(time_since));
    }

    Ok(())
}

pub struct ChapterArgs {
    pub id_chapter: String,
    pub manga_id: String,
    pub title: String,
    pub chapter_title: String,
    pub language: Languages,
    pub number: f64,
    pub volume_number: Option<String>,
    pub img_url: Option<String>,
}

/// These function looks very similar  to the implementation `impl FetchChapterBookmarked for MangadexClient` but where it is called
/// provides with data that reduce one api call
pub async fn read_chapter(chapter: &ChapterArgs) -> Result<(ChapterToRead, MangaToRead), Box<dyn std::error::Error>> {
    use crate::backend::fetch::MangadexClient;

    let chapter_response: ChapterPagesResponse =
        MangadexClient::global().get_chapter_pages(&chapter.id_chapter).await?.json().await?;

    let aggregate_res: AggregateChapterResponse = MangadexClient::global()
        .search_chapters_aggregate(&chapter.manga_id, chapter.language)
        .await?
        .json()
        .await?;

    let connection = Database::get_connection()?;
    save_history(
        MangaReadingHistorySave {
            id: &chapter.manga_id,
            title: &chapter.title,
            img_url: chapter.img_url.as_deref(),
            chapter: ChapterToSaveHistory {
                id: &chapter.id_chapter,
                title: &chapter.chapter_title,
                translated_language: chapter.language.as_iso_code(),
            },
        },
        &connection,
    )?;

    let config = MangaTuiConfig::get();

    let chapter_to_read: ChapterToRead = ChapterToRead {
        id: chapter.id_chapter.clone(),
        title: chapter.chapter_title.clone(),
        number: chapter.number,
        volume_number: chapter.volume_number.clone(),
        language: chapter.language,
        num_page_bookmarked: None,
        pages_url: chapter_response.get_files_based_on_quality_as_url(config.image_quality),
    };

    let manga_to_read: MangaToRead = MangaToRead {
        title: chapter.title.clone(),
        manga_id: chapter.manga_id.clone(),
        list: ListOfChapters::from(aggregate_res),
    };

    Ok((chapter_to_read, manga_to_read))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::ops::AddAssign;
    use std::path::{Path, PathBuf};

    use fake::faker::name::en::Name;
    use fake::Fake;
    use manga_tui::exists;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
    use uuid::Uuid;

    use super::*;
    use crate::backend::api_responses::{ChapterAttribute, ChapterData};
    use crate::backend::fetch::fake_api_client::MockMangadexClient;

    async fn validate_progress_sent(
        mut rx: UnboundedReceiver<MangaPageEvents>,
        expected_amount_files: f64,
        expected_id_sent: String,
    ) {
        let mut iterations = 0.0;
        for _ in 0..(expected_amount_files as usize) {
            let event = rx.recv().await.expect("no event was sent");
            match event {
                MangaPageEvents::SetDownloadProgress(ratio_progress, manga_id) => {
                    assert_eq!(manga_id, expected_id_sent);
                    assert_eq!(iterations / expected_amount_files, ratio_progress);
                    iterations.add_assign(1.0);
                },
                _ => panic!("wrong event was sent"),
            }
        }
    }

    async fn validate_download_all_chapter_progress(mut rx: UnboundedReceiver<MangaPageEvents>, total_chapters: f64) {
        for _ in 0..(total_chapters as usize) {
            let event = rx.recv().await.expect("no event was sent");
            match event {
                MangaPageEvents::SetDownloadAllChaptersProgress => {},
                MangaPageEvents::SaveChapterDownloadStatus(_, _) => {},
                _ => panic!("wrong event was sent"),
            }
        }
    }

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
            "1",
            &Name().fake::<String>(),
            &Languages::default().as_human_readable(),
        )
    }

    #[tokio::test]
    #[ignore]
    async fn download_a_chapter_given_a_api_response_raw_images_reporting_pages_progress() -> Result<(), Box<dyn Error>> {
        let chapter_to_download = get_chapter_for_testing();
        let directory_to_download = create_tests_directory()?;

        let (sender_progress, receiver_progress) = unbounded_channel::<MangaPageEvents>();
        let expected_amount_files = 3;
        let chapter_id = Uuid::new_v4().to_string();
        let report_progress = true;

        download_chapter_task(
            chapter_to_download.clone(),
            MockMangadexClient::new().with_amount_returning_items(expected_amount_files),
            ImageQuality::Low,
            directory_to_download.clone(),
            DownloadType::Raw,
            chapter_id.clone(),
            report_progress,
            sender_progress,
        )
        .await?;

        validate_progress_sent(receiver_progress, expected_amount_files as f64, chapter_id).await;

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn download_a_chapter_given_a_api_response_cbz() -> Result<(), Box<dyn std::error::Error>> {
        let (sender_progress, receiver_progress) = unbounded_channel::<MangaPageEvents>();
        let expected_amount_files = 3;

        let chapter_id = Uuid::new_v4().to_string();
        let report_progress = true;

        download_chapter_task(
            get_chapter_for_testing(),
            MockMangadexClient::new().with_amount_returning_items(expected_amount_files),
            ImageQuality::Low,
            create_tests_directory()?,
            DownloadType::Cbz,
            chapter_id.clone(),
            report_progress,
            sender_progress,
        )
        .await?;

        validate_progress_sent(receiver_progress, expected_amount_files as f64, chapter_id).await;

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn download_a_chapter_given_a_api_response_epub_with_progress() -> Result<(), Box<dyn std::error::Error>> {
        let (sender_progress, receiver_progress) = unbounded_channel::<MangaPageEvents>();

        let expected_amount_files = 3;
        let chapter_id = Uuid::new_v4().to_string();
        let should_report_progress = true;

        download_chapter_task(
            get_chapter_for_testing(),
            MockMangadexClient::new().with_amount_returning_items(expected_amount_files),
            ImageQuality::Low,
            create_tests_directory()?,
            DownloadType::Epub,
            chapter_id.clone(),
            should_report_progress,
            sender_progress,
        )
        .await?;

        validate_progress_sent(receiver_progress, expected_amount_files as f64, chapter_id).await;

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn download_all_chapters_expected_events() -> Result<(), Box<dyn std::error::Error>> {
        let directory_to_download = create_tests_directory()?;
        let (sender, mut rx) = unbounded_channel::<MangaPageEvents>();
        let total_chapters = 3;

        let mut chapters: Vec<ChapterData> = vec![];
        for index in 0..total_chapters {
            chapters.push(ChapterData {
                id: Uuid::new_v4().into(),
                type_field: "chapter".into(),
                attributes: ChapterAttribute {
                    chapter: Some(index.to_string()),
                    ..Default::default()
                },
                ..Default::default()
            })
        }

        let response = ChapterResponse {
            data: chapters,

            ..Default::default()
        };

        let api_client = MockMangadexClient::new().with_amount_returning_items(2).with_chapter_response(response);

        let manga_id = Uuid::new_v4().to_string();
        let manga_title = Uuid::new_v4().to_string();
        let language = Languages::default();
        let file_format = DownloadType::Cbz;
        let image_quality = ImageQuality::Low;

        download_all_chapters(api_client, DownloadAllChapters {
            sender,
            manga_id,
            manga_title,
            image_quality,
            directory_to_download: directory_to_download.clone(),
            file_format,
            language,
        })
        .await?;

        let expected_event = rx.recv().await.expect("no event was sent");

        assert_eq!(MangaPageEvents::StartDownloadProgress(total_chapters as f64), expected_event);

        validate_download_all_chapter_progress(rx, total_chapters as f64).await;

        Ok(())
    }
}
