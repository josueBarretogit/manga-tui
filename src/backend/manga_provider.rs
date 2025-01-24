use std::error::Error;
use std::future::Future;
use std::io::Cursor;

use bytes::Bytes;
use image::{DynamicImage, ImageReader};
use manga_tui::SearchTerm;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::Span;

use super::filter::Languages;

pub mod mangadex;

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub enum Rating {
    #[default]
    Normal,
    Moderate,
    Nsfw,
}

impl Rating {
    pub fn style(&self) -> Style {
        match self {
            Self::Moderate => Style::new().black().bg(Color::Yellow),
            Self::Normal => Style::new(),
            Self::Nsfw => Style::new().black().bg(Color::Red),
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct Genres {
    pub title: String,
    pub rating: Rating,
}

impl Genres {
    pub fn new(title: String, rating: Rating) -> Self {
        Self { title, rating }
    }
}

impl From<Genres> for Span<'_> {
    fn from(value: Genres) -> Self {
        Span::styled(format!(" {} ", value.title), value.rating.style())
    }
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct PopularManga {
    pub id: String,
    pub title: String,
    pub genres: Vec<Genres>,
    pub description: String,
    pub status: MangaStatus,
    pub cover_img_url: Option<String>,
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub struct RecentlyAddedManga {
    pub id: String,
    pub title: String,
    pub description: String,
    pub cover_img_url: Option<String>,
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub enum MangaStatus {
    #[default]
    Ongoing,
    Cancelled,
    Completed,
    Hiatus,
}

impl From<MangaStatus> for Span<'_> {
    fn from(value: MangaStatus) -> Self {
        match value {
            MangaStatus::Hiatus => Span::raw(" 🟡 hiatus"),
            MangaStatus::Ongoing => Span::raw(" 🟢 ongoing"),
            MangaStatus::Cancelled => Span::raw(" 🟠 cancelled"),
            MangaStatus::Completed => Span::raw(" 🔵 completed"),
        }
    }
}

#[derive(Default, Clone, Debug, PartialEq)]
pub struct Author {
    pub id: String,
    pub name: String,
}

#[derive(Default, Clone, Debug, PartialEq)]
pub struct Artist {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Manga {
    pub id: String,
    pub title: String,
    pub genres: Vec<Genres>,
    pub description: String,
    pub status: MangaStatus,
    pub cover_img_url: Option<String>,
    pub cover_img_url_lower_quality: Option<String>,
    pub languages: Vec<Languages>,
    pub rating: f32,
    pub artist: Artist,
    pub author: Author,
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Chapter {
    pub id: String,
    pub manga_id: String,
    pub title: String,
    pub language: Languages,
    pub chapter_number: String,
    pub volume_number: Option<String>,
    pub scanlator: Option<String>,
    pub publication_date: String,
}

#[derive(Debug, Default, PartialEq, Clone)]
pub enum ChapterOrderBy {
    Ascending,
    #[default]
    Descending,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Pagination {
    pub current_page: u32,
    pub items_per_page: u32,
    pub total_items: u32,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            current_page: 1,
            items_per_page: 16,
            total_items: 100,
        }
    }
}

impl Pagination {
    pub fn new(current_page: u32, total_chapters: u32, items_per_page: u32) -> Self {
        Self {
            current_page,
            items_per_page,
            total_items: total_chapters,
        }
    }

    pub fn go_next_page(&mut self) {
        if self.current_page * self.items_per_page < self.total_items {
            self.current_page += 1;
        }
    }

    pub fn go_previous_page(&mut self) {
        if self.current_page != 1 {
            self.current_page -= 1;
        }
    }

    pub fn get_total_pages(&self) -> u32 {
        self.total_items.div_ceil(self.items_per_page)
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct ChapterFilters {
    pub order: ChapterOrderBy,
    pub language: Languages,
}

pub trait GetRawImage {
    fn get_raw_image(&self, url: &str) -> impl Future<Output = Result<Bytes, Box<dyn Error>>> + Send;
}

pub trait DecodeBytesToImage: GetRawImage + Clone + Send + 'static + Sync {
    fn get_image(&self, cover_img_url: &str) -> impl Future<Output = Result<DynamicImage, Box<dyn Error>>> + Send {
        Box::pin(async {
            let raw_image_bytes = self.get_raw_image(cover_img_url).await?;

            let image = ImageReader::new(Cursor::new(raw_image_bytes)).with_guessed_format()?.decode()?;

            Ok(image)
        })
    }
}

pub trait SearchMangaById: Clone + Send + 'static + Sync {
    fn get_manga_by_id(&self, manga_id: &str) -> impl Future<Output = Result<Manga, Box<dyn Error>>> + Send;
}

pub trait DownloadChapter: Clone + Send + 'static + Sync {
    fn download_one_chapter<F: Fn() + Send + 'static>(
        &self,
        chapter: Chapter,
        on_page_progress: F,
    ) -> impl Future<Output = Result<(), Box<dyn Error>>> + Send;
    fn download_all_chapters(&self, chapter: Vec<Chapter>) -> impl Future<Output = Result<(), Box<dyn Error>>> + Send;
}

/// Most manga websites have a section where the top 10 mangas of the month are on display in their
/// homepage, as well as the recently added mangas
pub trait HomePageMangaProvider: DecodeBytesToImage + SearchMangaById + Clone + Send + 'static + Sync {
    fn get_popular_mangas(&self) -> impl Future<Output = Result<Vec<PopularManga>, Box<dyn Error>>> + Send;
    fn get_recently_added_mangas(&self) -> impl Future<Output = Result<Vec<RecentlyAddedManga>, Box<dyn Error>>> + Send;
}

pub trait MangaPageProvider: DecodeBytesToImage + Clone + Send + 'static + Sync {
    fn get_chapters(
        &self,
        manga_id: &str,
        filters: ChapterFilters,
        pagination: Pagination,
    ) -> impl Future<Output = Result<(Vec<Chapter>, Pagination), Box<dyn Error>>> + Send;
    //fn get_all_chapters(&self) -> impl Future<Output = Result<Vec<Chapter>, Box<dyn Error>>> + Send;
    //fn get_chapter_by_id(&self, chapter_id: &str) -> impl Future<Output = Result<Chapter, Box<dyn Error>>> + Send;
}

//pub trait SearchPageProvider: SearchMangaCover + SearchMangaById + Clone + Send + 'static + Sync {
//    fn search_mangas(&self, search_term: SearchTerm) -> impl Future<Output = Result<Vec<PopularManga>, Box<dyn Error>>> + Send;
//}

#[cfg(test)]
pub mod mock {
    //use super::{HomePageMangaProvider, PopularManga, RecentlyAddedManga};
    //
    //#[derive(Clone)]
    //pub struct MockMangaPageProvider {}
    //
    //impl MockMangaPageProvider {
    //    pub fn new() -> Self {
    //        Self {}
    //    }
    //}
    //
    //impl HomePageMangaProvider for MockMangaPageProvider {
    //    async fn get_popular_mangas(&self) -> Result<Vec<PopularManga>, Box<dyn std::error::Error>> {
    //        Ok(vec![PopularManga::default()])
    //    }
    //
    //    async fn get_recently_added_mangas(&self) -> Result<Vec<RecentlyAddedManga>, Box<dyn std::error::Error>> {
    //        Ok(vec![RecentlyAddedManga::default()])
    //    }
    //}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_goes_to_next_page() {
        let mut pagination = Pagination::new(1, 10, 5);

        pagination.go_next_page();

        assert_eq!(pagination.current_page, 2);

        pagination.go_next_page();

        assert_eq!(pagination.current_page, 2);

        let mut pagination = Pagination::new(1, 15, 5);

        pagination.go_next_page();
        pagination.go_next_page();
        pagination.go_next_page();

        assert_eq!(pagination.current_page, 3);
    }

    #[test]
    fn pagination_goes_to_previosu_page() {
        let mut pagination = Pagination::new(3, 10, 5);

        pagination.go_previous_page();

        assert_eq!(pagination.current_page, 2);

        pagination.go_previous_page();
        pagination.go_previous_page();
        pagination.go_previous_page();

        assert_eq!(pagination.current_page, 1);
    }

    #[test]
    fn pagination_calculates_amount_of_pages() {
        let pagination = Pagination::new(1, 15, 5);

        assert_eq!(3, pagination.get_total_pages());

        let pagination = Pagination::new(1, 109, 16);

        assert_eq!(7, pagination.get_total_pages())
    }
}
