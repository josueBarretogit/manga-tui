//! # MangaPill Provider Module
//!
//! This module provides an implementation of the [`MangaProvider`] trait for the MangaPill website
//! (`https://mangapill.com`). It handles fetching manga data, chapters, images, and search results
//! from the MangaPill website.
//!
//! ## Overview
//!
//! The `MangaPillProvider` struct implements various traits that enable it to:
//! - Fetch manga information by ID
//! - Search for mangas with filters
//! - Retrieve chapter lists and individual chapters
//! - Get chapter page images
//! - Fetch popular and recently added mangas from the home page
//! - Get latest chapters for feed functionality
//!
//! ## Key Features
//!
//! - **Caching**: All requests are cached with different durations based on content type
//! - **HTML Parsing**: Uses scrapers to parse HTML responses from the website
//! - **Browser-like Headers**: Mimics browser behavior to avoid Cloudflare blocking
//! - **Error Handling**: Comprehensive error types for different failure scenarios
//!
//! ## Important Notes
//!
//! - MangaPill only provides English language content
//! - Volume information and chapter titles are not available
//! - Image quality settings do not apply (only one quality available)
//! - The `Referer` header is required for chapter page requests to bypass Cloudflare
//!
//! ## Module Structure
//!
//! - `filter_state`: Defines filter state and handler for search functionality
//! - `filter_widget`: UI widget for displaying and interacting with filters
//! - `response`: Contains parsers and scrapers for extracting data from HTML responses
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use http::header::{ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, CACHE_CONTROL, CONNECTION, HOST, REFERER};
use http::{HeaderMap, HeaderValue, StatusCode};
use manga_tui::SearchTerm;
use reqwest::{Client, Url};
use response::{LatestMangas, PopularMangasMangaPill};

use super::{
    Chapter, ChapterFilters, ChapterOrderBy, ChapterPageUrl, DecodeBytesToImage, FeedPageProvider, FetchChapterBookmarked,
    GetChapterPages, GetChaptersResponse, GetMangasResponse, GetRawImage, GoToReadChapter, HomePageMangaProvider, Languages,
    LatestChapter, ListOfChapters, Manga, MangaPageProvider, MangaProvider, MangaProviders, Pagination, PopularManga,
    ProviderIdentity, ReaderPageProvider, RecentlyAddedManga, SearchChapterById, SearchMangaById, SearchMangaPanel,
    SearchPageProvider,
};
use crate::backend::cache::{CacheDuration, Cacher, InsertEntry};
use crate::backend::html_parser::HtmlElement;
use crate::backend::html_parser::scraper::Scraper;
use crate::backend::manga_provider::mangapill::filter_state::{MangaPillFilterState, MangaPillFiltersProvider};
use crate::backend::manga_provider::mangapill::filter_widget::MangaPillFilterWidget;
use crate::backend::manga_provider::mangapill::response::{
    ChapterPageDataError, ChapterPageDataParser, ChapterPagesScraper, ChaptersError, LatestMangaError, MangaPageDataParser,
    MangaPageError, MangaPillChaptersParser, PopularMangaParseError, SearchPageError, SearchPageMangasParser,
};
use crate::backend::manga_provider::{ChapterToRead, Genres, SearchManga};
use crate::global::get_random_user_agent;

pub mod filter_state;
pub mod filter_widget;
mod response;

/// Base URL for the MangaPill website
static MANGA_PILL_URL: &str = "https://mangapill.com";

/// MangaPill provider implementation for fetching manga data from `https://mangapill.com`
///
/// This struct implements the [`MangaProvider`] trait and provides functionality to interact
/// with the MangaPill website. It handles HTTP requests, HTML parsing, caching, and data
/// transformation.
///
/// ## Important Limitations
///
/// - **Volume Information**: MangaPill does not provide volume numbers for chapters
/// - **Chapter Titles**: Chapter titles are not available from the API
/// - **Language Support**: Only English language content is available
/// - **Image Quality**: Image quality settings are ignored as only one quality is available
/// - **URL Format**:
///   - Manga pages: `https://mangapill.com/manga/{id}/{slug}`
///   - Chapter pages: `https://mangapill.com/chapters/{id}-{chapter_id}/{slug}`
///
/// ## Browser Headers
///
/// The provider uses browser-like headers including a random user agent to avoid detection.
/// The `Referer` header (`https://mangapill.com/`) is required for chapter page requests
/// to bypass Cloudflare protection.
///
/// ## Caching Strategy
///
/// Different cache durations are used based on content type:
/// - Chapter pages: Long duration (rarely change)
/// - Manga pages: Very long duration (static content)
/// - Home page: Short duration (updates frequently)
/// - Search results: Very short duration (highly dynamic)
///
/// ## Fields
///
/// - `client`: HTTP client configured with browser-like headers and cookie support
/// - `base_url`: Base URL for MangaPill website
/// - `cache_provider`: Cache implementation for storing responses
#[derive(Clone, Debug)]
pub struct MangaPillProvider {
    /// HTTP client for making requests to MangaPill
    client: Client,
    /// Base URL of the MangaPill website
    base_url: Url,
    /// Cache provider for storing and retrieving cached responses
    cache_provider: Arc<dyn Cacher>,
}

