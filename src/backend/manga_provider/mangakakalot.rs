use std::error::Error;
use std::fmt::Write;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use filter_state::{MangakakalotFilterState, MangakakalotFiltersProvider};
use filter_widget::MangakakalotFilterWidget;
use http::header::{ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, CACHE_CONTROL, CONNECTION, COOKIE, HOST, REFERER};
use http::{HeaderMap, HeaderValue, StatusCode};
use manga_tui::SearchTerm;
use reqwest::cookie::Jar;
use reqwest::{Client, Url};
use response::{
    extract_id_from_url, from_timestamp, ChapterPageResponse, ChapterUrls, GetPopularMangasResponse, MangaPageData,
    ManganatoChaptersResponse, NewAddedMangas, SearchMangaResponse,
};

use super::{
    Author, Chapter, ChapterOrderBy, ChapterPageUrl, DecodeBytesToImage, FeedPageProvider, FetchChapterBookmarked, Genres,
    GetChapterPages, GetMangasResponse, GetRawImage, GoToReadChapter, HomePageMangaProvider, Languages, LatestChapter,
    ListOfChapters, MangaPageProvider, MangaProvider, PopularManga, ProviderIdentity, ReaderPageProvider, RecentlyAddedManga,
    SearchChapterById, SearchMangaById, SearchMangaPanel, SearchPageProvider,
};
use crate::backend::html_parser::{HtmlElement, ParseHtml};
use crate::backend::manga_provider::ChapterToRead;

pub static MANGAKAKALOT_BASE_URL: &str = "https://www.mangakakalot.gg";
static MANGAKAKALOT_REFERER: &str = "https://www.mangakakalot.gg/";

pub mod filter_state;
pub mod filter_widget;
pub mod response;

/// Mangakakalot: `https://www.mangakakalot.gg/`
/// Some things to keep in mind:
/// - All `ids` of manga and chapter are actually Urls
/// - The only language they provide is english,
/// - Since it is a website headers that mimic the behavior of a browser must be used, including a User agent like : `Mozilla/5.0
///   (Windows NT 10.0; Win64; x64; rv:133.0) Gecko/20100101 Firefox/133.0`
/// - There is no way of getting images with lower or higher quality, so `image_quality` doesnt apply to manganto
/// - The `Referer` header `https://www.mangakakalot.gg/` must be used to get chapter pages when reading a chapter or else cloudfare
///   blocks the request, it is not required on the rest of requests
#[derive(Clone, Debug)]
pub struct MangakakalotProvider {
    client: Client,
    base_url: Url,
    image_client: Client,
}

impl MangakakalotProvider {
    pub const MANGANATO_MANGA_LANGUAGE: &[Languages] = &[Languages::English];

