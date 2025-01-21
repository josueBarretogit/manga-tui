use std::error::Error;
use std::future::Future;

use image::DynamicImage;
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

pub trait SearchMangaCover {
    fn get_manga_cover(&self, cover_img_url: &str) -> impl Future<Output = Result<DynamicImage, Box<dyn Error>>> + Send;
    /// Some manga providers may have a way of getting the cover with a lower resolution, which
    /// reduces memory comsumption
    fn get_manga_cover_lower_quality(
        &self,
        cover_img_url: &str,
    ) -> impl Future<Output = Result<DynamicImage, Box<dyn Error>>> + Send;
}

/// Most manga websites have a section where the top 10 mangas of the month are on display in their
/// homepage, as well as the recently added mangas
pub trait HomePageMangaProvider: SearchMangaCover + Clone + Send + 'static {
    fn get_popular_mangas(&self) -> impl Future<Output = Result<Vec<PopularManga>, Box<dyn Error>>> + Send;
    fn get_recently_added_mangas(&self) -> impl Future<Output = Result<Vec<RecentlyAddedManga>, Box<dyn Error>>> + Send;
}

pub trait MangaPageProvider: SearchMangaCover + Clone + Send + 'static {
    fn get_manga_by_id(&self, manga_id: &str) -> impl Future<Output = Result<Manga, Box<dyn Error>>> + Send;
}

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