impl MangaPillProvider {
    /// Cache duration for chapter pages (long-lived, rarely change)
    const CHAPTER_PAGE_CACHE_DURATION: CacheDuration = CacheDuration::Long;
    /// Cache duration for home page content (short-lived, updates frequently)
    const HOME_PAGE_CACHE_DURATION: CacheDuration = CacheDuration::Short;
    /// Cache duration for manga detail pages (very long-lived, static content)
    const MANGA_PAGE_CACHE_DURATION: CacheDuration = CacheDuration::LongLong;
    /// Cache duration for search results (very short-lived, highly dynamic)
    /// The search page cache is the shortest because it may change a lot
    const SEARCH_PAGE_CACHE_DURATION: CacheDuration = CacheDuration::VeryShort;

    /// Creates a new `MangaPillProvider` instance
    ///
    /// # Arguments
    ///
    /// * `base_url` - The base URL for the MangaPill website
    /// * `cache_provider` - A cache implementation for storing HTTP responses
    ///
    /// # Returns
    ///
    /// A new `MangaPillProvider` instance with:
    /// - HTTP client configured with browser-like headers
    /// - Cookie store enabled for session management
    /// - 30-second timeout for requests
    /// - Random user agent to avoid detection
    ///
    /// # Headers Configured
    ///
    /// The client is configured with headers that mimic a real browser:
    /// - `Referer`: Set to MangaPill base URL (required for Cloudflare bypass)
    /// - `Host`: mangapill.com
    /// - `Accept`: HTML and JSON content types
    /// - `Accept-Encoding`: gzip, deflate
    /// - `Accept-Language`: en-US, en
    /// - `Connection`: keep-alive
    /// - `DNT`: 1 (Do Not Track)
    /// - Various `sec-fetch-*` headers for browser compatibility
    pub fn new(base_url: Url, cache_provider: Arc<dyn Cacher>) -> Self {
        let mut default_headers = HeaderMap::new();

        default_headers.insert(REFERER, HeaderValue::from_static(MANGA_PILL_URL));
        default_headers.insert(HOST, HeaderValue::from_static("mangapill.com"));
        default_headers.insert(ACCEPT, HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"));

        default_headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate"));
        default_headers.insert(CACHE_CONTROL, HeaderValue::from_static("max-age=604800"));
        default_headers.insert(
            ACCEPT,
            HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8,application/json"),
        );

        default_headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.5"));
        default_headers.insert(CONNECTION, HeaderValue::from_static("keep-alive"));

        default_headers.insert("DNT", HeaderValue::from_static("1"));
        default_headers.insert("sec-fetch-site", HeaderValue::from_static("none"));
        default_headers.insert("sec-fetch-mode", HeaderValue::from_static("navigate"));
        default_headers.insert("sec-fetch-user", HeaderValue::from_static("?1"));
        default_headers.insert("sec-fetch-dest", HeaderValue::from_static("document"));

        let client = Client::builder()
            .cookie_store(true)
            .timeout(Duration::from_secs(30))
            .default_headers(default_headers)
            .user_agent(get_random_user_agent())
            .build()
            .unwrap();

        Self {
            client,
            base_url,
            cache_provider,
        }
    }

    /// Maps HTML document to a `ChapterToRead` structure
    ///
    /// Parses the HTML content of a chapter page and extracts chapter information
    /// including the manga title, chapter number, and page URLs.
    ///
    /// # Arguments
    ///
    /// * `doc` - HTML content of the chapter page
    /// * `chapter_id` - The chapter identifier
    ///
    /// # Returns
    ///
    /// A `ChapterToRead` struct containing:
    /// - Chapter ID
    /// - Manga title (used as chapter title since chapter titles aren't available)
    /// - Chapter number
    /// - Page URLs for all images in the chapter
    ///
    /// # Errors
    ///
    /// Returns `ChapterPageDataError` if the HTML cannot be parsed or required
    /// data is missing.
    fn map_chapter_to_read(&self, doc: String, chapter_id: &str) -> Result<ChapterToRead, ChapterPageDataError> {
        let html_parser = Scraper::new(HtmlElement::new(doc));
        let parser = ChapterPageDataParser::new(html_parser);

        let chapter = parser.scrape_page()?;

        let chapter: ChapterToRead = ChapterToRead {
            id: chapter_id.to_string(),
            title: chapter.manga_title,
            number: chapter.number,
            volume_number: None,
            num_page_bookmarked: None,
            language: Languages::default(),
            pages_url: chapter.pages_url,
        };
        Ok(chapter)
    }

    /// Maps HTML document to a `Manga` structure
    ///
    /// Parses the HTML content of a manga detail page and extracts comprehensive
    /// manga information including title, description, status, genres, and cover image.
    ///
    /// # Arguments
    ///
    /// * `doc` - HTML content of the manga page
    /// * `manga_id` - The manga identifier (e.g., "manga/580/blue-lock")
    ///
    /// # Returns
    ///
    /// A `Manga` struct containing:
    /// - Full manga ID and safe ID for downloads
    /// - Title and description
    /// - Publication status (Ongoing, Completed, etc.)
    /// - Genres/tags
    /// - Cover image URL
    /// - Language (always English for MangaPill)
    ///
    /// # Errors
    ///
    /// Returns `MangaPageError` if the HTML cannot be parsed, required data is missing,
    /// or the manga ID format is invalid.
    fn map_manga(&self, doc: String, manga_id: &str) -> Result<Manga, MangaPageError> {
        let html_parser = Scraper::new(HtmlElement::new(doc));
        let parser = MangaPageDataParser::new(html_parser);

        let manga = parser.scrape_manga_page()?;

        let id_safe_for_download = manga_id
            .split("/")
            .nth(1)
            .ok_or(format!("Could not extract id for manga: {}", manga_id))?
            .to_string();

        let manga = Manga {
            id: manga_id.to_string(),
            title: manga.title,
            description: manga.description.unwrap_or("No description".to_string()),
            status: manga.status.into(),
            cover_img_url: manga.cover_url,
            languages: vec![Languages::English],
            genres: manga.tags.into_iter().map(Genres::from).collect(),
            id_safe_for_download,
            ..Default::default()
        };

        Ok(manga)
    }

    /// Maps HTML document to a list of popular mangas
    ///
    /// Parses the home page HTML to extract the list of popular mangas displayed
    /// on the front page.
    ///
    /// # Arguments
    ///
    /// * `doc` - HTML content of the home page
    ///
    /// # Returns
    ///
    /// A vector of `PopularManga` structs, each containing:
    /// - Manga ID and page URL
    /// - Title
    /// - Cover image URL
    /// - Description (latest chapter number if available)
    ///
    /// # Errors
    ///
    /// Returns `PopularMangaParseError` if the HTML cannot be parsed or the
    /// popular mangas section is missing.
    fn map_popular_mangas(&self, doc: String) -> Result<Vec<PopularManga>, PopularMangaParseError> {
        let html_parser = Scraper::new(HtmlElement::new(doc));
        let parser = PopularMangasMangaPill::new(html_parser);

        let mangas: Vec<PopularManga> = parser
            .scrape_popular_mangas()?
            .into_iter()
            .map(|ma| {
                let manga: PopularManga = PopularManga {
                    id: ma.page_url,
                    title: ma.title,
                    genres: vec![],
                    description: ma.latest_chapter.map(|la| format!("Latest chapter: {la}")).unwrap_or("".to_string()),
                    status: None,
                    cover_img_url: ma.cover_url,
                };
                manga
            })
            .collect();

        Ok(mangas)
    }

    /// Maps HTML document to a list of recently added mangas
    ///
    /// Parses the home page HTML to extract the list of recently added/updated mangas
    /// displayed on the front page.
    ///
    /// # Arguments
    ///
    /// * `doc` - HTML content of the home page
    ///
    /// # Returns
    ///
    /// A vector of `RecentlyAddedManga` structs, each containing:
    /// - Manga ID and page URL
    /// - Title
    /// - Cover image URL
    /// - Description (latest chapter number if available)
    ///
    /// # Errors
    ///
    /// Returns `LatestMangaError` if the HTML cannot be parsed or the
    /// recently added section is missing.
    fn map_latest_mangas(&self, doc: String) -> Result<Vec<RecentlyAddedManga>, LatestMangaError> {
        let html_parser = Scraper::new(HtmlElement::new(doc));
        let parser = LatestMangas::new(html_parser);

        let mangas: Vec<RecentlyAddedManga> = parser
            .scrape_latest_mangas()?
            .into_iter()
            .map(|ma| {
                let manga: RecentlyAddedManga = RecentlyAddedManga {
                    id: ma.page_url,
                    title: ma.title,
                    description: ma.latest_chapter.map(|la| format!("Latest chapter: {la}")).unwrap_or("".to_string()),
                    cover_img_url: ma.cover_url,
                };
                manga
            })
            .collect();
        Ok(mangas)
    }

    /// Maps HTML document to a paginated list of chapters
    ///
    /// Parses the manga detail page HTML to extract all chapters, applies filters
    /// (ordering), and paginates the results.
    ///
    /// # Arguments
    ///
    /// * `doc` - HTML content of the manga page
    /// * `manga_id` - The manga identifier
    /// * `filters` - Chapter filters (ordering, etc.)
    /// * `pagination` - Pagination parameters (current page, items per page)
    ///
    /// # Returns
    ///
    /// A `GetChaptersResponse` containing:
    /// - Paginated list of chapters
    /// - Total number of chapters
    ///
    /// # Notes
    ///
    /// - Chapters are sorted based on `filters.order` (Ascending/Descending)
    /// - Chapter titles are empty strings (not provided by MangaPill)
    /// - Volume numbers are always `None` (not provided by MangaPill)
    ///
    /// # Errors
    ///
    /// Returns `ChaptersError` if the HTML cannot be parsed or the chapters
    /// section is missing.
    fn map_chapters_from_manga_page(
        &self,
        doc: String,
        manga_id: &str,
        filters: ChapterFilters,
        pagination: Pagination,
    ) -> Result<GetChaptersResponse, ChaptersError> {
        let html_parser = Scraper::new(HtmlElement::new(doc));
        let parser = MangaPillChaptersParser::new(html_parser);

        let pages = parser.scrape_list_of_chapters()?;
        let total = pages.chapters.len();

        let mut chapters: Vec<Chapter> = pages
            .chapters
            .into_iter()
            .map(|cha| Chapter {
                id_safe_for_download: cha.full_url.split("/").nth(1).unwrap().to_string(),
                id: cha.full_url,
                manga_id: manga_id.to_string(),
                title: "".to_string(),
                chapter_number: cha.number,
                ..Default::default()
            })
            .collect();

        if filters.order == ChapterOrderBy::Ascending {
            chapters.reverse();
        }

        let res = GetChaptersResponse {
            chapters: chapters
                .into_iter()
                .skip((pagination.items_per_page * (pagination.current_page - 1)) as usize)
                .take(pagination.items_per_page as usize)
                .collect(),
            total_chapters: total as u32,
        };

        Ok(res)
    }

    /// Maps HTML document to search results
    ///
    /// Parses the search page HTML to extract manga search results and determines
    /// if there are more pages available.
    ///
    /// # Arguments
    ///
    /// * `doc` - HTML content of the search results page
    /// * `_filters` - Filter state (currently unused for MangaPill)
    /// * `_pagination` - Pagination parameters (currently unused in mapping)
    ///
    /// # Returns
    ///
    /// A `GetMangasResponse` containing:
    /// - List of matching mangas with basic information
    /// - Total number of results (estimated if pagination exists)
    /// - Whether a next page is available
    ///
    /// # Notes
    ///
    /// - If pagination controls are found, total is set to 10000 (estimated)
    /// - All results are marked as English language
    /// - Status information is included when available
    ///
    /// # Errors
    ///
    /// Returns `SearchPageError` if the HTML cannot be parsed or the search
    /// results section is missing.
    fn map_search_result(
        &self,
        doc: String,
        _filters: <Self as SearchPageProvider>::InnerState,
        _pagination: Pagination,
    ) -> Result<GetMangasResponse, SearchPageError> {
        let html_parser = Scraper::new(HtmlElement::new(doc));
        let parser = SearchPageMangasParser::new(html_parser);

        let search_result = parser.scrape_search_page()?;
        let mut total = search_result.mangas.len();

        let mangas: Vec<SearchManga> = search_result
            .mangas
            .into_iter()
            .map(|ma| SearchManga {
                id: ma.page_url,
                cover_img_url: ma.cover_url,
                title: ma.title,
                status: Some(ma.status.into()),
                languages: vec![Languages::English],
                ..Default::default()
            })
            .collect();

        if search_result.next_page.is_some() || search_result.previous_page.is_some() {
            total = 10000;
        }

        Ok(GetMangasResponse {
            mangas,
            total_mangas: total as u32,
            next_page: search_result.next_page.is_some(),
        })
    }

    /// Creates a new `MangaPillProvider` with the default MangaPill URL
    ///
    /// Convenience method that initializes the provider with the standard
    /// MangaPill base URL (`https://mangapill.com`).
    ///
    /// # Arguments
    ///
    /// * `cache_provider` - A cache implementation for storing HTTP responses
    ///
    /// # Returns
    ///
    /// A new `MangaPillProvider` instance configured for the official MangaPill website
    #[inline]
    pub fn init(cache_provider: Arc<dyn Cacher>) -> Self {
        Self::new(MANGA_PILL_URL.parse().unwrap(), cache_provider)
    }

    /// Fetches a chapter by ID with caching support
    ///
    /// Retrieves chapter data from cache if available, otherwise fetches from
    /// the MangaPill website and caches the result.
    ///
    /// # Arguments
    ///
    /// * `chapter_id` - The chapter identifier (e.g., "chapters/9183-10009000/someone-hertz-chapter-9")
    ///
    /// # Returns
    ///
    /// A `ChapterToRead` struct containing chapter information and page URLs
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails
    /// - The response status is not OK
    /// - The HTML cannot be parsed
    /// - Required chapter data is missing
    async fn get_chapter(&self, chapter_id: &str) -> Result<ChapterToRead, Box<dyn Error>> {
        let url = format!("{}{}", self.base_url, chapter_id);
        let cache = self.cache_provider.get(&url)?;

        match cache {
            Some(cached) => {
                let doc: String = String::from_utf8(cached.data)?;
                let chapter = self.map_chapter_to_read(doc, chapter_id)?;

                Ok(chapter)
            },
            None => {
                let response = self.client.get(&url).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!(
                        "manga page with id:  could not be found on MangaPill, more details about the response:
                        {response:#?}"
                    )
                    .into());
                }

                let doc = response.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &url,
                        data: doc.as_bytes(),
                        duration: Self::MANGA_PAGE_CACHE_DURATION,
                    })
                    .ok();

                Ok(self.map_chapter_to_read(doc, chapter_id)?)
            },
        }
    }

    /// Fetches the complete list of chapters for a manga with caching support
    ///
    /// Retrieves all chapters for a manga from cache if available, otherwise
    /// fetches from the MangaPill website and caches the result.
    ///
    /// # Arguments
    ///
    /// * `manga_id` - The manga identifier (e.g., "manga/9183/someone-hertz")
    ///
    /// # Returns
    ///
    /// A `ListOfChapters` struct containing all chapters for the manga
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The HTTP request fails
    /// - The response status is not OK
    /// - The HTML cannot be parsed
    /// - The chapters section is missing
    async fn get_list_of_chapters(&self, manga_id: &str) -> Result<ListOfChapters, Box<dyn Error>> {
        let url = format!("{}{}", self.base_url, manga_id);
        let cache = self.cache_provider.get(&url)?;

        match cache {
            Some(cached) => {
                let doc: String = String::from_utf8(cached.data)?;
                let html_parser = Scraper::new(HtmlElement::new(doc));
                let parser = MangaPillChaptersParser::new(html_parser);

                let chapters = parser.scrape_list_of_chapters()?;

                let list_of_chapters: ListOfChapters = ListOfChapters::from(chapters);

                Ok(list_of_chapters)
            },
            None => {
                let response = self.client.get(&url).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!(
                        "manga page with id:  could not be found on MangaPill, more details about the response:
                        {response:#?}"
                    )
                    .into());
                }

                let doc = response.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &url,
                        data: doc.as_bytes(),
                        duration: Self::MANGA_PAGE_CACHE_DURATION,
                    })
                    .ok();

                let html_parser = Scraper::new(HtmlElement::new(doc));
                let parser = MangaPillChaptersParser::new(html_parser);

                let chapters = parser.scrape_list_of_chapters()?;

                let list_of_chapters: ListOfChapters = ListOfChapters::from(chapters);

                Ok(list_of_chapters)
            },
        }
    }
}

