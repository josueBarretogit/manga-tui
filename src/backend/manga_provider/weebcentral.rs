use std::error::Error;
use std::fmt::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use filter_state::{WeebcentralFilterState, WeebcentralFiltersProvider};
use filter_widget::WeebcentralFilterWidget;
use http::header::{ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, CACHE_CONTROL, CONNECTION, HOST, REFERER};
use http::{HeaderMap, HeaderValue, StatusCode, status};
use manga_tui::SearchTerm;
use reqwest::cookie::Jar;
use reqwest::{Client, Url};
use response::{
    ChapterPageData, ChapterPagesLinks, LatestMangas, MangaPageData, PopularMangasWeebCentral, SearchPageMangas,
    WeebcentralChapters,
};

use super::{
    Author, Chapter, ChapterFilters, ChapterOrderBy, ChapterPageUrl, DecodeBytesToImage, FeedPageProvider, FetchChapterBookmarked,
    Genres, GetChapterPages, GetChaptersResponse, GetMangasResponse, GetRawImage, GoToReadChapter, HomePageMangaProvider,
    Languages, LatestChapter, ListOfChapters, Manga, MangaPageProvider, MangaProvider, MangaProviders, Pagination, PopularManga,
    ProviderIdentity, ReaderPageProvider, RecentlyAddedManga, SearchChapterById, SearchMangaById, SearchMangaPanel,
    SearchPageProvider,
};
use crate::backend::cache::{Cacher, InsertEntry};
use crate::backend::html_parser::{HtmlElement, ParseHtml};
use crate::backend::manga_provider::ChapterToRead;
use crate::config::ImageQuality;

pub mod filter_state;
pub mod filter_widget;
mod response;

pub static WEEBCENTRAL_BASE_URL: &str = "https://weebcentral.com/";

/// Weebcentral: `https://weebcentral.com/`
/// Some things to keep in mind:
/// - This site does not provide which volume a chapter is in and the chapter's title is also not provided
/// - The url of a Manga page can be built like this: `https://weebcentral.com/series/{manga_id}`
/// - The url of a Chapter page can be built like this: `https://weebcentral.com/chapter/{chapter_id}`
/// - The only language they provide is english,
/// - Since it is a website headers that mimic the behavior of a browser must be used, including a User agent like : `Mozilla/5.0
///   (Windows NT 10.0; Win64; x64; rv:133.0) Gecko/20100101 Firefox/133.0`
/// - There is no way of getting images with lower or higher quality, so `image_quality` doesnt apply to weebcentral
/// - The `Referer` header `https://weebcentral.com/` must be used to get chapter pages when reading a chapter or else cloudfare
///   blocks the request, it is not required in other requests
#[derive(Clone, Debug)]
pub struct WeebcentralProvider {
    client: Client,
    base_url: Url,
    chapter_pages_header: HeaderMap,
    cache_provider: Arc<dyn Cacher>,
}

impl WeebcentralProvider {
    const CHAPTER_PAGE_CACHE_DURATION: Duration = Duration::from_secs(30);
    const HOME_PAGE_CACHE_DURATION: Duration = Duration::from_secs(10);
    const MANGA_PAGE_CACHE_DURATION: Duration = Duration::from_secs(40);
    /// The search page cache is the shortest because it may change a lot
    const SEARCH_PAGE_CACHE_DURATION: Duration = Duration::from_secs(5);

    pub fn new(base_url: Url, cache_provider: Arc<dyn Cacher>) -> Self {
        let mut default_headers = HeaderMap::new();

        default_headers.insert(REFERER, HeaderValue::from_static("https://google.com"));
        default_headers.insert(HOST, HeaderValue::from_static("weebcentral.com"));
        default_headers.insert(ACCEPT, HeaderValue::from_static("*/*"));

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

        let mut chapter_pages_header = HeaderMap::new();

        chapter_pages_header.insert(CACHE_CONTROL, HeaderValue::from_static("max-age=604800"));
        chapter_pages_header.insert(REFERER, HeaderValue::from_static(WEEBCENTRAL_BASE_URL));
        chapter_pages_header.insert(CONNECTION, HeaderValue::from_static("keep-alive"));
        chapter_pages_header.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate"));

        chapter_pages_header
            .insert(ACCEPT, HeaderValue::from_static("image/avif,image/webp,image/png,image/svg+xml,image/*;q=0.8,*/*;q=0.5"));

        let client = Client::builder()
            .cookie_store(true)
            .timeout(Duration::from_secs(30))
            .default_headers(default_headers)
            .user_agent("Mozilla/5.0 (X11; Linux x86_64; rv:133.0) Gecko/20100101 Firefox/133.0")
            .build()
            .unwrap();

        Self {
            client,
            base_url,
            cache_provider,
            chapter_pages_header,
        }
    }

