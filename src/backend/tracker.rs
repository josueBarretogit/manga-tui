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
