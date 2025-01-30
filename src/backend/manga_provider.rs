use std::error::Error;
use std::fmt::Display;
use std::future::Future;
use std::io::Cursor;

use bytes::Bytes;
use image::{DynamicImage, GenericImageView, ImageReader};
use manga_tui::SearchTerm;
use mangadex::filter::FilterListItem;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::Span;
use reqwest::Url;
use serde::Deserialize;
use strum::{Display, EnumIter, IntoEnumIterator};

use super::database::ChapterBookmarked;
use super::tui::Events;
use crate::config::{DownloadType, ImageQuality};
use crate::global::PREFERRED_LANGUAGE;
use crate::view::pages::reader::ListOfChapters;
use crate::view::widgets::StatefulWidgetFrame;

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

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
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
#[derive(Debug, Display, EnumIter, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Languages {
    French,
    #[default]
    English,
    Spanish,
    #[strum(to_string = "Spanish (latam)")]
    SpanishLa,
    Italian,
    Japanese,
    Korean,
    #[strum(to_string = "Portuguese (brazil)")]
    BrazilianPortuguese,
    #[strum(to_string = "Portuguese")]
    Portuguese,
    #[strum(to_string = "Chinese (traditional)")]
    TraditionalChinese,
    #[strum(to_string = "Chinese (simplified)")]
    SimplifiedChinese,
    Russian,
    German,
    Burmese,
    Arabic,
    Bulgarian,
    Vietnamese,
    Croatian,
    Hungarian,
    Dutch,
    Mongolian,
    Turkish,
    Ukrainian,
    Thai,
    Catalan,
    Indonesian,
    Filipino,
    Hindi,
    Romanian,
    Hebrew,
    Polish,
    Persian,
    // Some language that is missing from this `list`
    Unkown,
}

impl From<FilterListItem> for Languages {
    fn from(value: FilterListItem) -> Self {
        Self::iter()
            .find(|lang| value.name == format!("{} {}", lang.as_emoji(), lang.as_human_readable()))
            .unwrap_or_default()
    }
}

impl Languages {
    pub fn as_emoji(self) -> &'static str {
        match self {
            Self::Mongolian => "🇲🇳",
            Self::Polish => "🇵🇱",
            Self::Persian => "🇮🇷",
            Self::Romanian => "🇷🇴",
            Self::Hungarian => "🇭🇺",
            Self::Hebrew => "🇮🇱",
            Self::Filipino => "🇵🇭",
            Self::Catalan => "",
            Self::Hindi => "🇮🇳",
            Self::Indonesian => "🇮🇩",
            Self::Thai => "🇹🇭",
            Self::Turkish => "🇹🇷",
            Self::SimplifiedChinese => "🇨🇳",
            Self::TraditionalChinese => "🇨🇳",
            Self::Italian => "🇮🇹",
            Self::Vietnamese => "🇻🇳",
            Self::English => "🇺🇸",
            Self::Dutch => "🇳🇱",
            Self::French => "🇫🇷",
            Self::Korean => "🇰🇷",
            Self::German => "🇩🇪",
            Self::Arabic => "🇸🇦",
            Self::Spanish => "🇪🇸",
            Self::Russian => "🇷🇺",
            Self::Japanese => "🇯🇵",
            Self::Burmese => "🇲🇲",
            Self::Croatian => "🇭🇷",
            Self::SpanishLa => "🇲🇽",
            Self::Bulgarian => "🇧🇬",
            Self::Ukrainian => "🇺🇦",
            Self::BrazilianPortuguese => "🇧🇷",
            Self::Portuguese => "🇵🇹",
            Self::Unkown => unreachable!(),
        }
    }

    pub fn get_preferred_lang() -> &'static Languages {
        PREFERRED_LANGUAGE.get_or_init(Self::default)
    }

    pub fn as_human_readable(self) -> String {
        self.to_string()
    }

    pub fn as_iso_code(self) -> &'static str {
        match self {
            Self::Mongolian => "mn",
            Self::Persian => "fa",
            Self::Polish => "pl",
            Self::Romanian => "ro",
            Self::Hungarian => "hu",
            Self::Hebrew => "he",
            Self::Filipino => "fi",
            Self::Catalan => "ca",
            Self::Hindi => "hi",
            Self::Indonesian => "id",
            Self::Turkish => "tr",
            Self::Spanish => "es",
            Self::French => "fr",
            Self::English => "en",
            Self::Japanese => "ja",
            Self::Dutch => "nl",
            Self::Korean => "ko",
            Self::German => "de",
            Self::Arabic => "ar",
            Self::BrazilianPortuguese => "pt-br",
            Self::Portuguese => "pt",
            Self::Russian => "ru",
            Self::Burmese => "my",
            Self::Croatian => "hr",
            Self::SpanishLa => "es-la",
            Self::Bulgarian => "bg",
            Self::Ukrainian => "uk",
            Self::Vietnamese => "vi",
            Self::TraditionalChinese => "zh-hk",
            Self::Italian => "it",
            Self::SimplifiedChinese => "zh",
            Self::Thai => "th",
            Languages::Unkown => "",
        }
    }

    pub fn try_from_iso_code(code: &str) -> Option<Self> {
        Self::iter().find(|lang| lang.as_iso_code() == code)
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
    /// Most mangas providers show the rating of the manga, if they dont then 0.0 should be used
    /// instead
    pub rating: f64,
    /// Some manga providers provide the artist of the manga
    pub artist: Option<Artist>,
    /// Some manga providers provide the author of the manga
    pub author: Option<Artist>,
}