/// Implementation of `GetRawImage` trait for fetching raw image bytes
///
/// This implementation handles fetching chapter page images with caching support.
/// Images are cached with a long duration since they rarely change.
impl GetRawImage for MangaPillProvider {
    async fn get_raw_image(&self, url: &str) -> Result<bytes::Bytes, Box<dyn Error>> {
        let cache = self.cache_provider.get(url)?;

        match cache {
            Some(cached) => Ok(cached.data.into()),
            None => {
                let response = self.client.get(url).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!("Could not get image on MangaPill with url: {url}").into());
                }

                let bytes = response.bytes().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: url,
                        data: &bytes,
                        duration: CacheDuration::Long,
                    })
                    .ok();

                Ok(bytes)
            },
        }
    }
}

/// Implementation of `DecodeBytesToImage` trait
///
/// Provides functionality to decode raw image bytes into displayable image formats.
impl DecodeBytesToImage for MangaPillProvider {}

/// Implementation of `SearchMangaPanel` trait
///
/// Enables the provider to be used in search panel functionality.
impl SearchMangaPanel for MangaPillProvider {}

/// Implementation of `ProviderIdentity` trait
///
/// Identifies this provider as the MangaPill provider in the system.
impl ProviderIdentity for MangaPillProvider {
    fn name(&self) -> MangaProviders {
        MangaProviders::Mangapill
    }
}

