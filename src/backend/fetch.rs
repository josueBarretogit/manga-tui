use std::time::Duration as StdDuration;

use bytes::Bytes;
use chrono::Months;
use once_cell::sync::OnceCell;
use reqwest::{Body, Client, Method, Request, RequestBuilder, Response, StatusCode, Url};

use super::filter::Languages;
use super::{ChapterPagesResponse, ChapterResponse, MangaStatisticsResponse, SearchMangaResponse};
use crate::backend::filter::{Filters, IntoParam};
use crate::view::pages::manga::ChapterOrder;

#[derive(Clone, Debug)]
pub struct MangadexClient {
    client: reqwest::Client,
    api_url_base: Url,
    cover_img_url_base: Url,
}

#[derive(Clone, Debug)]
pub struct MockApiClient {}

impl MockApiClient {
    pub fn new() -> &'static Self {
        &MockApiClient {}
    }
}

pub trait ApiClient {
    async fn get_chapter_page(&self, endpoint: &str, file_name: &str) -> Result<Response, reqwest::Error>;
}

pub static MANGADEX_CLIENT_INSTANCE: OnceCell<MangadexClient> = once_cell::sync::OnceCell::new();

pub static API_URL_BASE: &str = "https://api.mangadex.org";

pub static COVER_IMG_URL_BASE: &str = "https://uploads.mangadex.org/covers";

pub static ITEMS_PER_PAGE_CHAPTERS: u32 = 16;

pub static ITEMS_PER_PAGE_LATEST_CHAPTERS: u32 = 5;

pub static ITEMS_PER_PAGE_SEARCH: u32 = 10;

impl ApiClient for &MangadexClient {
    async fn get_chapter_page(&self, endpoint: &str, file_name: &str) -> Result<Response, reqwest::Error> {
        self.client
            .get(format!("{}/{}", endpoint, file_name))
            .timeout(StdDuration::from_secs(20))
            .send()
            .await
    }
}

impl ApiClient for &MockApiClient {
    async fn get_chapter_page(&self, endpoint: &str, file_name: &str) -> Result<Response, reqwest::Error> {
        Client::new().get(format!("{endpoint}/{file_name}")).send().await
    }
}