/// Optional values exist because some manga providers dont include them when using their search
/// functionality
#[derive(Debug, Default, PartialEq, Clone)]
pub struct SearchManga {
    pub id: String,
    pub title: String,
    pub genres: Vec<Genres>,
    pub description: Option<String>,
    pub status: Option<MangaStatus>,
    pub cover_img_url: Option<String>,
    pub languages: Vec<Languages>,
    /// Some manga providers provide the artist of the manga
    pub artist: Option<Artist>,
    /// Some manga providers provide the author of the manga
    pub author: Option<Artist>,
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
pub struct LatestChapter {
    pub id: String,
    pub manga_id: String,
    pub title: String,
    pub language: Languages,
    pub chapter_number: String,
    pub volume_number: Option<String>,
    pub publication_date: String,
}

#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub enum ChapterOrderBy {
    Ascending,
    #[default]
    Descending,
}

impl ChapterOrderBy {
    pub fn toggle(self) -> Self {
        match self {
            ChapterOrderBy::Ascending => ChapterOrderBy::Descending,
            ChapterOrderBy::Descending => ChapterOrderBy::Ascending,
        }
    }
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

    pub fn can_go_next_page(&self) -> bool {
        self.current_page * self.items_per_page < self.total_items
    }

    pub fn can_go_previous_page(&self) -> bool {
        self.current_page != 1
    }

    pub fn get_total_pages(&self) -> u32 {
        self.total_items.div_ceil(self.items_per_page)
    }

    /// Used when searching for the first time
    pub fn from_total_items(total_items: u32) -> Self {
        Self {
            current_page: 1,
            items_per_page: 16,
            total_items,
        }
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct GetChaptersResponse {
    pub chapters: Vec<Chapter>,
    pub total_chapters: u32,
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct ChapterFilters {
    pub order: ChapterOrderBy,
    pub language: Languages,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ChapterToRead {
    pub id: String,
    pub title: String,
    pub number: f64,
    /// This is string because it could also be "none" for chapters with no volume associated
    pub volume_number: Option<String>,
    pub num_page_bookmarked: Option<u32>,
    pub language: Languages,
    pub pages_url: Vec<Url>,
}

impl Display for ChapterToRead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"
     id: {},
     title: {},
     number: {},
     volume: {}
     Page bookmarked: {}
     language: {},
        "#,
            self.id,
            self.title,
            self.number,
            self.volume_number.clone().unwrap_or("none".to_string()),
            self.num_page_bookmarked.unwrap_or(0),
            self.language,
        )
    }
}

impl Default for ChapterToRead {
    fn default() -> Self {
        Self {
            id: String::default(),
            number: 1.0,
            title: String::default(),
            volume_number: Some("1".to_string()),
            pages_url: vec![],
            language: Languages::default(),
            num_page_bookmarked: None,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Default)]
pub struct MangaPanel {
    pub image_decoded: DynamicImage,
    pub dimensions: (u32, u32),
}

/// Struct mainly used to download a chapter, thats why extension is needed
#[derive(Debug, PartialEq, Clone, Default)]
pub struct ChapterPage {
    pub bytes: Bytes,
    pub extension: String,
}

/// Struct mainly used to download a chapter, thats why extension is needed
#[derive(Debug, PartialEq, Clone)]
pub struct ChapterPageUrl {
    pub url: Url,
    pub extension: String,
}

pub trait GetRawImage {
    fn get_raw_image(&self, url: &str) -> impl Future<Output = Result<Bytes, Box<dyn Error>>> + Send;
}

pub trait DecodeBytesToImage: GetRawImage + Clone + Send + Sync {
    fn get_image(&self, cover_img_url: &str) -> impl Future<Output = Result<DynamicImage, Box<dyn Error>>> + Send {
        Box::pin(async {
            let raw_image_bytes = self.get_raw_image(cover_img_url).await?;

            let image = ImageReader::new(Cursor::new(raw_image_bytes)).with_guessed_format()?.decode()?;

            Ok(image)
        })
    }
}

pub trait SearchChapterById: Send + Clone {
    fn search_chapter(
        &self,
        chapter_id: &str,
        manga_id: &str,
    ) -> impl Future<Output = Result<ChapterToRead, Box<dyn Error>>> + Send;
}

pub trait FetchChapterBookmarked: Send + Clone + Sync {
    fn fetch_chapter_bookmarked(
        &self,
        chapter: ChapterBookmarked,
    ) -> impl Future<Output = Result<(ChapterToRead, ListOfChapters), Box<dyn Error>>> + Send;
}

pub trait GoToReadChapter: Send + Clone + 'static + Sync {
    fn read_chapter(
        &self,
        chapter_id: &str,
        manga_id: &str,
    ) -> impl Future<Output = Result<(ChapterToRead, ListOfChapters), Box<dyn Error>>> + Send;
}

pub trait SearchMangaPanel: DecodeBytesToImage + Send + Clone {
    fn search_manga_panel(&self, endpoint: Url) -> impl Future<Output = Result<MangaPanel, Box<dyn Error>>> + Send {
        Box::pin(async move {
            let image_decoded = self.get_image(endpoint.as_str()).await?;

            let dimensions = image_decoded.dimensions();

            Ok(MangaPanel {
                image_decoded,
                dimensions,
            })
        })
    }
}

pub trait SearchMangaById: Clone + Send + Sync {
    fn get_manga_by_id(&self, manga_id: &str) -> impl Future<Output = Result<Manga, Box<dyn Error>>> + Send;
}

pub trait GetChapterPages {
    fn get_chapter_pages_url(
        &self,
        chapter_id: &str,
        manga_id: &str,
        image_quality: ImageQuality,
    ) -> impl Future<Output = Result<Vec<Url>, Box<dyn Error>>> + Send;