    /// Constructs the url with which we can get the pages of a chapter
    /// Returns https://weebcentral.com/chapters/01JJB9BP43FHYCHAAZDVXKPSEW/images?is_prev=False&current_page=1&reading_style=long_strip
    fn make_chapter_pages_url(&self, chapter_id: &str) -> String {
        format!("{}chapters/{chapter_id}/images?is_prev=False&current_page=1&reading_style=long_strip", self.base_url)
    }

    /// Constructs the url with which we can get the list of chapters of a manga
    /// Returns https://weebcentral.com/series/01JEXZK546WKQ91SNBTXYPT3VN/full-chapter-list
    fn make_full_chapter_list_url(&self, manga_id: &str) -> String {
        format!("{}series/{manga_id}/full-chapter-list", self.base_url)
    }

    /// Constructs the url with which we can get the chapter page
    /// Returns https://weebcentral.com/chapters/01JEXZK546WKQ91SNBTXYPT3VN
    fn make_chapter_page_url(&self, chapter_id: &str) -> String {
        format!("{}chapters/{chapter_id}", self.base_url)
    }

    /// Constructs the url with which we can get the manga page
    /// Returns https://weebcentral.com/series/01JEXZK546WKQ91SNBTXYPT3VN
    fn make_manga_page_url(&self, manga_id: &str) -> String {
        format!("{}series/{manga_id}", self.base_url)
    }

    /// Weebcentral doesn't provide:
    /// - title
    /// - volume_number
    /// - thus volume_number cannot be set
    fn map_chapter_to_read(&self, chapter_id: &str, chapter_data: ChapterPageData, pages: Vec<ChapterPageUrl>) -> ChapterToRead {
        ChapterToRead {
            id: chapter_id.to_string(),
            title: "".to_string(),
            number: chapter_data.number.parse().unwrap_or_default(),
            volume_number: None,
            num_page_bookmarked: None,
            language: Languages::English,
            pages_url: pages.into_iter().map(|page| page.url).collect(),
        }
    }

    /// Weebcentral doesn't provide:
    /// - chapter title
    /// - volume_number
    fn map_chapters(&self, chapters: WeebcentralChapters, manga_id: &str) -> Vec<Chapter> {
        chapters
            .chapters
            .into_iter()
            .map(|chap| Chapter {
                id: chap.id.clone(),
                id_safe_for_download: chap.id,
                manga_id: manga_id.to_string(),
                title: "".to_string(),
                language: Languages::English,
                chapter_number: chap.number,
                volume_number: None,
                scanlator: Some("Weeb central".to_string()),
                publication_date: chap.datetime,
            })
            .collect()
    }

    fn map_chapters_with_filters(
        &self,
        chapters: WeebcentralChapters,
        manga_id: &str,
        filters: ChapterFilters,
        pagination: Pagination,
    ) -> GetChaptersResponse {
        let total_chapters = chapters.chapters.len();

        let mut chapters: Vec<Chapter> = self.map_chapters(chapters, manga_id);

        if filters.order == ChapterOrderBy::Ascending {
            chapters.reverse();
        }

        let from = pagination.index_to_slice_from();
        let to = pagination.to_index(total_chapters);

        let chapters = chapters.as_slice().get(from..to).unwrap_or(&[]);

        GetChaptersResponse {
            total_chapters: total_chapters as u32,
            chapters: chapters.to_vec(),
        }
    }

    /// Weebcentral doesn't provide:
    /// - chapter title
    /// - volume_number
    fn map_chapters_latest_chapters(&self, chapters: WeebcentralChapters, manga_id: &str) -> Vec<LatestChapter> {
        chapters
            .chapters
            .into_iter()
            .take(5)
            .map(|chap| LatestChapter {
                id: chap.id.clone(),
                manga_id: manga_id.to_string(),
                title: "".to_string(),
                language: Languages::English,
                chapter_number: chap.number,
                volume_number: None,
                publication_date: chap.datetime,
            })
            .collect()
    }

