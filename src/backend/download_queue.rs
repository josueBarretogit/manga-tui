use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::backend::AppDirectories;
use crate::backend::database::{Database, SetChapterDownloaded};
use crate::backend::error_log::write_to_error_log;
use crate::backend::manga_downloader::cbz_downloader::CbzDownloader;
use crate::backend::manga_downloader::epub_downloader::EpubDownloader;
use crate::backend::manga_downloader::pdf_downloader::PdfDownloader;
use crate::backend::manga_downloader::raw_images::RawImagesDownloader;
use crate::backend::manga_downloader::{ChapterToDownloadSanitized, MangaDownloader};
use crate::backend::manga_provider::{Chapter, Languages, MangaProvider, MangaProviders};
use crate::backend::tui::Events;
use crate::config::{DownloadType, MangaTuiConfig};

#[derive(Clone, Debug, PartialEq)]
pub struct EnqueueChapterItem {
    pub manga_id: String,
    pub manga_id_safe_for_download: String,
    pub manga_title: String,
    pub manga_cover_url: Option<String>,
    pub lang: Languages,
    pub chapter: Chapter,
    pub provider: MangaProviders,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnqueueDownloadBatch {
    pub manga_id: String,
    pub manga_id_safe_for_download: String,
    pub manga_title: String,
    pub manga_cover_url: Option<String>,
    pub lang: Languages,
    pub chapters: Vec<Chapter>,
    pub provider: MangaProviders,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct DownloadQueueProgress {
    pub is_downloading: bool,
    pub current_manga_title: String,
    pub current_chapter_number: String,
    pub current_page_percentage: f64,
    pub completed_in_batch: usize,
    pub total_in_batch: usize,
    pub remaining_items: usize,
}

#[derive(Debug)]
pub enum DownloadQueueCommand {
    Enqueue(EnqueueDownloadBatch),
    CancelAll,
}

pub struct DownloadQueueWorker<T: MangaProvider> {
    client: Arc<T>,
    global_event_tx: UnboundedSender<Events>,
    command_rx: UnboundedReceiver<DownloadQueueCommand>,
    queue: VecDeque<EnqueueChapterItem>,
    completed_in_batch: usize,
    total_in_batch: usize,
}

impl<T: MangaProvider + 'static> DownloadQueueWorker<T> {
    pub fn new(
        client: Arc<T>,
        global_event_tx: UnboundedSender<Events>,
        command_rx: UnboundedReceiver<DownloadQueueCommand>,
    ) -> Self {
        Self {
            client,
            global_event_tx,
            command_rx,
            queue: VecDeque::new(),
            completed_in_batch: 0,
            total_in_batch: 0,
        }
    }

    pub async fn run(mut self) {
        loop {
            // Drain any pending commands before starting next download
            while let Ok(cmd) = self.command_rx.try_recv() {
                match cmd {
                    DownloadQueueCommand::Enqueue(batch) => {
                        let count = batch.chapters.len();
                        for chapter in batch.chapters {
                            self.queue.push_back(EnqueueChapterItem {
                                manga_id: batch.manga_id.clone(),
                                manga_id_safe_for_download: batch.manga_id_safe_for_download.clone(),
                                manga_title: batch.manga_title.clone(),
                                manga_cover_url: batch.manga_cover_url.clone(),
                                lang: batch.lang,
                                chapter,
                                provider: batch.provider,
                            });
                        }
                        self.total_in_batch += count;
                        self.send_progress(false, "", "", 0.0);
                    },
                    DownloadQueueCommand::CancelAll => {
                        self.queue.clear();
                        self.completed_in_batch = 0;
                        self.total_in_batch = 0;
                        self.send_progress(false, "", "", 0.0);
                    },
                }
            }

            if let Some(item) = self.queue.pop_front() {
                self.send_progress(true, &item.manga_title, &item.chapter.chapter_number, 0.0);

                let config = MangaTuiConfig::get().clone();
                let event_tx_clone = self.global_event_tx.clone();
                let manga_title_clone = item.manga_title.clone();
                let chapter_num_clone = item.chapter.chapter_number.clone();
                let completed = self.completed_in_batch;
                let total = self.total_in_batch;
                let remaining = self.queue.len();

                let pages_result = self
                    .client
                    .get_chapter_pages(&item.chapter.id, &item.manga_id, config.image_quality, move |percent, _| {
                        event_tx_clone
                            .send(Events::DownloadQueueProgress(DownloadQueueProgress {
                                is_downloading: true,
                                current_manga_title: manga_title_clone.clone(),
                                current_chapter_number: chapter_num_clone.clone(),
                                current_page_percentage: percent,
                                completed_in_batch: completed,
                                total_in_batch: total,
                                remaining_items: remaining + 1,
                            }))
                            .ok();
                    })
                    .await
                    .map_err(|e| e.to_string());

                match pages_result {
                    Ok(pages) => {
                        let original_chapter_title = item.chapter.title.clone();
                        let chapter_id = item.chapter.id.clone();
                        let chapter_to_download = ChapterToDownloadSanitized {
                            chapter_id: item.chapter.id_safe_for_download,
                            manga_id: item.manga_id_safe_for_download,
                            manga_title: item.manga_title.clone().into(),
                            chapter_title: item.chapter.title.clone().into(),
                            chapter_number: item.chapter.chapter_number.clone(),
                            volume_number: item.chapter.volume_number,
                            language: item.lang,
                            scanlator: item.chapter.scanlator.unwrap_or_default().into(),
                            download_type: config.download_type,
                            pages,
                            provider: Some(item.provider),
                        };

                        let downloader: &dyn MangaDownloader = match config.download_type {
                            DownloadType::Cbz => &CbzDownloader::new(),
                            DownloadType::Raw => &RawImagesDownloader::new(),
                            DownloadType::Epub => &EpubDownloader::new(),
                            DownloadType::Pdf => &PdfDownloader::new(),
                        };

                        let download_res = downloader
                            .save_chapter_in_file_system(&AppDirectories::MangaDownloads.get_full_path(), chapter_to_download);

                        if let Err(e) = download_res {
                            write_to_error_log(format!("Failed to download chapter: {original_chapter_title}, error: {e}").into());
                        } else if let Ok(conn) = Database::get_connection() {
                            let database = Database::new(&conn);
                            let _ = database.set_chapter_downloaded(SetChapterDownloaded {
                                id: &chapter_id,
                                title: &original_chapter_title,
                                manga_id: &item.manga_id,
                                manga_title: &item.manga_title,
                                img_url: item.manga_cover_url.as_deref(),
                                provider: item.provider,
                            });
                        }

                        self.completed_in_batch += 1;
                        self.global_event_tx
                            .send(Events::DownloadQueueChapterFinished {
                                manga_id: item.manga_id,
                                chapter_id,
                            })
                            .ok();
                    },
                    Err(e) => {
                        write_to_error_log(format!("Failed to fetch pages for chapter: {}, error: {e}", item.chapter.title).into());
                        self.completed_in_batch += 1;
                    },
                }

                // Rate limiting delay between chapters (2 seconds)
                tokio::time::sleep(Duration::from_secs(2)).await;

                if self.queue.is_empty() {
                    let total = self.completed_in_batch;
                    self.completed_in_batch = 0;
                    self.total_in_batch = 0;
                    self.send_progress(false, "", "", 0.0);
                    self.global_event_tx.send(Events::DownloadQueueFinished(total)).ok();
                } else {
                    self.send_progress(false, "", "", 0.0);
                }
            } else {
                // Queue is empty: wait for next command
                match self.command_rx.recv().await {
                    Some(DownloadQueueCommand::Enqueue(batch)) => {
                        let count = batch.chapters.len();
                        for chapter in batch.chapters {
                            self.queue.push_back(EnqueueChapterItem {
                                manga_id: batch.manga_id.clone(),
                                manga_id_safe_for_download: batch.manga_id_safe_for_download.clone(),
                                manga_title: batch.manga_title.clone(),
                                manga_cover_url: batch.manga_cover_url.clone(),
                                lang: batch.lang,
                                chapter,
                                provider: batch.provider,
                            });
                        }
                        self.total_in_batch += count;
                        self.send_progress(false, "", "", 0.0);
                    },
                    Some(DownloadQueueCommand::CancelAll) => {
                        self.queue.clear();
                        self.completed_in_batch = 0;
                        self.total_in_batch = 0;
                        self.send_progress(false, "", "", 0.0);
                    },
                    None => {
                        // Channel sender dropped, terminate worker
                        break;
                    },
                }
            }
        }
    }

    fn send_progress(&self, is_downloading: bool, title: &str, ch_num: &str, percent: f64) {
        self.global_event_tx
            .send(Events::DownloadQueueProgress(DownloadQueueProgress {
                is_downloading,
                current_manga_title: title.to_string(),
                current_chapter_number: ch_num.to_string(),
                current_page_percentage: percent,
                completed_in_batch: self.completed_in_batch,
                total_in_batch: self.total_in_batch,
                remaining_items: self.queue.len(),
            }))
            .ok();
    }
}