    fn get_chapter_pages_url_with_extension(
        &self,
        chapter_id: &str,
        manga_id: &str,
        image_quality: ImageQuality,
    ) -> impl Future<Output = Result<Vec<ChapterPageUrl>, Box<dyn Error>>> + Send;

    /// `on_progress` is used to indicate how many pages have been fetched
    fn get_chapter_pages_with_progress<F: Fn(f64, &str) + 'static + Send>(
        &self,
        chapter_id: &str,
        manga_id: &str,
        image_quality: ImageQuality,
        on_progress: F,
    ) -> impl Future<Output = Result<Vec<ChapterPage>, Box<dyn Error>>> + Send;
}

/// Most manga websites have a section where the top 10 mangas of the month are on display in their
/// homepage, as well as the recently added mangas
pub trait HomePageMangaProvider: DecodeBytesToImage + SearchMangaById + Clone + Send + Sync + 'static {
    fn get_popular_mangas(&self) -> impl Future<Output = Result<Vec<PopularManga>, Box<dyn Error>>> + Send;
    fn get_recently_added_mangas(&self) -> impl Future<Output = Result<Vec<RecentlyAddedManga>, Box<dyn Error>>> + Send;
}

pub trait MangaPageProvider:
    DecodeBytesToImage + GoToReadChapter + GetChapterPages + FetchChapterBookmarked + Clone + Send + Sync
{
    fn get_chapters(
        &self,
        manga_id: &str,
        filters: ChapterFilters,
        pagination: Pagination,
    ) -> impl Future<Output = Result<GetChaptersResponse, Box<dyn Error>>> + Send;
    fn get_all_chapters(&self) -> impl Future<Output = Result<Vec<Chapter>, Box<dyn Error>>> + Send;
}

pub trait ReaderPageProvider: SearchMangaPanel + SearchChapterById + Send + Sync + 'static {}

pub trait EventHandler {
    fn handle_events(&mut self, events: Events);
}

pub trait FiltersHandler: EventHandler {
    type InnerState: Send + Clone;

    fn toggle(&mut self);
    fn is_open(&self) -> bool;
    fn is_typing(&self) -> bool;
    fn get_state(&self) -> &Self::InnerState;
}

pub trait FiltersWidget: StatefulWidgetFrame<State = Self::FilterState> {
    type FilterState;
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct GetMangasResponse {
    pub mangas: Vec<SearchManga>,
    pub total_mangas: u32,
}

pub trait SearchPageProvider: DecodeBytesToImage + SearchMangaById + Clone + Send + Sync + 'static {
    /// The filter state that will be used in api calls, needs to be `Send` in order to do so
    type InnerState: Send + Clone;
    /// Struct which handles the key events of the user
    type FiltersHandler: FiltersHandler<InnerState = Self::InnerState>;
    /// The widget used to show the filters to the user
    type Widget: FiltersWidget<FilterState = Self::FiltersHandler>;

    fn search_mangas(
        &self,
        search_term: Option<SearchTerm>,
        filters: Self::InnerState,
        pagination: Pagination,
    ) -> impl Future<Output = Result<GetMangasResponse, Box<dyn Error>>> + Send;
}

pub trait FeedPageProvider: SearchMangaById + Clone + Send + Sync + 'static {
    fn get_latest_chapters(&self, manga_id: &str) -> impl Future<Output = Result<Vec<LatestChapter>, Box<dyn Error>>> + Send;
}

pub trait MangaProvider:
    HomePageMangaProvider + MangaPageProvider + SearchPageProvider + ReaderPageProvider + FeedPageProvider + Send + Sync
{
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