    async fn get_chapter(&self, chapter_id: &str) -> Result<ChapterToRead, Box<dyn Error>> {
        let pages_url = self.get_chapter_pages_url_with_extension(chapter_id, "", ImageQuality::default()).await?;

        let chapter_page_url = self.make_chapter_page_url(chapter_id);

        let cache = self.cache_provider.get(&chapter_page_url)?;

        match cache {
            Some(cached) => {
                let chapter_data = ChapterPageData::parse_html(HtmlElement::new(cached.data))?;

                Ok(self.map_chapter_to_read(chapter_id, chapter_data, pages_url))
            },
            None => {
                let response = self.client.get(&chapter_page_url).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!("Could not get additional data for chapter with id: {chapter_id} {:#?}", response).into());
                }

                let doc = response.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &chapter_page_url,
                        data: &doc,
                        duration: Self::CHAPTER_PAGE_CACHE_DURATION,
                    })
                    .ok();

                let chapter_data = ChapterPageData::parse_html(HtmlElement::new(doc))?;

                Ok(self.map_chapter_to_read(chapter_id, chapter_data, pages_url))
            },
        }
    }

    async fn get_list_of_chapters(&self, manga_id: &str) -> Result<ListOfChapters, Box<dyn Error>> {
        let url = self.make_full_chapter_list_url(manga_id);

        let cache = self.cache_provider.get(&url)?;

        match cache {
            Some(cached) => {
                let chapters = WeebcentralChapters::parse_html(HtmlElement::new(cached.data))?;

                Ok(ListOfChapters::from(chapters))
            },
            None => {
                let response = self.client.get(&url).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!(
                        "Could not get list of chapters on weebcentral, more details about the response: {:#?}",
                        response
                    )
                    .into());
                }

                let doc = response.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &url,
                        data: &doc,
                        duration: Self::CHAPTER_PAGE_CACHE_DURATION,
                    })
                    .ok();

                let chapters = WeebcentralChapters::parse_html(HtmlElement::new(doc))?;

                Ok(ListOfChapters::from(chapters))
            },
        }
    }
}

impl GetRawImage for WeebcentralProvider {
    async fn get_raw_image(&self, url: &str) -> Result<bytes::Bytes, Box<dyn Error>> {
        let response = self.client.get(url).headers(self.chapter_pages_header.clone()).send().await?;

        if response.status() != StatusCode::OK {
            return Err(format!("Could not get image on weebcentral with url: {url}").into());
        }

        Ok(response.bytes().await?)
    }
}

impl DecodeBytesToImage for WeebcentralProvider {}

impl SearchMangaPanel for WeebcentralProvider {}

impl HomePageMangaProvider for WeebcentralProvider {
    async fn get_popular_mangas(&self) -> Result<Vec<super::PopularManga>, Box<dyn Error>> {
        let cache = self.cache_provider.get(self.base_url.as_str())?;

        match cache {
            Some(cached) => {
                let response = PopularMangasWeebCentral::parse_html(HtmlElement::new(cached.data))?;

                Ok(response.mangas.into_iter().map(PopularManga::from).collect())
            },
            None => {
                let response = self.client.get(self.base_url.clone()).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!(
                        "could not get popular mangas on weebcentral, more details about the response : {:#?}",
                        response
                    )
                    .into());
                }

                let doc = response.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: self.base_url.as_str(),
                        data: &doc,
                        duration: Self::HOME_PAGE_CACHE_DURATION,
                    })
                    .ok();

                let response = PopularMangasWeebCentral::parse_html(HtmlElement::new(doc))?;

                Ok(response.mangas.into_iter().map(PopularManga::from).collect())
            },
        }
    }

    async fn get_recently_added_mangas(&self) -> Result<Vec<super::RecentlyAddedManga>, Box<dyn Error>> {
        let cache = self.cache_provider.get(self.base_url.as_str())?;

        match cache {
            Some(cached) => {
                let new_mangas = LatestMangas::parse_html(HtmlElement::new(cached.data))?;

                Ok(new_mangas.mangas.into_iter().map(RecentlyAddedManga::from).collect())
            },
            None => {
                let response = self.client.get(self.base_url.clone()).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!(
                        "could not find recently added mangas on weebcentral, more details about the response: {}",
                        response.status()
                    )
                    .into());
                }

                let doc = response.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: self.base_url.as_str(),
                        data: &doc,
                        duration: Self::HOME_PAGE_CACHE_DURATION,
                    })
                    .ok();

                let new_mangas = LatestMangas::parse_html(HtmlElement::new(doc))?;

                Ok(new_mangas.mangas.into_iter().map(RecentlyAddedManga::from).collect())
            },
        }
    }
}