    pub fn new(base_url: Url) -> Self {
        let mut default_headers = HeaderMap::new();

        default_headers.insert(REFERER, HeaderValue::from_static("https://google.com"));
        default_headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate"));
        default_headers.insert(CACHE_CONTROL, HeaderValue::from_static("max-age=604800"));
        default_headers.insert(
            ACCEPT,
            HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8,application/json"),
        );

        default_headers.insert("priority", HeaderValue::from_static("u=0, i"));
        default_headers.insert("sec-fetch-site", HeaderValue::from_static("cross-site"));
        default_headers.insert("sec-fetch-mode", HeaderValue::from_static("navigate"));
        default_headers.insert("sec-fetch-user", HeaderValue::from_static("?1"));
        default_headers.insert("sec-fetch-dest", HeaderValue::from_static("document"));
        default_headers.insert("TE", HeaderValue::from_static("trailers"));
        default_headers.insert("DNT", HeaderValue::from_static("1"));
        default_headers.insert("Upgrade-Insecure-Requests", HeaderValue::from_static("1"));
        default_headers.insert(CONNECTION, HeaderValue::from_static("keep-alive"));
        default_headers.insert(HOST, HeaderValue::from_static("www.mangakakalot.gg"));
        default_headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.5"));

        let mut chapter_pages_header = HeaderMap::new();

        chapter_pages_header.insert(CACHE_CONTROL, HeaderValue::from_static("max-age=604800"));
        chapter_pages_header.insert(REFERER, HeaderValue::from_static(MANGAKAKALOT_REFERER));
        chapter_pages_header.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate"));
        chapter_pages_header
            .insert(ACCEPT, HeaderValue::from_static("image/avif,image/webp,image/png,image/svg+xml,image/*;q=0.8,*/*;q=0.5"));

        chapter_pages_header.insert("Upgrade-Insecure-Requests", HeaderValue::from_static("1"));
        chapter_pages_header.insert(CONNECTION, HeaderValue::from_static("keep-alive"));

        let  client = Client::builder()
            .cookie_store(true)
            .default_headers(default_headers)
            .timeout(Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/51.0.2704.79 Safari/537.36 Edge/14.14393")
            .build()
            .unwrap();

        let  image_client = Client::builder()
            .cookie_store(true)
            .default_headers(chapter_pages_header)
            .timeout(Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/51.0.2704.79 Safari/537.36 Edge/14.14393")
            .build()
            .unwrap();

        Self {
            client,
            base_url,
            image_client,
        }
    }

    /// The search query / term in manganato needs to be something like this: "Oshi no ko" =>
    /// "oshi_no_ko"
    fn format_search_term(search_term: SearchTerm) -> String {
        let mut search: String = search_term.get().split(" ").fold(String::new(), |mut acc, word| {
            let _ = write!(acc, "{}_", word);
            acc
        });

        search.pop();
        search
    }

    /// From one endpoint we can get both the chapter to read and the list of chapters so thats why
    /// this method exists
    /// `chapter_id` is expected to be a full url like: `https://chapmanganato.to/manga-bp1004524/chapter-20`
    /// this is because on manganato a chapter doesnt have a id per se
    /// and the `manga_id` is actually not required
    async fn get_chapter_page(&self, manga_id: &str, chapter_id: &str) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
        let response = self.client.get(chapter_id).send().await?;

        if response.status() == StatusCode::FORBIDDEN {
            return Err(format!(
                "Could not search chapter of manga on manganato with id: {manga_id} the status code is 403, set the enviroment variable MANGANATO_COOKIE so that the request is accepeted",
            )
            .into());
        }

        if response.status() != StatusCode::OK {
            return Err(format!(
                "Could not search chapter of manga on manganato with id : {manga_id}, status code : {}, {chapter_id}",
                response.status()
            )
            .into());
        }

        let doc = response.text().await?;

        let response = ChapterPageResponse::parse_html(HtmlElement::new(doc))?;

        let chapter_to_read: ChapterToRead = ChapterToRead {
            id: chapter_id.to_string(),
            title: response.title.unwrap_or("no title".to_string()),
            number: response.number.parse().unwrap(),
            volume_number: response.volume_number,
            num_page_bookmarked: None,
            language: Languages::English,
            pages_url: response.pages_url.urls.into_iter().flat_map(|raw_url| Url::parse(&raw_url)).collect(),
        };

        let list_of_chapters = ListOfChapters::from(response.chapters_list);

        Ok((chapter_to_read, list_of_chapters))
    }
}

impl ProviderIdentity for MangakakalotProvider {
    fn name(&self) -> super::MangaProviders {
        super::MangaProviders::Mangakakalot
    }
}

impl GetRawImage for MangakakalotProvider {
    async fn get_raw_image(&self, url: &str) -> Result<bytes::Bytes, Box<dyn Error>> {
        let response = self.image_client.get(url).send().await?;

        if response.status() != StatusCode::OK {
            return Err(format!("Could not get image on manganato with url : {url}").into());
        }

        Ok(response.bytes().await?)
    }
}

impl DecodeBytesToImage for MangakakalotProvider {}

impl SearchMangaPanel for MangakakalotProvider {}

impl SearchMangaById for MangakakalotProvider {
    /// `manga_id` is expected to be the url which points to the manga page
    /// example `https://manganato.com/manga-js987275`
    async fn get_manga_by_id(&self, manga_id: &str) -> Result<super::Manga, Box<dyn Error>> {
        let response = self.client.get(manga_id).send().await?;

        if response.status() == StatusCode::FORBIDDEN {
            return Err(format!(
                "Could not search chapter of manga on manganato with id: {manga_id} the status code is 403, set the enviroment variable MANGANATO_COOKIE so that the request is accepeted",
            )
            .into());
        }

        if response.status() != StatusCode::OK {
            return Err(format!("manga page with id : {manga_id} could not be found on manganato").into());
        }

        let doc = response.text().await?;

        let manga = MangaPageData::parse_html(HtmlElement::new(doc))?;

        let authors = manga.authors.map(|name| Author {
            name,
            ..Default::default()
        });

        Ok(super::Manga {
            id: manga_id.to_string(),
            id_safe_for_download: extract_id_from_url(manga_id),
            title: manga.title,
            genres: manga.genres.into_iter().map(Genres::from).collect(),
            description: manga.description,
            status: manga.status.into(),
            cover_img_url: manga.cover_url.clone(),
            languages: Self::MANGANATO_MANGA_LANGUAGE.into(),
            rating: manga.rating,
            // There is no way of knowing the artist/artists of the manga on manganato
            artist: None,
            author: authors,
        })
    }
}

impl SearchChapterById for MangakakalotProvider {
    async fn search_chapter(&self, chapter_id: &str, manga_id: &str) -> Result<super::ChapterToRead, Box<dyn Error>> {
        let chapter = self.get_chapter_page(manga_id, chapter_id).await?;
        Ok(chapter.0)
    }
}

impl HomePageMangaProvider for MangakakalotProvider {
    async fn get_popular_mangas(&self) -> Result<Vec<super::PopularManga>, Box<dyn Error>> {
        let response = self.client.get(self.base_url.clone()).send().await?;

        if response.status() != StatusCode::OK {
            return Err(format!("could not get popular mangas on manganato, details about the response : {:#?}", response).into());
        }

        let doc = response.text().await?;

        let response = GetPopularMangasResponse::parse_html(HtmlElement::new(doc))?;

        Ok(response.mangas.into_iter().map(PopularManga::from).collect())
    }