/// Implementation of `SearchMangaById` trait for fetching manga by ID
///
/// Provides functionality to retrieve detailed manga information by its identifier.
/// Results are cached with a very long duration since manga details rarely change.
impl SearchMangaById for MangaPillProvider {
    async fn get_manga_by_id(&self, manga_id: &str) -> Result<super::Manga, Box<dyn Error>> {
        let url = format!("{}{}", self.base_url, manga_id);
        let cache = self.cache_provider.get(&url)?;

        match cache {
            Some(cached) => {
                let doc: String = String::from_utf8(cached.data)?;

                Ok(self.map_manga(doc, manga_id)?)
            },
            None => {
                let response = self.client.get(&url).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!(
                        "manga page with id: {manga_id} could not be found on MangaPill, more details about the response:
                        {response:#?}"
                    )
                    .into());
                }

                let doc = response.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &url,
                        data: doc.as_bytes(),
                        duration: Self::MANGA_PAGE_CACHE_DURATION,
                    })
                    .ok();

                Ok(self.map_manga(doc, manga_id)?)
            },
        }
    }
}

/// Implementation of `HomePageMangaProvider` trait for home page content
///
/// Provides functionality to fetch popular and recently added mangas from the
/// MangaPill home page. Results are cached with a short duration since home
/// page content updates frequently.
impl HomePageMangaProvider for MangaPillProvider {
    async fn get_popular_mangas(&self) -> Result<Vec<super::PopularManga>, Box<dyn Error>> {
        let cache = self.cache_provider.get(self.base_url.as_str())?;

        match cache {
            Some(cached) => {
                let doc: String = String::from_utf8(cached.data)?;

                Ok(self.map_popular_mangas(doc)?)
            },
            None => {
                let response = self.client.get(self.base_url.as_str()).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!(
                        "Could not get popular mangas on MangaPill, more details about the response : {response:#?}"
                    )
                    .into());
                }

                let doc = response.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: self.base_url.as_str(),
                        data: doc.as_bytes(),
                        duration: Self::HOME_PAGE_CACHE_DURATION,
                    })
                    .ok();

                Ok(self.map_popular_mangas(doc)?)
            },
        }
    }

    async fn get_recently_added_mangas(&self) -> Result<Vec<super::RecentlyAddedManga>, Box<dyn Error>> {
        let cache = self.cache_provider.get(self.base_url.as_str())?;

        match cache {
            Some(cached) => {
                let doc: String = String::from_utf8(cached.data)?;

                Ok(self.map_latest_mangas(doc)?)
            },
            None => {
                let response = self.client.get(self.base_url.as_str()).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!(
                        "could not find recently added mangas on MangaPill, more details about the response: {}",
                        response.status()
                    )
                    .into());
                }

                let doc = response.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: self.base_url.as_str(),
                        data: doc.as_bytes(),
                        duration: Self::HOME_PAGE_CACHE_DURATION,
                    })
                    .ok();

                Ok(self.map_latest_mangas(doc)?)
            },
        }
    }
}