impl MangadexClient {
    pub fn global() -> &'static MangadexClient {
        MANGADEX_CLIENT_INSTANCE.get().expect("could not build mangadex client")
    }

    pub fn new(api_url_base: Url, cover_img_url_base: Url) -> Self {
        let user_agent = format!(
            "manga-tui/{} ({}/{}/{})",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::FAMILY,
            std::env::consts::OS,
            std::env::consts::ARCH
        );
        let client = Client::builder()
            .timeout(StdDuration::from_secs(10))
            .user_agent(user_agent)
            .build()
            .unwrap();
        Self {
            client,
            api_url_base,
            cover_img_url_base,
        }
    }

    pub async fn search_mangas(&self, search_term: &str, page: u32, filters: Filters) -> Result<Response, reqwest::Error> {
        let offset = (page - 1) * ITEMS_PER_PAGE_SEARCH;

        let search_by_title = if search_term.trim().is_empty() { "".to_string() } else { format!("title={search_term}") };

        let url = format!(
            "{}/manga?{}&includes[]=cover_art&includes[]=author&includes[]=artist&limit=10&offset={}{}&includedTagsMode=AND&excludedTagsMode=OR&hasAvailableChapters=true",
            self.api_url_base,
            search_by_title,
            offset,
            filters.into_param(),
        );

        self.client.get(url).send().await
    }

    pub async fn get_cover_for_manga(&self, id_manga: &str, file_name: &str) -> Result<Response, reqwest::Error> {
        let file_name = format!("{}.512.jpg", file_name);
        self.client
            .get(format!("{}/{}/{}", self.cover_img_url_base, id_manga, file_name))
            .send()
            .await
    }

    pub async fn get_cover_for_manga_lower_quality(&self, id_manga: &str, file_name: &str) -> Result<bytes::Bytes, reqwest::Error> {
        let file_name = format!("{}.256.jpg", file_name);
        self.client
            .get(format!("{}/{}/{}", self.cover_img_url_base, id_manga, file_name))
            .send()
            .await?
            .bytes()
            .await
    }

    pub async fn get_manga_chapters(
        &self,
        id: &str,
        page: u32,
        language: Languages,
        order: ChapterOrder,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let language = language.as_iso_code();
        let page = (page - 1) * ITEMS_PER_PAGE_CHAPTERS;

        let order = format!("order[volume]={order}&order[chapter]={order}");
        let endpoint = format!(
            "{}/manga/{}/feed?limit={ITEMS_PER_PAGE_CHAPTERS}&offset={}&{}&translatedLanguage[]={}&includes[]=scanlation_group&includeExternalUrl=0&contentRating[]=safe&contentRating[]=suggestive&contentRating[]=erotica&contentRating[]=pornographic",
            self.api_url_base, id, page, order, language
        );

        self.client.get(endpoint).send().await
    }

    pub async fn get_chapter_pages(&self, id: &str) -> Result<Response, reqwest::Error> {
        let endpoint = format!("{}/at-home/server/{}", self.api_url_base, id);

        self.client.get(endpoint).send().await
    }

    pub async fn get_manga_statistics(&self, id_manga: &str) -> Result<MangaStatisticsResponse, reqwest::Error> {
        let endpoint = format!("{}/statistics/manga/{}", self.api_url_base, id_manga);

        self.client.get(endpoint).send().await?.json().await
    }

    pub async fn get_popular_mangas(&self) -> Result<SearchMangaResponse, reqwest::Error> {
        let current_date = chrono::offset::Local::now().date_naive().checked_sub_months(Months::new(1)).unwrap();

        let endpoint = format!(
            "{}/manga?includes[]=cover_art&includes[]=artist&includes[]=author&order[followedCount]=desc&contentRating[]=safe&contentRating[]=suggestive&hasAvailableChapters=true&availableTranslatedLanguage[]={}&createdAtSince={}T00:00:00",
            self.api_url_base,
            Languages::get_preferred_lang().as_iso_code(),
            current_date
        );

        self.client.get(endpoint).send().await?.json().await
    }

    pub async fn get_recently_added(&self) -> Result<SearchMangaResponse, reqwest::Error> {
        let endpoint = format!(
            "{}/manga?limit=5&contentRating[]=safe&contentRating[]=suggestive&contentRating[]=erotica&order[createdAt]=desc&includes[]=cover_art&includes[]=artist&includes[]=author&hasAvailableChapters=true&availableTranslatedLanguage[]={}",
            self.api_url_base,
            Languages::get_preferred_lang().as_iso_code()
        );

        self.client.get(endpoint).send().await?.json().await
    }

    // Todo! store image in this repo since it may change in the future
    pub async fn get_mangadex_image_support(&self) -> Result<Bytes, reqwest::Error> {
        self.client
            .get("https://mangadex.org/img/namicomi/support-dex-chan-1.png")
            .send()
            .await?
            .bytes()
            .await
    }

    pub async fn get_one_manga(&self, manga_id: &str) -> Result<super::feed::OneMangaResponse, reqwest::Error> {
        let endpoint = format!("{}/manga/{}?includes[]=cover_art&includes[]=author&includes[]=artist", self.api_url_base, manga_id);
        self.client.get(endpoint).send().await?.json().await
    }

    pub async fn get_latest_chapters(&self, manga_id: &str) -> Result<ChapterResponse, reqwest::Error> {
        let endpoint = format!(
            "{}/manga/{}/feed?limit={}&includes[]=scanlation_group&offset=0&contentRating[]=safe&contentRating[]=suggestive&contentRating[]=erotica&contentRating[]=pornographic&order[readableAt]=desc",
            self.api_url_base, manga_id, ITEMS_PER_PAGE_LATEST_CHAPTERS
        );
        self.client.get(endpoint).send().await?.json().await
    }

    pub async fn get_tags(&self) -> Result<super::tags::TagsResponse, reqwest::Error> {
        let endpoint = format!("{}/manga/tag", self.api_url_base);

        self.client.get(endpoint).send().await?.json().await
    }

    pub async fn get_authors(&self, name: &str) -> Result<super::authors::AuthorsResponse, reqwest::Error> {
        let endpoint = format!("{}/author?name={}", self.api_url_base, name);

        self.client.get(endpoint).send().await?.json().await
    }

    pub async fn check_status(&self) -> Result<StatusCode, reqwest::Error> {
        let endpoint = format!("{}/ping", self.api_url_base);

        Ok(self.client.get(endpoint).send().await?.status())
    }

    pub async fn get_all_chapters_for_manga(&self, id: &str, language: Languages) -> Result<ChapterResponse, reqwest::Error> {
        let language = language.as_iso_code();

        let order = "order[volume]=asc&order[chapter]=asc";

        let endpoint = format!(
            "{}/manga/{}/feed?limit=300&offset=0&{}&translatedLanguage[]={}&includes[]=scanlation_group&includeExternalUrl=0&contentRating[]=safe&contentRating[]=suggestive&contentRating[]=erotica&contentRating[]=pornographic",
            self.api_url_base, id, order, language
        );

        self.client.get(endpoint).timeout(StdDuration::from_secs(10)).send().await?.json().await
    }
}