    async fn get_recently_added_mangas(&self) -> Result<Vec<super::RecentlyAddedManga>, Box<dyn Error>> {
        let response = self.client.get(self.base_url.clone()).send().await?;

        if response.status() != StatusCode::OK {
            return Err(format!("could not find recently added mangas on manganato, status code : {}", response.status()).into());
        }

        let doc = response.text().await?;

        let new_mangas = NewAddedMangas::parse_html(HtmlElement::new(doc))?;

        Ok(new_mangas.mangas.into_iter().map(RecentlyAddedManga::from).collect())
    }
}

impl SearchPageProvider for MangakakalotProvider {
    type FiltersHandler = MangakakalotFiltersProvider;
    type InnerState = MangakakalotFilterState;
    type Widget = MangakakalotFilterWidget;

    async fn search_mangas(
        &self,
        search_term: Option<SearchTerm>,
        _filters: Self::InnerState,
        pagination: super::Pagination,
    ) -> Result<super::GetMangasResponse, Box<dyn Error>> {
        let search = Self::format_search_term(search_term.ok_or("title is required to search on manganato")?);
        let endpoint = format!("{}search/story/{}", self.base_url, search);

        let response = self
            .client
            .get(endpoint.clone())
            .query(&[("page", pagination.current_page.to_string())])
            .send()
            .await?;

        if response.status() != StatusCode::OK {
            return Err(
                format!("could not search mangas on manganato, status code : {}, search : {endpoint}", response.status()).into()
            );
        }

        let doc = response.text().await?;

        let result = SearchMangaResponse::parse_html(HtmlElement::new(doc))?;

        Ok(GetMangasResponse::from(result))
    }
}

impl GoToReadChapter for MangakakalotProvider {
    async fn read_chapter(
        &self,
        chapter_id: &str,
        manga_id: &str,
    ) -> Result<(super::ChapterToRead, super::ListOfChapters), Box<dyn Error>> {
        self.get_chapter_page(manga_id, chapter_id).await
    }
}

impl GetChapterPages for MangakakalotProvider {
    /// On manganato `image_quality` has no effect because they dont provide ways to get images
    /// with lower or higher quality
    async fn get_chapter_pages_url_with_extension(
        &self,
        chapter_id: &str,
        _manga_id: &str,
        _image_quality: crate::config::ImageQuality,
    ) -> Result<Vec<super::ChapterPageUrl>, Box<dyn Error>> {
        let response = self.client.get(chapter_id).send().await?;

        if response.status() != StatusCode::OK {
            return Err(format!("could not get pages url for chapter with id : {chapter_id}").into());
        }

        let doc = response.text().await?;

        let pages = ChapterUrls::parse_html(HtmlElement::new(doc))?;

        let mut pages_url: Vec<ChapterPageUrl> = vec![];

        for page in pages.urls {
            let url = Url::parse(&page).unwrap_or("https://localhost".parse().unwrap());
            let extension = Path::new(&page).extension().unwrap().to_str().unwrap().to_string();

            pages_url.push(ChapterPageUrl { url, extension });
        }
        Ok(pages_url)
    }
}

impl FetchChapterBookmarked for MangakakalotProvider {
    async fn fetch_chapter_bookmarked(
        &self,
        chapter: crate::backend::database::ChapterBookmarked,
    ) -> Result<(super::ChapterToRead, super::ListOfChapters), Box<dyn Error>> {
        let (mut chapter_to_read, list_of_chapters) = self.get_chapter_page(&chapter.manga_id, &chapter.id).await?;
        chapter_to_read.num_page_bookmarked = chapter.number_page_bookmarked;
        Ok((chapter_to_read, list_of_chapters))
    }
}

impl ReaderPageProvider for MangakakalotProvider {}

impl MangaPageProvider for MangakakalotProvider {
    /// On manganato the chapters are not paginated, so it is important to paginate them
    /// client-side
    async fn get_chapters(
        &self,
        manga_id: &str,
        filters: super::ChapterFilters,
        pagination: super::Pagination,
    ) -> Result<super::GetChaptersResponse, Box<dyn Error>> {
        let response = self.client.get(manga_id).send().await?;

        if response.status() != StatusCode::OK {
            return Err(format!("could not find manga page for : {manga_id}").into());
        }

        let doc = response.text().await?;

        let response = ManganatoChaptersResponse::parse_html(HtmlElement::new(doc))?;

        let total_chapters = response.total_chapters;

        let mut chapters: Vec<Chapter> = response
            .chapters
            .into_iter()
            .map(|chap| Chapter {
                id: chap.page_url.clone(),
                id_safe_for_download: format!("{}-{}", extract_id_from_url(manga_id), extract_id_from_url(chap.page_url)),
                title: chap.title.unwrap_or("no title".to_string()),
                volume_number: chap.volume,
                scanlator: Some("Manganato".to_string()),
                language: Languages::English,
                chapter_number: chap.number,
                manga_id: manga_id.to_string(),
                publication_date: from_timestamp(chap.uploaded_at.parse().unwrap_or_default()).unwrap_or_default(),
            })
            .collect();

        if filters.order == ChapterOrderBy::Ascending {
            chapters.reverse();
        }

        let from = pagination.index_to_slice_from();
        let to = pagination.to_index(total_chapters as usize);

        let chapters = chapters.as_slice().get(from..to).unwrap_or(&[]);

        Ok(super::GetChaptersResponse {
            chapters: chapters.to_vec(),
            total_chapters,
        })
    }