/// Implementation of `SearchChapterById` trait for finding chapters by ID
///
/// Provides functionality to search and retrieve a specific chapter by its identifier.
impl SearchChapterById for MangaPillProvider {
    async fn search_chapter(&self, chapter_id: &str, _manga_id: &str) -> Result<ChapterToRead, Box<dyn Error>> {
        let chapter = self.get_chapter(chapter_id).await?;
        Ok(chapter)
    }
}

/// Implementation of `GoToReadChapter` trait for reading chapters
///
/// Provides functionality to read a chapter, returning both the chapter data
/// and the complete list of chapters for navigation purposes.
impl GoToReadChapter for MangaPillProvider {
    async fn read_chapter(&self, chapter_id: &str, manga_id: &str) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
        let chapter = self.get_chapter(chapter_id).await?;
        let list_of_chapters = self.get_list_of_chapters(manga_id).await?;
        Ok((chapter, list_of_chapters))
    }
}

/// Implementation of `GetChapterPages` trait for fetching chapter page URLs
///
/// Provides functionality to retrieve all page image URLs for a given chapter.
/// The `image_quality` parameter is ignored as MangaPill only provides one quality.
/// Results are cached with a long duration.
impl GetChapterPages for MangaPillProvider {
    async fn get_chapter_pages_url_with_extension(
        &self,
        chapter_id: &str,
        _manga_id: &str,
        _image_quality: crate::config::ImageQuality,
    ) -> Result<Vec<ChapterPageUrl>, Box<dyn Error>> {
        let url = format!("{}{}", self.base_url, chapter_id);

        let cache = self.cache_provider.get(&url)?;

        match cache {
            Some(cached) => {
                let doc: String = String::from_utf8(cached.data)?;
                let html_parser = Scraper::new(HtmlElement::new(doc));
                let parser = ChapterPagesScraper::new(html_parser);

                let pages = parser.scrape_pages_from_chapter()?;

                Ok(pages)
            },
            None => {
                let res = self.client.get(&url).send().await?;

                if res.status() != StatusCode::OK {
                    return Err(format!(
                        "Could not get chapter pages for chapter with id: {chapter_id}, more detailes about the response: {res:#?}"
                    )
                    .into());
                }

                let doc = res.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &url,
                        data: doc.as_bytes(),
                        duration: Self::CHAPTER_PAGE_CACHE_DURATION,
                    })
                    .ok();

                let html_parser = Scraper::new(HtmlElement::new(doc));
                let parser = ChapterPagesScraper::new(html_parser);

                let pages = parser.scrape_pages_from_chapter()?;

                Ok(pages)
            },
        }
    }
}

