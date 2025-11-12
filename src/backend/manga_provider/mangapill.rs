use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use chrono::NaiveDate;
use http::header::{ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, CACHE_CONTROL, CONNECTION, HOST, REFERER};
use http::{HeaderMap, HeaderValue, StatusCode};
use manga_tui::SearchTerm;
use reqwest::{Client, Url};
use response::{ChapterPageData, LatestMangas, MangaPageData, PopularMangasMangaPill, SearchPageMangas};
use serde::Serialize;

use super::{
    Chapter, ChapterFilters, ChapterOrderBy, ChapterPageUrl, DecodeBytesToImage, FeedPageProvider, FetchChapterBookmarked,
    GetChapterPages, GetChaptersResponse, GetMangasResponse, GetRawImage, GoToReadChapter, HomePageMangaProvider, Languages,
    LatestChapter, ListOfChapters, Manga, MangaPageProvider, MangaProvider, MangaProviders, Pagination, PopularManga,
    ProviderIdentity, ReaderPageProvider, RecentlyAddedManga, SearchChapterById, SearchMangaById, SearchMangaPanel,
    SearchPageProvider,
};
use crate::backend::cache::{CacheDuration, Cacher, InsertEntry};
use crate::backend::html_parser::scraper::Scraper;
use crate::backend::html_parser::{HtmlElement, ParseHtml};
use crate::backend::manga_provider::mangapill::filter_state::{MangaPillFilterState, MangaPillFiltersProvider};
use crate::backend::manga_provider::mangapill::filter_widget::MangaPillFilterWidget;
use crate::backend::manga_provider::mangapill::response::{
    ChapterPageDataError, ChapterPageDataParser, ChapterPagesScraper, ChaptersError, LatestMangaError, MangaPageDataParser,
    MangaPageError, MangaPillChaptersParser, MangaPillStatus, PopularMangaParseError, SearchPageError, SearchPageMangasParser,
};
use crate::backend::manga_provider::{ChapterToRead, Genres, SearchManga};
use crate::config::ImageQuality;

pub mod filter_state;
pub mod filter_widget;
mod response;

static MANGA_PILL_URL: &str = "https://mangapill.com";

/// MangaPill: `https://MangaPill.com/`
/// Some things to keep in mind:
/// - This site does not provide which volume a chapter is in and the chapter's title is also not provided
/// - The url of a Manga page can be built like this: `https://MangaPill.com/series/{manga_id}`
/// - The url of a Chapter page can be built like this: `https://MangaPill.com/chapter/{chapter_id}`
/// - The only language they provide is english,
/// - Since it is a website headers that mimic the behavior of a browser must be used, including a User agent like : `Mozilla/5.0
///   (Windows NT 10.0; Win64; x64; rv:133.0) Gecko/20100101 Firefox/133.0`
/// - There is no way of getting images with lower or higher quality, so `image_quality` doesnt apply to MangaPill
/// - The `Referer` header `https://MangaPill.com/` must be used to get chapter pages when reading a chapter or else cloudfare
///   blocks the request, it is not required in other requests
#[derive(Clone, Debug)]
pub struct MangaPillProvider {
    client: Client,
    base_url: Url,
    chapter_pages_header: HeaderMap,
    cache_provider: Arc<dyn Cacher>,
}

impl MangaPillProvider {
    const CHAPTER_PAGE_CACHE_DURATION: CacheDuration = CacheDuration::Long;
    const HOME_PAGE_CACHE_DURATION: CacheDuration = CacheDuration::Short;
    const MANGA_PAGE_CACHE_DURATION: CacheDuration = CacheDuration::LongLong;
    /// The search page cache is the shortest because it may change a lot
    const SEARCH_PAGE_CACHE_DURATION: CacheDuration = CacheDuration::VeryShort;

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

        let mut chapter_pages_header = HeaderMap::new();

        chapter_pages_header.insert(CACHE_CONTROL, HeaderValue::from_static("max-age=604800"));
        chapter_pages_header.insert(REFERER, HeaderValue::from_static(MANGA_PILL_URL));
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

    fn map_search_result(
        &self,
        doc: String,
        filters: <Self as SearchPageProvider>::InnerState,
        pagination: Pagination,
    ) -> Result<GetMangasResponse, SearchPageError> {
        let html_parser = Scraper::new(HtmlElement::new(doc));
        let parser = SearchPageMangasParser::new(html_parser);

        let search_result = parser.scrape_search_page()?;
        let total = search_result.mangas.len();

        let mut mangas: Vec<SearchManga> = search_result
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

        Ok(GetMangasResponse {
            mangas,
            total_mangas: total as u32,
        })
    }

    #[inline]
    pub fn init(cache_provider: Arc<dyn Cacher>) -> Self {
        Self::new(MANGA_PILL_URL.parse().unwrap(), cache_provider)
    }

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

impl DecodeBytesToImage for MangaPillProvider {}
impl SearchMangaPanel for MangaPillProvider {}

impl ProviderIdentity for MangaPillProvider {
    fn name(&self) -> MangaProviders {
        MangaProviders::Mangapill
    }
}

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
        let cache = self.cache_provider.get(&self.base_url.as_str())?;

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

impl SearchChapterById for MangaPillProvider {
    async fn search_chapter(&self, chapter_id: &str, manga_id: &str) -> Result<ChapterToRead, Box<dyn Error>> {
        let chapter = self.get_chapter(chapter_id).await?;
        Ok(chapter)
    }
}

impl GoToReadChapter for MangaPillProvider {
    async fn read_chapter(&self, chapter_id: &str, manga_id: &str) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
        let chapter = self.get_chapter(chapter_id).await?;
        let list_of_chapters = self.get_list_of_chapters(manga_id).await?;
        Ok((chapter, list_of_chapters))
    }
}

impl GetChapterPages for MangaPillProvider {
    async fn get_chapter_pages_url_with_extension(
        &self,
        chapter_id: &str,
        manga_id: &str,
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
                        duration: Self::MANGA_PAGE_CACHE_DURATION,
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

impl FetchChapterBookmarked for MangaPillProvider {
    async fn fetch_chapter_bookmarked(
        &self,
        chapter: crate::backend::database::ChapterBookmarked,
    ) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
        self.read_chapter(&chapter.id, &chapter.manga_id).await
    }
}

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

    async fn get_all_chapters(&self, manga_id: &str, language: Languages) -> Result<Vec<Chapter>, Box<dyn Error>> {
        let res = self
            .get_chapters(manga_id, ChapterFilters::default(), Pagination::new(1, 10000000, 10000000))
            .await?;

        Ok(res.chapters)
    }
}

impl ReaderPageProvider for MangaPillProvider {}

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
        let search = search_term.map(|se| format!("q={se}")).unwrap_or_default();

        let endpoint = format!("{}search?{}", self.base_url, search);

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

                Ok(self.map_search_result(doc, filters, pagination)?)
            },
        }
    }
}

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