#[cfg(test)]
mod test {

    use httpmock::Method::GET;
    use httpmock::{MockServer, Then};
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::backend::*;

    #[test]
    fn expected_mangadex_endpoints() {
        assert_eq!("https://api.mangadex.org", API_URL_BASE);
        assert_eq!("https://uploads.mangadex.org/covers", COVER_IMG_URL_BASE);
    }

    #[tokio::test]
    async fn search_mangas_mangadex_works() {
        let server = MockServer::start_async().await;

        let expected = SearchMangaResponse::default();

        let request = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path_contains("/manga")
                    .query_param("title", "some title")
                    .header_exists("User-Agent")
                    .query_param("includes[]", "cover_art")
                    .query_param("includes[]", "artist")
                    .query_param("includes[]", "author")
                    .query_param_exists("limit")
                    .query_param_exists("offset");

                then.status(200).header("content-type", "application/json").json_body_obj(&expected);
            })
            .await;

        let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());

        let response = client
            .search_mangas("some title", 1, Filters::default())
            .await
            .expect("an issue ocurrend when calling search_mangas");

        request.assert_async().await;

        let response = response.json().await.expect("Could not deserialize search_mangas response");

        assert_eq!(expected, response);
    }

    #[tokio::test]
    async fn get_cover_image_works() {
        let server = MockServer::start_async().await;

        let expected = "some_image_bytes".as_bytes();

        let request = server
            .mock_async(|when, then| {
                when.method(GET).path_contains("id_manga").header_exists("User-Agent");

                then.status(200).header("content-type", "image/jpeg").body(expected);
            })
            .await;

        let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());

        let response = client
            .get_cover_for_manga("id_manga", "cover_image.png")
            .await
            .expect("could not get cover for a manga");

        request.assert_async().await;

        let image_bytes = response.bytes().await.expect("could not get the bytes of the cover");

        assert_eq!(expected, image_bytes);
    }

    #[tokio::test]
    async fn get_manga_chapters_mangadex() {
        let server = MockServer::start_async().await;
        let expected = ChapterResponse::default();
        let default_language = Languages::default();
        let default_chapter_order = ChapterOrder::default();

        let request = server
            .mock_async(|when, then| {
                when.method(GET)
                    .header_exists("User-Agent")
                    .path_contains("id_manga")
                    .path_contains("feed")
                    .query_param("offset", "0")
                    .query_param("translatedLanguage[]", default_language.as_iso_code())
                    .query_param("order[volume]", default_chapter_order.to_string())
                    .query_param("order[chapter]", default_chapter_order.to_string());

                then.status(200).header("content-type", "application/json").json_body_obj(&expected);
            })
            .await;

        let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());

        let response = client
            .get_manga_chapters("id_manga", 1, Languages::default(), ChapterOrder::default())
            .await
            .expect("could not get manga chapters");

        request.assert_async().await;

        let response: ChapterResponse = response.json().await.expect("Could not deserialize response");

        assert_eq!(response, expected);
    }

    #[tokio::test]
    async fn get_chapter_pages_response() {
        let server = MockServer::start_async().await;
        let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());

        let expected = ChapterPagesResponse::default();

        let request = server
            .mock_async(|when, then| {
                when.method(GET)
                    .header_exists("User-Agent")
                    .path_contains("id_manga")
                    .path_contains("at-home")
                    .path_contains("server");

                then.status(200).header("content-type", "application/json").json_body_obj(&expected);
            })
            .await;

        let response = client.get_chapter_pages("id_manga").await.expect("Error calling get_chapter_pages");

        request.assert_async().await;

        let response: ChapterPagesResponse = response.json().await.expect("Could not deserialize ChapterPagesResponse");

        assert_eq!(response, expected);
    }

    #[tokio::test]
    async fn get_chapter_page() {
        let server = MockServer::start_async().await;
        let client = &MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());

        let expected = "some_page_bytes";

        let request = server
            .mock_async(|when, then| {
                when.method(GET).header_exists("User-Agent");

                then.status(200).body(expected.as_bytes());
            })
            .await;
        let response = client
            .get_chapter_page(&server.base_url(), "chapter.png")
            .await
            .expect("could not get chapter page");

        request.assert_async().await;

        //assert_eq!(expected, response)
    }
}