/// Implementation of `FetchChapterBookmarked` trait for bookmarked chapters
///
/// Provides functionality to fetch a bookmarked chapter along with its manga's
/// chapter list for navigation purposes.
impl FetchChapterBookmarked for MangaPillProvider {
    async fn fetch_chapter_bookmarked(
        &self,
        chapter: crate::backend::database::ChapterBookmarked,
    ) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
        self.read_chapter(&chapter.id, &chapter.manga_id).await
    }
}

/// Implementation of `MangaPageProvider` trait for manga page operations
///
/// Provides functionality to retrieve paginated chapter lists for a manga with
/// filtering and ordering support. Also provides a method to get all chapters
/// without pagination.
impl MangaPageProvider for MangaPillProvider {
    async fn get_chapters(
        &self,
        manga_id: &str,
        filters: super::ChapterFilters,
        pagination: super::Pagination,
    ) -> Result<super::GetChaptersResponse, Box<dyn Error>> {
        let url = format!("{}{}", self.base_url, manga_id);

        let cache = self.cache_provider.get(&url)?;
        match cache {
            Some(cached) => {
                let doc: String = String::from_utf8(cached.data)?;

                Ok(self.map_chapters_from_manga_page(doc, manga_id, filters, pagination)?)
            },
            None => {
                let response = self.client.get(&url).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!(
                        "Could not get chapters for manga: {manga_id}, more details about the response: {response:#?}"
                    )
                    .into());
                }

                let doc = response.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &url,
                        data: doc.as_bytes(),
                        duration: Self::MANGA_PAGE_CACHE_DURATION,
                    })
                    .ok();

                Ok(self.map_chapters_from_manga_page(doc, manga_id, filters, pagination)?)
            },
        }
    }

    async fn get_all_chapters(&self, manga_id: &str, _language: Languages) -> Result<Vec<Chapter>, Box<dyn Error>> {
        let res = self
            .get_chapters(manga_id, ChapterFilters::default(), Pagination::new(1, 10000000, 10000000))
            .await?;

        Ok(res.chapters)
    }
}

/// Implementation of `ReaderPageProvider` trait
///
/// Enables the provider to be used in the reader page functionality.
impl ReaderPageProvider for MangaPillProvider {}

/// Implementation of `SearchPageProvider` trait for manga search
///
/// Provides functionality to search for mangas with optional search terms and filters.
/// Results are cached with a very short duration since search results are highly dynamic.
///
/// ## Associated Types
///
/// - `FiltersHandler`: `MangaPillFiltersProvider` - Handles filter state and UI interactions
/// - `InnerState`: `MangaPillFilterState` - Filter state (currently empty for MangaPill)
/// - `Widget`: `MangaPillFilterWidget` - UI widget for displaying filters
impl SearchPageProvider for MangaPillProvider {
    type FiltersHandler = MangaPillFiltersProvider;
    type InnerState = MangaPillFilterState;
    type Widget = MangaPillFilterWidget;

    async fn search_mangas(
        &self,
        search_term: Option<SearchTerm>,
        filters: Self::InnerState,
        pagination: super::Pagination,
    ) -> Result<GetMangasResponse, Box<dyn Error>> {
        let current_page = pagination.current_page;
        let search = search_term.map(|se| format!("q={se}")).unwrap_or_default();

        let endpoint = format!("{}search?{}&page={current_page}", self.base_url, search);

        let cache = self.cache_provider.get(&endpoint)?;

        match cache {
            Some(cached) => {
                let doc: String = String::from_utf8(cached.data)?;

                Ok(self.map_search_result(doc, filters, pagination)?)
            },
            None => {
                let response = self.client.get(&endpoint).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!("Failed to search mangas on mangapill, with the query:{endpoint} \n {response:#?}").into());
                }

                let doc = response.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &endpoint,
                        data: doc.as_bytes(),
                        duration: Self::SEARCH_PAGE_CACHE_DURATION,
                    })
                    .ok();

                Ok(self.map_search_result(doc, filters, pagination)?)
            },
        }
    }
}

