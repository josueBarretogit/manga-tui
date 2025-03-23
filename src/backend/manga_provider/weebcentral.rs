use std::error::Error;
use std::fmt::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use http::header::{ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, CACHE_CONTROL, CONNECTION, REFERER};
use http::{HeaderMap, HeaderValue, StatusCode};
use manga_tui::SearchTerm;
use reqwest::cookie::Jar;
use reqwest::{Client, Url};

use super::{
    Author, Chapter, ChapterOrderBy, ChapterPageUrl, DecodeBytesToImage, FeedPageProvider, FetchChapterBookmarked, Genres,
    GetChapterPages, GetMangasResponse, GetRawImage, GoToReadChapter, HomePageMangaProvider, Languages, LatestChapter,
    ListOfChapters, MangaPageProvider, MangaProvider, MangaProviders, PopularManga, ProviderIdentity, ReaderPageProvider,
    RecentlyAddedManga, SearchChapterById, SearchMangaById, SearchMangaPanel, SearchPageProvider,
};
use crate::backend::cache::{Cacher, InsertEntry};
use crate::backend::html_parser::{HtmlElement, ParseHtml};
use crate::backend::manga_provider::ChapterToRead;

mod response;

pub static WEEBCENTRAL_BASE_URL: &str = "https://weebcentral.com/";

//pub mod filter_state;
//pub mod filter_widget;
//pub mod response;

/// Weebcentral: `https://manganato.com`
/// Some things to keep in mind:
/// - All `ids` of manga and chapter are actually Urls
/// - The only language they provide is english,
/// - Since it is a website headers that mimic the behavior of a browser must be used, including a User agent like : `Mozilla/5.0
///   (Windows NT 10.0; Win64; x64; rv:133.0) Gecko/20100101 Firefox/133.0`
/// - There is no way of getting images with lower or higher quality, so `image_quality` doesnt apply to manganto
/// - The `Referer` header `https://chapmanganato.to` must be used to get chapter pages when reading a chapter or else cloudfare
///   blocks the request, it is not required on the rest of requests
#[derive(Clone, Debug)]
pub struct WeebcentralProvider {
    client: reqwest::Client,
    base_url: Url,
    chapter_pages_header: HeaderMap,
    cache_provider: Arc<dyn Cacher>,
}

impl WeebcentralProvider {
    const CHAPTER_PAGE_CACHE_DURATION: Duration = Duration::from_secs(30);
    const HOME_PAGE_CACHE_DURATION: Duration = Duration::from_secs(10);
    pub const MANGANATO_MANGA_LANGUAGE: &[Languages] = &[Languages::English];
    const MANGA_PAGE_CACHE_DURATION: Duration = Duration::from_secs(40);
    const SEARCH_PAGE_CACHE_DURATION: Duration = Duration::from_secs(5);

    pub fn new(base_url: Url, cache_provider: Arc<dyn Cacher>) -> Self {
        let mut default_headers = HeaderMap::new();

        default_headers.insert(REFERER, HeaderValue::from_static("https://google.com"));
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
            .cookie_provider(Arc::new(Jar::default()))
            .default_headers(default_headers)
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:133.0) Gecko/20100101 Firefox/133.0")
            .build()
            .unwrap();

        Self {
            client,
            base_url,
            cache_provider,
            chapter_pages_header,
        }
    }
}

impl ProviderIdentity for WeebcentralProvider {
    fn name(&self) -> super::MangaProviders {
        MangaProviders::Weebcentral
    }
}

impl GetRawImage for WeebcentralProvider {
    async fn get_raw_image(&self, url: &str) -> Result<bytes::Bytes, Box<dyn Error>> {
        let response = self
            .client
            .get(url)
            .timeout(Duration::from_secs(3))
            .headers(self.chapter_pages_header.clone())
            .send()
            .await?;

        if response.status() != StatusCode::OK {
            return Err(format!("Could not get image on manganato with url : {url}").into());
        }

        Ok(response.bytes().await?)
    }
}

impl DecodeBytesToImage for WeebcentralProvider {}