impl SearchMangaById for WeebcentralProvider {
    async fn get_manga_by_id(&self, manga_id: &str) -> Result<super::Manga, Box<dyn Error>> {
        let url = self.make_manga_page_url(manga_id);
        let cache = self.cache_provider.get(&url)?;

        match cache {
            Some(cached_page) => {
                let manga = MangaPageData::parse_html(HtmlElement::new(cached_page.data))?;

                Ok(Manga::from(manga))
            },
            None => {
                let response = self.client.get(&url).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!(
                        "manga page with id: {manga_id} could not be found on weebcentral, more details about the response: {:#?}",
                        response
                    )
                    .into());
                }

                let doc = response.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &url,
                        data: &doc,
                        duration: Self::MANGA_PAGE_CACHE_DURATION,
                    })
                    .ok();

                let manga = MangaPageData::parse_html(HtmlElement::new(doc))?;

                Ok(Manga::from(manga))
            },
        }
    }
}

impl GoToReadChapter for WeebcentralProvider {
    async fn read_chapter(&self, chapter_id: &str, manga_id: &str) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
        let chapter = self.get_chapter(chapter_id).await?;
        let list_of_chapters = self.get_list_of_chapters(manga_id).await?;
        Ok((chapter, list_of_chapters))
    }
}

impl GetChapterPages for WeebcentralProvider {
    /// On Weebcentral `manga_id` is not required to get a chapter's pages and neither is
    /// `image_quality`
    async fn get_chapter_pages_url_with_extension(
        &self,
        chapter_id: &str,
        manga_id: &str,
        image_quality: crate::config::ImageQuality,
    ) -> Result<Vec<ChapterPageUrl>, Box<dyn Error>> {
        let url = self.make_chapter_pages_url(chapter_id);

        let cache = self.cache_provider.get(&url)?;

        match cache {
            Some(cached) => {
                let pages = ChapterPagesLinks::parse_html(HtmlElement::new(cached.data))?;

                Ok(pages.pages.into_iter().map(ChapterPageUrl::from).collect())
            },
            None => {
                let res = self.client.get(&url).send().await?;

                if res.status() != StatusCode::OK {
                    return Err(format!(
                        "Could not get chapter pages for chapter with id: {chapter_id}, more detailes about the response: {:#?}",
                        res
                    )
                    .into());
                }

                let html = res.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &url,
                        data: &html,
                        duration: Self::CHAPTER_PAGE_CACHE_DURATION,
                    })
                    .ok();

                let pages = ChapterPagesLinks::parse_html(HtmlElement::new(html))?;

                Ok(pages.pages.into_iter().map(ChapterPageUrl::from).collect())
            },
        }
    }
}

impl FetchChapterBookmarked for WeebcentralProvider {
    async fn fetch_chapter_bookmarked(
        &self,
        chapter: crate::backend::database::ChapterBookmarked,
    ) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
        self.read_chapter(&chapter.id, &chapter.manga_id).await
    }
}

impl MangaPageProvider for WeebcentralProvider {
    async fn get_chapters(
        &self,
        manga_id: &str,
        filters: super::ChapterFilters,
        pagination: super::Pagination,
    ) -> Result<super::GetChaptersResponse, Box<dyn Error>> {
        let full_list_url = self.make_full_chapter_list_url(manga_id);

        let cache = self.cache_provider.get(&full_list_url)?;
        match cache {
            Some(cached) => {
                let chapters = WeebcentralChapters::parse_html(HtmlElement::new(cached.data))?;
                Ok(self.map_chapters_with_filters(chapters, manga_id, filters, pagination))
            },
            None => {
                let response = self.client.get(&full_list_url).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!(
                        "Could not get chapters for manga: {manga_id}, more details about the response: {:#?}",
                        response
                    )
                    .into());
                }
                let response = response.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &full_list_url,
                        data: &response,
                        duration: Self::CHAPTER_PAGE_CACHE_DURATION,
                    })
                    .ok();

                let chapters = WeebcentralChapters::parse_html(HtmlElement::new(response))?;

                Ok(self.map_chapters_with_filters(chapters, manga_id, filters, pagination))
            },
        }
    }

    async fn get_all_chapters(&self, manga_id: &str, language: Languages) -> Result<Vec<Chapter>, Box<dyn Error>> {
        let full_list_url = self.make_full_chapter_list_url(manga_id);

        let cache = self.cache_provider.get(&full_list_url)?;
        match cache {
            Some(cached) => {
                let chapters = WeebcentralChapters::parse_html(HtmlElement::new(cached.data))?;

                Ok(self.map_chapters(chapters, manga_id))
            },
            None => {
                let response = self.client.get(&full_list_url).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!(
                        "Could not get chapters for manga: {manga_id}, more details about the response: {:#?}",
                        response
                    )
                    .into());
                }
                let response = response.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &full_list_url,
                        data: &response,
                        duration: Self::CHAPTER_PAGE_CACHE_DURATION,
                    })
                    .ok();

                let chapters = WeebcentralChapters::parse_html(HtmlElement::new(response))?;

                Ok(self.map_chapters(chapters, manga_id))
            },
        }
    }
}