    async fn get_all_chapters(&self, manga_id: &str, _language: Languages) -> Result<Vec<super::Chapter>, Box<dyn Error>> {
        let response = self.client.get(manga_id).send().await?;

        if response.status() != StatusCode::OK {
            return Err(format!("could not find manga page for : {manga_id}").into());
        }

        let doc = response.text().await?;

        let response = ManganatoChaptersResponse::parse_html(HtmlElement::new(doc))?;

        let chapters: Vec<Chapter> = response
            .chapters
            .into_iter()
            .map(|chap| Chapter {
                id: chap.page_url.clone(),
                id_safe_for_download: format!("{}-{}", extract_id_from_url(manga_id), extract_id_from_url(chap.page_url)),
                title: chap.title.unwrap_or("no title".to_string()),
                volume_number: chap.volume,
                scanlator: Some("Manganato".to_string()),
                language: Languages::English,
                chapter_number: chap.number,
                manga_id: manga_id.to_string(),
                publication_date: from_timestamp(chap.uploaded_at.parse().unwrap_or_default()).unwrap_or_default(),
            })
            .collect();

        Ok(chapters)
    }
}

impl FeedPageProvider for MangakakalotProvider {
    async fn get_latest_chapters(&self, manga_id: &str) -> Result<Vec<super::LatestChapter>, Box<dyn Error>> {
        let doc = self.client.get(manga_id).send().await?.text().await?;

        let data = ManganatoChaptersResponse::parse_html(HtmlElement::new(doc))?;

        Ok(data
            .chapters
            .into_iter()
            .take(4)
            .map(|chap| LatestChapter {
                id: chap.page_url,
                title: chap.title.unwrap_or("no title".to_string()),
                chapter_number: chap.number,
                manga_id: manga_id.to_string(),
                language: Languages::English,
                volume_number: chap.volume,
                publication_date: from_timestamp(chap.uploaded_at.parse().unwrap_or_default()).unwrap_or_default(),
            })
            .collect())
    }
}

impl MangaProvider for MangakakalotProvider {}

#[cfg(test)]
mod tests {

