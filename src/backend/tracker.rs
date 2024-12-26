use std::error::Error;

use futures::Future;
use manga_tui::SearchTerm;
use serde::{Deserialize, Serialize};

pub mod anilist;

#[derive(Debug, Deserialize, Serialize, Default, PartialEq, Eq)]
pub struct MangaToTrack {
    pub id: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct MarkAsRead<'a> {
    pub id: &'a str,
    pub chapter_number: u32,
    pub volume_number: Option<u32>,
}

pub trait MangaTracker: Send + Clone + 'static {
    fn search_manga_by_title(
        &self,
        title: SearchTerm,
    ) -> impl Future<Output = Result<Option<MangaToTrack>, Box<dyn std::error::Error>>> + Send;

    /// Implementors may require api key / account token in order to perform this operation
    fn mark_manga_as_read_with_chapter_count(
        &self,
        manga: MarkAsRead<'_>,
    ) -> impl Future<Output = Result<(), Box<dyn Error>>> + Send;
}

pub async fn update_reading_progress(
    manga_title: SearchTerm,
    chapter_number: u32,
    volume_number: Option<u32>,
    tracker: impl MangaTracker,
) -> Result<(), Box<dyn Error>> {
    let response = tracker.search_manga_by_title(manga_title).await?;
    if let Some(manga) = response {
        tracker
            .mark_manga_as_read_with_chapter_count(MarkAsRead {
                id: &manga.id,
                chapter_number,
                volume_number,
            })
            .await?;
    }
    Ok(())
}

pub fn track_manga<T, F>(tracker: Option<T>, manga_title: String, chapter_number: u32, volume_number: Option<u32>, on_error: F)
where
    T: MangaTracker,
    F: Fn(String) + Send + 'static,
{
    if let Some(tracker) = tracker {
        tokio::spawn(async move {
            let title = SearchTerm::trimmed(&manga_title);
            if let Some(search_term) = title {
                let response = update_reading_progress(search_term, chapter_number, volume_number, tracker).await;
                if let Err(e) = response {
                    on_error(e.to_string());
                }
            }
        });
    }
}