/// Implementation of `FeedPageProvider` trait for feed functionality
///
/// Provides functionality to retrieve the latest chapters for a manga, typically
/// used for feed/update notifications. Returns the 4 most recent chapters.
impl FeedPageProvider for MangaPillProvider {
    async fn get_latest_chapters(&self, manga_id: &str) -> Result<Vec<LatestChapter>, Box<dyn Error>> {
        let res = self
            .get_chapters(manga_id, ChapterFilters::default(), Pagination::new(1, 10000, 4))
            .await?;
        Ok(res
            .chapters
            .into_iter()
            .map(|cha| LatestChapter {
                id: cha.id,
                manga_id: manga_id.to_string(),
                title: cha.title,
                language: Languages::English,
                chapter_number: cha.chapter_number,
                volume_number: None,
                publication_date: None,
            })
            .collect())
    }
}

/// Implementation of `MangaProvider` trait
///
/// This marker trait implementation indicates that `MangaPillProvider` is a
/// complete manga provider implementation that can be used throughout the application.
impl MangaProvider for MangaPillProvider {}

#[cfg(test)]
mod tests {

    use httpmock::Method::GET;
    use httpmock::MockServer;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::backend::cache::mock::EmptyCache;
    use crate::backend::manga_provider::MangaStatus;

    //#[tokio::test]
    //async fn it_calls_image_endpoint_with_expected_headers() -> Result<(), Box<dyn Error>> {
    //    let server = MockServer::start_async().await;
    //
    //    let expected = b"some image bytes";
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            // The referer is important to be presented, if not when requesting a chapter page
    //            // it will be blocked by cloudfare
    //            when.method(GET)
    //                .header("user-agent", "Mozilla/5.0 (X11; Linux x86_64; rv:133.0) Gecko/20100101 Firefox/133.0")
    //                .header("referer", "https://MangaPill.com/");
    //
    //            then.status(200).body(*expected);
    //        })
    //        .await;
    //
    //    let MangaPill_provider = MangaPillProvider::new(server.url("/MangaPilltest").parse().unwrap(), EmptyCache::new_arc());
    //
    //    let response = MangaPill_provider.get_raw_image(server.base_url().as_str()).await?;
    //
    //    request.assert_async().await;
    //
    //    assert_eq!(expected.to_vec(), response);
    //
    //    Ok(())
    //}
    //

    #[cfg(feature = "local_only")]
    #[tokio::test]
    async fn test_image_endpoints() -> Result<(), Box<dyn Error>> {
        let manga_pill = MangaPillProvider::new(MANGA_PILL_URL.parse().unwrap(), EmptyCache::new_arc());

        let res = manga_pill
            .get_raw_image("https://cdn.readdetectiveconan.com/file/mangapill/i/9295.jpeg?h=019a1638-5359-7d31-979c-c5b2f44275d0")
            .await?;

        Ok(())
    }

    #[cfg(feature = "local_only")]
    #[tokio::test]
    async fn test_manga_page() -> Result<(), Box<dyn Error>> {
        let manga_pill = MangaPillProvider::new(MANGA_PILL_URL.parse().unwrap(), EmptyCache::new_arc());

        let res = manga_pill.get_manga_by_id("manga/580/blue-lock").await?;

        let expected: Manga = Manga {
            id: "manga/580/blue-lock".to_string(),
            id_safe_for_download: "580".to_string(),
            title: "Blue Lock".to_string(),
            genres: ["Action", "Drama", "Shounen", "Sports"].into_iter().map(|ti| Genres {
                title: ti.to_string(),
                rating: crate::backend::manga_provider::Rating::Normal,
            }).collect(),
            description: r#"The story begins with Japan's elimination from the 2018 FIFA World Cup, which prompts the Japanese Football Union to start a programme scouting high school players who will begin training in preparation for the 2022 Cup. Isagi Youichi, a forward, receives an invitation to this programme soon after his team loses the chance to go to Nationals because he passed to his less-skilled teammate - who missed - without trying to make the game-changing goal by himself.Their coach will be Ego Jinpachi, who intends to "destroy Japanese loser football" by introducing a radical new training regimen: isolate 300 young forwards in a prison-like institution called "Blue Lock" and put them through rigorous training aimed at creating "the world's greatest egotist striker.""#.to_string(),
            status: crate::backend::manga_provider::MangaStatus::Ongoing,
            cover_img_url: "https://cdn.readdetectiveconan.com/file/mangapill/i/580.jpeg".to_string(),
            languages: vec![Languages::English],
            ..Default::default()
        };

        assert_eq!(expected, res);

        Ok(())
    }