    use httpmock::Method::GET;
    use httpmock::MockServer;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::backend::manga_provider::{ChapterFilters, Manga, Pagination};
    use crate::config::ImageQuality;

    static HOME_PAGE_DOCUMENT: &str = include_str!("../../../data_test/mangakakalot/home_page.html");
    static SEARCH_DOCUMENT: &str = include_str!("../../../data_test/mangakakalot/search_page.html");
    static MANGA_PAGE_DOCUMENT: &str = include_str!("../../../data_test/mangakakalot/manga_page.html");
    static CHAPTER_PAGE_DOCUMENT: &str = include_str!("../../../data_test/mangakakalot/chapter_page.html");

    #[test]
    fn expected_manganato_endpoints() {
        assert_eq!("https://www.mangakakalot.gg", MANGAKAKALOT_BASE_URL);
    }

    #[test]
    fn parses_search_term_correctly() {
        let searchterm = SearchTerm::trimmed_lowercased("death note").unwrap();

        assert_eq!("death_note", MangakakalotProvider::format_search_term(searchterm));

        let searchterm = SearchTerm::trimmed_lowercased("oshi no ko").unwrap();

        assert_eq!("oshi_no_ko", MangakakalotProvider::format_search_term(searchterm));
    }

    #[tokio::test]
    async fn it_calls_image_endpoint() -> Result<(), Box<dyn Error>> {
        let server = MockServer::start_async().await;

        let expected = b"some image bytes";

        let request = server
            .mock_async(|when, then| {
                // The referer is important to be presented, if not when requesting a chapter page
                // it will be blocked by cloudfare
                when.method(GET)
                    .header_exists("user-agent")
                    .header("referer", "https://www.mangakakalot.gg/");

                then.status(200).body(expected.clone());
            })
            .await;

        let manganato = MangakakalotProvider::new(server.url("/manganatotest").parse().unwrap());

        let response = manganato.get_raw_image(server.base_url().as_str()).await?;

        request.assert_async().await;

        assert_eq!(expected.to_vec(), response);

        Ok(())
    }