impl SearchChapterById for WeebcentralProvider {
    async fn search_chapter(&self, chapter_id: &str, manga_id: &str) -> Result<ChapterToRead, Box<dyn Error>> {
        let chapter = self.get_chapter(chapter_id).await?;
        Ok(chapter)
    }
}

impl ReaderPageProvider for WeebcentralProvider {}

impl SearchPageProvider for WeebcentralProvider {
    type FiltersHandler = WeebcentralFiltersProvider;
    type InnerState = WeebcentralFilterState;
    type Widget = WeebcentralFilterWidget;

    async fn search_mangas(
        &self,
        search_term: Option<SearchTerm>,
        filters: Self::InnerState,
        pagination: super::Pagination,
    ) -> Result<GetMangasResponse, Box<dyn Error>> {
        let limit = 24;
        let offset = if pagination.current_page == 1 { 0 } else { limit * pagination.current_page };

        let search = match search_term {
            Some(text) => format!("text={text}"),
            None => "".to_string(),
        };

        let url = format!(
            "{}search/data?{search}&limit={limit}&offset={offset}&sort=Best+Match&order=Descending&official=Any&anime=Any&adult=Any&display_mode=Full+Display",
            self.base_url
        );

        let cache = self.cache_provider.get(&url)?;

        match cache {
            Some(cached) => {
                let mangas = SearchPageMangas::parse_html(HtmlElement::new(cached.data))?;

                Ok(GetMangasResponse::from(mangas))
            },
            None => {
                let res = self.client.get(&url).send().await?;

                if res.status() != StatusCode::OK {
                    return Err(format!(
                        "Could not search on weebcentral with url: {url}, more details about the response: {:#?}",
                        res
                    )
                    .into());
                }

                let doc = res.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &url,
                        data: &doc,
                        duration: Self::SEARCH_PAGE_CACHE_DURATION,
                    })
                    .ok();

                let mangas = SearchPageMangas::parse_html(HtmlElement::new(doc))?;

                Ok(GetMangasResponse::from(mangas))
            },
        }
    }
}

impl FeedPageProvider for WeebcentralProvider {
    async fn get_latest_chapters(&self, manga_id: &str) -> Result<Vec<LatestChapter>, Box<dyn Error>> {
        let full_list_url = self.make_full_chapter_list_url(manga_id);

        let cache = self.cache_provider.get(&full_list_url)?;
        match cache {
            Some(cached) => {
                let chapters = WeebcentralChapters::parse_html(HtmlElement::new(cached.data))?;

                Ok(self.map_chapters_latest_chapters(chapters, manga_id))
            },
            None => {
                let response = self.client.get(&full_list_url).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!(
                        "Could not get chapters for manga: {manga_id}, more details about the response: {:#?}",
                        response
                    )
                    .into());
                }
                let response = response.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &full_list_url,
                        data: &response,
                        duration: Self::CHAPTER_PAGE_CACHE_DURATION,
                    })
                    .ok();

                let chapters = WeebcentralChapters::parse_html(HtmlElement::new(response))?;

                Ok(self.map_chapters_latest_chapters(chapters, manga_id))
            },
        }
    }
}

impl ProviderIdentity for WeebcentralProvider {
    fn name(&self) -> super::MangaProviders {
        MangaProviders::Weebcentral
    }
}

impl MangaProvider for WeebcentralProvider {}