    #[cfg(feature = "local_only")]
    #[tokio::test]
    async fn test_home_page() -> Result<(), Box<dyn Error>> {
        let manga_pill = MangaPillProvider::new(MANGA_PILL_URL.parse().unwrap(), EmptyCache::new_arc());

        let res = dbg!(manga_pill.get_popular_mangas().await?);

        let expected: PopularManga = PopularManga {
            id: "manga/5281/sakamoto-days".to_string(),
            title: "Sakamoto Days".to_string(),
            genres: vec![],
            description: "Latest chapter: #236".to_string(),
            status: None,
            cover_img_url: "https://cdn.readdetectiveconan.com/file/mangapill/i/5281.jpeg".to_string(),
        };

        assert_eq!(expected, res.iter().find(|ma| ma.id == expected.id).unwrap().clone());

        let res = dbg!(manga_pill.get_recently_added_mangas().await?);

        let expected: RecentlyAddedManga = RecentlyAddedManga {
            id: "manga/9183/someone-hertz".to_string(),
            title: "Someone Hertz".to_string(),
            description: "Latest chapter: #9".to_string(),
            cover_img_url: "https://cdn.readdetectiveconan.com/file/mangapill/i/9183.jpeg?h=01996198-f927-74ce-b9c7-713fb10c3dbf"
                .to_string(),
        };

        //assert_eq!(expected, res.iter().find(|ma| ma.id == expected.id).unwrap().clone());

        Ok(())
    }

    #[cfg(feature = "local_only")]
    #[tokio::test]
    async fn test_chapter_page() -> Result<(), Box<dyn Error>> {
        let manga_pill = MangaPillProvider::new(MANGA_PILL_URL.parse().unwrap(), EmptyCache::new_arc());

        let res = dbg!(manga_pill.search_chapter("chapters/9183-10009000/someone-hertz-chapter-9", "").await?);

        let expected: ChapterToRead = ChapterToRead {
            id: "chapters/9183-10009000/someone-hertz-chapter-9".to_string(),
            title: "Someone Hertz Chapter 9".to_string(),
            number: 9 as f64,
            volume_number: None,
            num_page_bookmarked: None,
            language: Languages::default(),
            pages_url: vec![],
        };

        assert_eq!(expected.id, res.id);
        assert_eq!(expected.title, res.title);
        assert_eq!(expected.number, res.number);

        assert!(
            res.pages_url
                .iter()
                .find(|url| url.as_str()
                    == "https://cdn.readdetectiveconan.com/file/mangap/9183/10009000/019a6958-979a-7dae-8666-4c15c4562c25/1.jpeg")
                .is_some()
        );

        Ok(())
    }

    #[cfg(feature = "local_only")]
    #[tokio::test]
    async fn test_list_of_chapters() -> Result<(), Box<dyn Error>> {
        let manga_pill = MangaPillProvider::new(MANGA_PILL_URL.parse().unwrap(), EmptyCache::new_arc());

        let res = dbg!(
            manga_pill
                .read_chapter("chapters/9183-10009000/someone-hertz-chapter-9", "manga/9183/someone-hertz")
                .await?
        );

        Ok(())
    }

    #[cfg(feature = "local_only")]
    #[tokio::test]
    async fn test_chapter_pages() -> Result<(), Box<dyn Error>> {
        let manga_pill = MangaPillProvider::new(MANGA_PILL_URL.parse().unwrap(), EmptyCache::new_arc());

        let res = dbg!(
            manga_pill
                .get_chapter_pages_url_with_extension(
                    "chapters/9183-10009000/someone-hertz-chapter-9",
                    "manga/9183/someone-hertz",
                    ImageQuality::Low
                )
                .await?
        );

        Ok(())
    }

    #[cfg(feature = "local_only")]
    #[tokio::test]
    async fn test_get_chapter_of_manga_page() -> Result<(), Box<dyn Error>> {
        let manga_pill = MangaPillProvider::new(MANGA_PILL_URL.parse().unwrap(), EmptyCache::new_arc());

        let res = dbg!(
            manga_pill
                .get_chapters("manga/9183/someone-hertz", ChapterFilters::default(), Pagination::default())
                .await?
        );

        let res = manga_pill
            .get_chapters("manga/5281/sakamoto-days", ChapterFilters::default(), Pagination::default())
            .await?;

        assert_eq!(236, res.total_chapters);
        Ok(())
    }

    #[cfg(feature = "local_only")]
    #[tokio::test]
    async fn test_search_mangas() -> Result<(), Box<dyn Error>> {
        let manga_pill = MangaPillProvider::new(MANGA_PILL_URL.parse().unwrap(), EmptyCache::new_arc());

        let res = dbg!(
            manga_pill
                .search_mangas(SearchTerm::trimmed("school"), MangaPillFilterState {}, Pagination::default())
                .await?
        );

        let expected: SearchManga = SearchManga {
            id: "manga/3760/school-days".to_string(),
            title: "School Days".to_string(),
            genres: vec![],
            description: None,
            status: Some(MangaStatus::Completed),
            cover_img_url: "https://cdn.readdetectiveconan.com/file/mangapill/i/3760.jpg".to_string(),
            languages: vec![Languages::default()],
            artist: None,
            author: None,
        };

        assert_eq!(expected, res.mangas.first().unwrap().clone());

        let res = dbg!(
            manga_pill
                .search_mangas(SearchTerm::trimmed(""), MangaPillFilterState {}, Pagination::default())
                .await?
        );

        Ok(())
    }

    #[cfg(feature = "local_only")]
    #[tokio::test]
    async fn tes_get_latest_chapters_of_manga() -> Result<(), Box<dyn Error>> {
        let manga_pill = MangaPillProvider::new(MANGA_PILL_URL.parse().unwrap(), EmptyCache::new_arc());

        let res = dbg!(manga_pill.get_latest_chapters("manga/3760/school-days").await?);

        Ok(())
    }
}