impl SearchMangaPanel for WeebcentralProvider {}

impl HomePageMangaProvider for WeebcentralProvider {
    async fn get_popular_mangas(&self) -> Result<Vec<super::PopularManga>, Box<dyn Error>> {
        todo!()
    }

    async fn get_recently_added_mangas(&self) -> Result<Vec<super::RecentlyAddedManga>, Box<dyn Error>> {
        todo!()
    }
}

impl SearchMangaById for WeebcentralProvider {
    async fn get_manga_by_id(&self, manga_id: &str) -> Result<super::Manga, Box<dyn Error>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {

    use httpmock::Method::GET;
    use httpmock::MockServer;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::backend::cache::mock::EmptyCache;
    use crate::backend::manga_provider::{ChapterFilters, Manga, Pagination};
    use crate::config::ImageQuality;

    static HOME_PAGE_DOCUMENT: &str = include_str!("../../../data_test/manganato/home_page.html");
    static SEARCH_DOCUMENT: &str = include_str!("../../../data_test/manganato/search.html");
    static MANGA_PAGE_DOCUMENT: &str = include_str!("../../../data_test/manganato/manga_page.html");
    static CHAPTER_PAGE_DOCUMENT: &str = include_str!("../../../data_test/manganato/chapter_page.html");

    //#[test]
    //fn expected_manganato_endpoints() {
    //    assert_eq!("https://manganato.com", MANGANATO_BASE_URL);
    //}
    //
    //#[test]
    //fn parses_search_term_correctly() {
    //    let searchterm = SearchTerm::trimmed_lowercased("death note").unwrap();
    //
    //    assert_eq!("death_note", ManganatoProvider::format_search_term(searchterm));
    //
    //    let searchterm = SearchTerm::trimmed_lowercased("oshi no ko").unwrap();
    //
    //    assert_eq!("oshi_no_ko", ManganatoProvider::format_search_term(searchterm));
    //}
    //
    //#[tokio::test]
    //async fn it_calls_image_endpoint() -> Result<(), Box<dyn Error>> {
    //    let server = MockServer::start_async().await;
    //
    //    let expected = b"some image bytes";
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            // The referer is important to be presented, if not when requesting a chapter page
    //            // it will be blocked by cloudfare
    //            when.method(GET).header_exists("user-agent").header("referer", "https://chapmanganato.to");
    //
    //            then.status(200).body(expected.clone());
    //        })
    //        .await;
    //
    //    let manganato = ManganatoProvider::new(server.url("/manganatotest").parse().unwrap(), EmptyCache::new_arc());
    //
    //    let response = manganato.get_raw_image(server.base_url().as_str()).await?;
    //
    //    request.assert_async().await;
    //
    //    assert_eq!(expected.to_vec(), response);
    //
    //    Ok(())
    //}
    //
    //#[tokio::test]
    //async fn it_gets_popular_manga() -> Result<(), Box<dyn Error>> {
    //    let server = MockServer::start_async().await;
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET).header_exists("user-agent").path_contains("home_page");
    //
    //            then.status(200).body(HOME_PAGE_DOCUMENT);
    //        })
    //        .await;
    //
    //    let manganato = ManganatoProvider::new(server.url("/home_page").parse().unwrap(), EmptyCache::new_arc());
    //
    //    let response = manganato.get_popular_mangas().await?;
    //
    //    request.assert_async().await;
    //
    //    assert!(!response.is_empty());
    //
    //    assert_eq!(25, response.len());
    //
    //    Ok(())
    //}
    //
    //#[tokio::test]
    //async fn it_searches_manga_with_search_term() -> Result<(), Box<dyn Error>> {
    //    let server = MockServer::start_async().await;
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET)
    //                .path_contains("advanced_search")
    //                .query_param("s", "all")
    //                .query_param("page", "1")
    //                .query_param("keyw", "oshi_no_ko");
    //
    //            then.status(200).body(SEARCH_DOCUMENT);
    //        })
    //        .await;
    //
    //    let manganato = ManganatoProvider::new(server.url("/").parse().unwrap(), EmptyCache::new_arc());
    //
    //    let response = manganato
    //        .search_mangas(SearchTerm::trimmed_lowercased("oshi no ko"), ManganatoFilterState {}, Pagination::default())
    //        .await?;
    //
    //    request.assert_async().await;
    //
    //    assert!(!response.mangas.is_empty());
    //
    //    assert_eq!(24, response.mangas.len());
    //
    //    assert_eq!(607, response.total_mangas);
    //
    //    Ok(())
    //}
    //
    //#[tokio::test]
    //async fn it_gets_manga_page() -> Result<(), Box<dyn Error>> {
    //    let server = MockServer::start_async().await;
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET);
    //            then.status(200).body(include_str!("../../../data_test/manganato/manga_page.html"));
    //        })
    //        .await;
    //
    //    let base_url = server.url("");
    //
    //    let manganato = ManganatoProvider::new(base_url.clone().parse().unwrap(), EmptyCache::new_arc());
    //
    //    // shikanoko nokonoko koshitantan
    //    let response = manganato.get_manga_by_id(server.url("/manga-jq986499").as_str()).await?;
    //
    //    request.assert_async().await;
    //
    //    let expected: Manga = Manga {
    //        id: format!("{base_url}/manga-jq986499"),
    //        id_safe_for_download: "manga-jq986499".to_string(),
    //        ..Default::default()
    //    };
    //
    //    //Only this is important to test, the rest is tested in manganato/response.rs
    //    assert_eq!(expected.id_safe_for_download, response.id_safe_for_download);
    //    assert_eq!(expected.id, response.id);
    //
    //    Ok(())
    //}
    //
    //#[tokio::test]
    //async fn it_gets_chapters_of_manga_paginated() -> Result<(), Box<dyn Error>> {
    //    let server = MockServer::start_async().await;
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET);
    //            then.status(200).body(MANGA_PAGE_DOCUMENT);
    //        })
    //        .await;
    //
    //    let base_url = server.url("");
    //
    //    let manganato = ManganatoProvider::new(base_url.clone().parse().unwrap(), EmptyCache::new_arc());
    //
    //    // shikanoko nokonoko koshitantan
    //    let response = manganato
    //        .get_chapters(server.url("/manga-jq986499").as_str(), ChapterFilters::default(), Pagination::default())
    //        .await?;
    //
    //    request.assert_async().await;
    //
    //    assert!(!response.chapters.is_empty());
    //
    //    assert_eq!(48, response.total_chapters);
    //
    //    let expected: Chapter = Chapter {
    //        id: format!("https://chapmanganato.to/manga-jq986499/chapter-27"),
    //        id_safe_for_download: "manga-jq986499-chapter-27".to_string(),
    //        publication_date: chrono::DateTime::from_timestamp(1722694402, 0).unwrap().date_naive(),
    //        ..Default::default()
    //    };
    //
    //    let result = response.chapters.iter().last().ok_or("Expected chapter was not found")?;
    //
    //    assert_eq!(expected.id, result.id);
    //    assert_eq!(expected.id_safe_for_download, result.id_safe_for_download);
    //    assert_eq!(expected.publication_date, result.publication_date);
    //
    //    Ok(())
    //}
    //
    //#[tokio::test]
    //async fn it_gets_all_chapters() -> Result<(), Box<dyn Error>> {
    //    let server = MockServer::start_async().await;
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET);
    //            then.status(200).body(MANGA_PAGE_DOCUMENT);
    //        })
    //        .await;
    //
    //    let base_url = server.url("");
    //
    //    let manganato = ManganatoProvider::new(base_url.clone().parse().unwrap(), EmptyCache::new_arc());
    //
    //    let response = manganato
    //        .get_all_chapters(server.url("/manga-jq986499").as_str(), Languages::default())
    //        .await?;
    //
    //    request.assert_async().await;
    //
    //    assert!(!response.is_empty());
    //
    //    assert_eq!(48, response.len());
    //
    //    Ok(())
    //}
    //
    //#[tokio::test]
    //async fn it_gets_chapter_page() -> Result<(), Box<dyn Error>> {
    //    let server = MockServer::start_async().await;
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET);
    //            then.status(200).body(CHAPTER_PAGE_DOCUMENT);
    //        })
    //        .await;
    //
    //    let base_url = server.url("");
    //
    //    let manganato = ManganatoProvider::new(base_url.clone().parse().unwrap(), EmptyCache::new_arc());
    //
    //    // shikanoko nokonoko koshitantan
    //    let (chapter_to_read, chapter_list) = manganato.read_chapter(&server.url("/manga-jq986499/chapter-27"), "").await?;
    //
    //    request.assert_async().await;
    //
    //    assert_eq!(23, chapter_to_read.pages_url.len());
    //
    //    let volume = chapter_list.volumes.as_slice().last().ok_or("no volume in chapter list was found")?;
    //
    //    let chapters = volume.chapters.as_slice();
    //
    //    for chap in chapters {
    //        Url::parse(&chap.id).expect("id of chapters in chapter list must be valid urls");
    //    }
    //
    //    Ok(())
    //}
    //
    //#[tokio::test]
    //async fn it_gets_chapter_page_bookmarked() -> Result<(), Box<dyn Error>> {
    //    let server = MockServer::start_async().await;
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET);
    //            then.status(200).body(CHAPTER_PAGE_DOCUMENT);
    //        })
    //        .await;
    //
    //    let base_url = server.url("");
    //
    //    let manganato = ManganatoProvider::new(base_url.clone().parse().unwrap(), EmptyCache::new_arc());
    //
    //    let (chapter_to_read, _) = manganato
    //        .fetch_chapter_bookmarked(crate::backend::database::ChapterBookmarked {
    //            id: server.url("/manga-jq986499/chapter-27"),
    //            ..Default::default()
    //        })
    //        .await?;
    //
    //    request.assert_async().await;
    //
    //    assert_eq!(23, chapter_to_read.pages_url.len());
    //
    //    Ok(())
    //}
    //
    //#[tokio::test]
    //async fn it_gets_chapter_pages_with_url_and_extension() -> Result<(), Box<dyn Error>> {
    //    let server = MockServer::start_async().await;
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET);
    //            then.status(200).body(CHAPTER_PAGE_DOCUMENT);
    //        })
    //        .await;
    //
    //    let base_url = server.url("");
    //
    //    let manganato = ManganatoProvider::new(base_url.clone().parse().unwrap(), EmptyCache::new_arc());
    //
    //    let result = manganato
    //        .get_chapter_pages_url_with_extension(&server.url("/manga-jq986499/chapter-27"), "", ImageQuality::Low)
    //        .await?;
    //
    //    request.assert_async().await;
    //
    //    assert_eq!(23, result.len());
    //
    //    let last_page = result.last().ok_or("no last page found")?;
    //
    //    let expected: ChapterPageUrl = ChapterPageUrl {
    //        url: Url::parse(
    //            "https://v4.mkklcdnv6tempv2.com/img/tab_24/03/51/42/jq986499/vol5_chapter_27_come_back_nokotan/23-1722675417-o.webp",
    //        )?,
    //        extension: "webp".to_string(),
    //    };
    //
    //    assert_eq!(expected, *last_page);
    //
    //    Ok(())
    //}
    //
    //#[tokio::test]
    //async fn it_gets_latest_chapters_of_manga() -> Result<(), Box<dyn Error>> {
    //    let server = MockServer::start_async().await;
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET);
    //            then.status(200).body(MANGA_PAGE_DOCUMENT);
    //        })
    //        .await;
    //
    //    let base_url = server.url("");
    //
    //    let manganato = ManganatoProvider::new(base_url.clone().parse().unwrap(), EmptyCache::new_arc());
    //
    //    let manga_id = server.url("/manga-jq986499");
    //
    //    let result = manganato.get_latest_chapters(&manga_id).await?;
    //
    //    request.assert_async().await;
    //
    //    assert_eq!(4, result.len());
    //
    //    Ok(())
    //}
}