    #[tokio::test]
    async fn it_gets_popular_manga() -> Result<(), Box<dyn Error>> {
        let server = MockServer::start_async().await;

        let request = server
            .mock_async(|when, then| {
                when.method(GET).header_exists("user-agent").path_contains("home_page");

                then.status(200).body(HOME_PAGE_DOCUMENT);
            })
            .await;

        let manganato = MangakakalotProvider::new(server.url("/home_page").parse().unwrap());

        let response = manganato.get_popular_mangas().await?;

        request.assert_async().await;

        assert!(!response.is_empty());

        assert_eq!(20, response.len());

        Ok(())
    }

    #[tokio::test]
    async fn it_searches_manga_with_search_term() -> Result<(), Box<dyn Error>> {
        let server = MockServer::start_async().await;

        let request = server
            .mock_async(|when, then| {
                when.method(GET).path_contains("search/story").query_param("page", "1");

                then.status(200).body(SEARCH_DOCUMENT);
            })
            .await;

        let manganato = MangakakalotProvider::new(server.url("/").parse().unwrap());

        let response = manganato
            .search_mangas(SearchTerm::trimmed_lowercased("oshi no ko"), MangakakalotFilterState {}, Pagination::default())
            .await?;

        request.assert_async().await;

        assert!(!response.mangas.is_empty());

        assert_eq!(7, response.mangas.len());

        assert_eq!(7, response.total_mangas);

        Ok(())
    }

    #[tokio::test]
    async fn it_gets_manga_page() -> Result<(), Box<dyn Error>> {
        let server = MockServer::start_async().await;

        let request = server
            .mock_async(|when, then| {
                when.method(GET);
                then.status(200).body(MANGA_PAGE_DOCUMENT);
            })
            .await;

        let base_url = server.url("");

        let manganato = MangakakalotProvider::new(base_url.clone().parse().unwrap());

        let response = manganato.get_manga_by_id(server.url("/manga-jq986499").as_str()).await?;

        request.assert_async().await;

        let expected: Manga = Manga {
            id: format!("{base_url}/manga-jq986499"),
            id_safe_for_download: "manga-jq986499".to_string(),
            ..Default::default()
        };

        //Only this is important to test, the rest is tested in manganato/response.rs
        assert_eq!(expected.id_safe_for_download, response.id_safe_for_download);
        assert_eq!(expected.id, response.id);

        Ok(())
    }

    #[tokio::test]
    async fn it_gets_chapters_of_manga_paginated() -> Result<(), Box<dyn Error>> {
        let server = MockServer::start_async().await;

        let request = server
            .mock_async(|when, then| {
                when.method(GET);
                then.status(200).body(MANGA_PAGE_DOCUMENT);
            })
            .await;

        let base_url = server.url("");

        let manganato = MangakakalotProvider::new(base_url.clone().parse().unwrap());

        let response = manganato
            .get_chapters(server.url("/manga-jq986499").as_str(), ChapterFilters::default(), Pagination::default())
            .await?;

        request.assert_async().await;

        assert!(!response.chapters.is_empty());

        assert_eq!(17, response.total_chapters);

        let expected: Chapter = Chapter {
            id: format!(
                "https://www.mangakakalot.gg/manga/my-level-up-is-strange-reincarnation-of-a-great-man-in-a-different-world/chapter-2"
            ),
            id_safe_for_download: "manga-jq986499-chapter-2".to_string(),
            publication_date: chrono::DateTime::from_timestamp(1722694402, 0).unwrap().date_naive(),
            ..Default::default()
        };

        let result = response.chapters.iter().last().ok_or("Expected chapter was not found")?;

        assert_eq!(expected.id, result.id);
        assert_eq!(expected.id_safe_for_download, result.id_safe_for_download);
        //assert_eq!(expected.publication_date, result.publication_date);

        Ok(())
    }

    #[tokio::test]
    async fn it_gets_all_chapters() -> Result<(), Box<dyn Error>> {
        let server = MockServer::start_async().await;

        let request = server
            .mock_async(|when, then| {
                when.method(GET);
                then.status(200).body(MANGA_PAGE_DOCUMENT);
            })
            .await;

        let base_url = server.url("");

        let manganato = MangakakalotProvider::new(base_url.clone().parse().unwrap());

        let response = manganato
            .get_all_chapters(server.url("/manga-jq986499").as_str(), Languages::default())
            .await?;

        request.assert_async().await;

        assert!(!response.is_empty());

        assert_eq!(17, response.len());

        Ok(())
    }

    #[tokio::test]
    async fn it_gets_chapter_page() -> Result<(), Box<dyn Error>> {
        let server = MockServer::start_async().await;

        let request = server
            .mock_async(|when, then| {
                when.method(GET);
                then.status(200).body(CHAPTER_PAGE_DOCUMENT);
            })
            .await;

        let base_url = server.url("");

        let manganato = MangakakalotProvider::new(base_url.clone().parse().unwrap());

        let (chapter_to_read, chapter_list) = manganato.read_chapter(&server.url("/manga-jq986499/chapter-27"), "").await?;

        request.assert_async().await;

        assert_eq!(11, chapter_to_read.pages_url.len());

        let volume = chapter_list.volumes.as_slice().last().ok_or("no volume in chapter list was found")?;

        let chapters = volume.chapters.as_slice();

        for chap in chapters {
            Url::parse(&chap.id).expect("id of chapters in chapter list must be valid urls");
        }

        Ok(())
    }

    #[tokio::test]
    async fn it_gets_chapter_page_bookmarked() -> Result<(), Box<dyn Error>> {
        let server = MockServer::start_async().await;

        let request = server
            .mock_async(|when, then| {
                when.method(GET);
                then.status(200).body(CHAPTER_PAGE_DOCUMENT);
            })
            .await;

        let base_url = server.url("");

        let manganato = MangakakalotProvider::new(base_url.clone().parse().unwrap());

        let (chapter_to_read, _) = manganato
            .fetch_chapter_bookmarked(crate::backend::database::ChapterBookmarked {
                id: server.url("/manga-jq986499/chapter-27"),
                ..Default::default()
            })
            .await?;

        request.assert_async().await;

        assert_eq!(11, chapter_to_read.pages_url.len());

        Ok(())
    }

    #[tokio::test]
    async fn it_gets_chapter_pages_with_url_and_extension() -> Result<(), Box<dyn Error>> {
        let server = MockServer::start_async().await;

        let request = server
            .mock_async(|when, then| {
                when.method(GET);
                then.status(200).body(CHAPTER_PAGE_DOCUMENT);
            })
            .await;

        let base_url = server.url("");

        let manganato = MangakakalotProvider::new(base_url.clone().parse().unwrap());

        let result = manganato
            .get_chapter_pages_url_with_extension(&server.url("/manga-jq986499/chapter-27"), "", ImageQuality::Low)
            .await?;

        request.assert_async().await;

        assert_eq!(11, result.len());

        let last_page = result.last().ok_or("no last page found")?;

        let expected: ChapterPageUrl = ChapterPageUrl {
            url: Url::parse("https://storage.waitst.com/zin/sometimes-even-reality-is-a-lie/212/10.webp")?,
            extension: "webp".to_string(),
        };

        assert_eq!(expected, *last_page);

        Ok(())
    }

    #[tokio::test]
    async fn it_gets_latest_chapters_of_manga() -> Result<(), Box<dyn Error>> {
        let server = MockServer::start_async().await;

        let request = server
            .mock_async(|when, then| {
                when.method(GET);
                then.status(200).body(MANGA_PAGE_DOCUMENT);
            })
            .await;

        let base_url = server.url("");

        let manganato = MangakakalotProvider::new(base_url.clone().parse().unwrap());

        let manga_id = server.url("/manga-jq986499");

        let result = manganato.get_latest_chapters(&manga_id).await?;

        request.assert_async().await;

        assert_eq!(4, result.len());

        Ok(())
    }
}
