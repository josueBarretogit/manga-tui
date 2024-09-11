use std::time::Duration as StdDuration;

use bytes::Bytes;
use chrono::Months;
use manga_tui::SearchTerm;
use once_cell::sync::OnceCell;
use reqwest::{Client, Response, Url};
use serde::Serialize;
use serde_json::json;

use super::authors::AuthorsResponse;
use super::feed::OneMangaResponse;
use super::filter::Languages;
use super::tags::TagsResponse;
use super::{ChapterPagesResponse, ChapterResponse, MangaStatisticsResponse, SearchMangaResponse};
use crate::backend::filter::{Filters, IntoParam};
use crate::view::pages::manga::ChapterOrder;

pub trait ApiClient {
    async fn get_chapter_page(&self, endpoint: &str, file_name: &str) -> Result<Response, reqwest::Error>;

    async fn search_mangas(&self, search_term: Option<SearchTerm>, page: u32, filters: Filters)
    -> Result<Response, reqwest::Error>;

    async fn get_cover_for_manga(&self, id_manga: &str, file_name: &str) -> Result<Response, reqwest::Error>;

    async fn get_cover_for_manga_lower_quality(&self, id_manga: &str, file_name: &str) -> Result<Response, reqwest::Error>;

    async fn get_manga_chapters(
        &self,
        id: &str,
        page: u32,
        language: Languages,
        order: ChapterOrder,
    ) -> Result<Response, reqwest::Error>;

    async fn get_chapter_pages(&self, chapter_id: &str) -> Result<Response, reqwest::Error>;

    async fn get_manga_statistics(&self, id_manga: &str) -> Result<Response, reqwest::Error>;

    async fn get_popular_mangas(&self) -> Result<Response, reqwest::Error>;

    async fn get_recently_added(&self) -> Result<Response, reqwest::Error>;

    async fn get_one_manga(&self, manga_id: &str) -> Result<Response, reqwest::Error>;

    async fn get_latest_chapters(&self, manga_id: &str) -> Result<Response, reqwest::Error>;

    async fn get_tags(&self) -> Result<Response, reqwest::Error>;

    async fn get_authors(&self, name_to_search: SearchTerm) -> Result<Response, reqwest::Error>;

    async fn get_all_chapters_for_manga(&self, id: &str, language: Languages) -> Result<Response, reqwest::Error>;
}

#[derive(Clone, Debug)]
pub struct MockMangadexClient {}

#[derive(Clone, Debug)]
pub struct MangadexClient {
    client: reqwest::Client,
    api_url_base: Url,
    cover_img_url_base: Url,
}

pub static MANGADEX_CLIENT_INSTANCE: OnceCell<MangadexClient> = once_cell::sync::OnceCell::new();

pub static API_URL_BASE: &str = "https://api.mangadex.org";

pub static COVER_IMG_URL_BASE: &str = "https://uploads.mangadex.org/covers";

pub static ITEMS_PER_PAGE_CHAPTERS: u32 = 16;

pub static ITEMS_PER_PAGE_LATEST_CHAPTERS: u32 = 5;

pub static ITEMS_PER_PAGE_SEARCH: u32 = 10;

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

    // Not crucial this doesnt need to be tested
    pub async fn get_mangadex_image_support(&self) -> Result<Bytes, reqwest::Error> {
        self.client
            .get("https://mangadex.org/img/namicomi/support-dex-chan-1.png")
            .send()
            .await?
            .bytes()
            .await
    }

    /// Check if mangadex is available
    pub async fn check_status(&self) -> Result<Response, reqwest::Error> {
        let endpoint = format!("{}/ping", self.api_url_base);
        self.client.get(endpoint).send().await
    }
}

impl ApiClient for MangadexClient {
    async fn get_chapter_page(&self, endpoint: &str, file_name: &str) -> Result<Response, reqwest::Error> {
        self.client
            .get(format!("{endpoint}/{file_name}"))
            .timeout(StdDuration::from_secs(20))
            .send()
            .await
    }

    async fn search_mangas(
        &self,
        search_term: Option<SearchTerm>,
        page: u32,
        filters: Filters,
    ) -> Result<Response, reqwest::Error> {
        let offset = (page - 1) * ITEMS_PER_PAGE_SEARCH;

        let search_by_title = match search_term {
            Some(search) => format!("title={}", search),
            None => "".to_string(),
        };

        let filters = filters.into_param();

        let url = format!(
            "{}/manga?{search_by_title}&includes[]=cover_art&includes[]=author&includes[]=artist&limit={ITEMS_PER_PAGE_SEARCH}&offset={offset}{filters}&includedTagsMode=AND&excludedTagsMode=OR&hasAvailableChapters=true",
            self.api_url_base,
        );

        self.client.get(url).send().await
    }

    async fn get_cover_for_manga(&self, id_manga: &str, file_name: &str) -> Result<Response, reqwest::Error> {
        let file_name = format!("{file_name}.512.jpg");
        self.client
            .get(format!("{}/{id_manga}/{file_name}", self.cover_img_url_base))
            .send()
            .await
    }

    async fn get_cover_for_manga_lower_quality(&self, id_manga: &str, file_name: &str) -> Result<Response, reqwest::Error> {
        let file_name = format!("{file_name}.256.jpg");
        self.client
            .get(format!("{}/{id_manga}/{file_name}", self.cover_img_url_base))
            .send()
            .await
    }

    /// Used to get the chapters of a manga
    async fn get_manga_chapters(
        &self,
        manga_id: &str,
        page: u32,
        language: Languages,
        order: ChapterOrder,
    ) -> Result<Response, reqwest::Error> {
        let language = language.as_iso_code();
        let page = (page - 1) * ITEMS_PER_PAGE_CHAPTERS;

        let order = format!("order[volume]={order}&order[chapter]={order}");

        let endpoint = format!(
            "{}/manga/{manga_id}/feed?limit={ITEMS_PER_PAGE_CHAPTERS}&offset={page}&{order}&translatedLanguage[]={language}&includes[]=scanlation_group&includeExternalUrl=0&contentRating[]=safe&contentRating[]=suggestive&contentRating[]=erotica&contentRating[]=pornographic",
            self.api_url_base,
        );

        self.client.get(endpoint).send().await
    }

    /// Used to get the list of endpoints which provide the url to get a chapter's pages / panels
    async fn get_chapter_pages(&self, chapter_id: &str) -> Result<Response, reqwest::Error> {
        let endpoint = format!("{}/at-home/server/{chapter_id}", self.api_url_base);

        self.client.get(endpoint).send().await
    }

    /// Used in `manga` page to request the the amount of follows and stars a manga has
    async fn get_manga_statistics(&self, id_manga: &str) -> Result<Response, reqwest::Error> {
        let endpoint = format!("{}/statistics/manga/{id_manga}", self.api_url_base);

        self.client.get(endpoint).send().await
    }

    /// Used in `home` page to request the popular mangas of this month
    async fn get_popular_mangas(&self) -> Result<Response, reqwest::Error> {
        let current_date = chrono::offset::Local::now().date_naive().checked_sub_months(Months::new(1)).unwrap();
        let language = Languages::get_preferred_lang().as_iso_code();

        let endpoint = format!(
            "{}/manga?includes[]=cover_art&includes[]=artist&includes[]=author&order[followedCount]=desc&contentRating[]=safe&contentRating[]=suggestive&hasAvailableChapters=true&availableTranslatedLanguage[]={language}&createdAtSince={current_date}T00:00:00",
            self.api_url_base,
        );

        self.client.get(endpoint).send().await
    }

    /// Used in `home` page to request the most recently added mangas
    async fn get_recently_added(&self) -> Result<Response, reqwest::Error> {
        let language = Languages::get_preferred_lang().as_iso_code();
        let endpoint = format!(
            "{}/manga?limit=5&contentRating[]=safe&contentRating[]=suggestive&order[createdAt]=desc&includes[]=cover_art&includes[]=artist&includes[]=author&hasAvailableChapters=true&availableTranslatedLanguage[]={language}",
            self.api_url_base,
        );

        self.client.get(endpoint).send().await
    }

    /// Used in `feed` page to request a single manga
    async fn get_one_manga(&self, manga_id: &str) -> Result<Response, reqwest::Error> {
        let endpoint = format!("{}/manga/{manga_id}?includes[]=cover_art&includes[]=author&includes[]=artist", self.api_url_base);
        self.client.get(endpoint).send().await
    }

    /// Used in `feed` to request most recent chapters of a manga
    async fn get_latest_chapters(&self, manga_id: &str) -> Result<Response, reqwest::Error> {
        let endpoint = format!(
            "{}/manga/{manga_id}/feed?limit={ITEMS_PER_PAGE_LATEST_CHAPTERS}&includes[]=scanlation_group&offset=0&contentRating[]=safe&contentRating[]=suggestive&contentRating[]=erotica&contentRating[]=pornographic&order[readableAt]=desc",
            self.api_url_base,
        );

        self.client.get(endpoint).send().await
    }

    /// Request the tags / genres available on mangadex used in `FilterWidget`
    async fn get_tags(&self) -> Result<Response, reqwest::Error> {
        let endpoint = format!("{}/manga/tag", self.api_url_base);

        self.client.get(endpoint).send().await
    }

    /// Used in `FilterWidget` to search an author and artist
    async fn get_authors(&self, name_to_search: SearchTerm) -> Result<Response, reqwest::Error> {
        let endpoint = format!("{}/author?name={name_to_search}", self.api_url_base);

        self.client.get(endpoint).send().await
    }

    /// Used when downloading all chapters of a manga, request as much chapters as possible
    async fn get_all_chapters_for_manga(&self, manga_id: &str, language: Languages) -> Result<Response, reqwest::Error> {
        let language = language.as_iso_code();

        let order = "order[volume]=asc&order[chapter]=asc";

        let endpoint = format!(
            "{}/manga/{manga_id}/feed?limit=300&offset=0&{order}&translatedLanguage[]={language}&includes[]=scanlation_group&includeExternalUrl=0&contentRating[]=safe&contentRating[]=suggestive&contentRating[]=erotica&contentRating[]=pornographic",
            self.api_url_base
        );

        self.client.get(endpoint).timeout(StdDuration::from_secs(10)).send().await
    }
}

impl MockMangadexClient {
    pub fn new() -> Self {
        MockMangadexClient {}
    }

    pub fn mock_json_response(data: impl Serialize) -> Result<Response, reqwest::Error> {
        let mock_response = json!(data).to_string();
        Ok(http::Response::builder().body(mock_response).unwrap().into())
    }

    pub fn mock_bytes_response() -> Result<Response, reqwest::Error> {
        let image_bytes = include_bytes!("../../public/mangadex_support.jpg").to_vec();
        let response = http::Response::builder().body(image_bytes).unwrap();
        Ok(response.into())
    }
}

impl ApiClient for MockMangadexClient {
    async fn get_chapter_page(&self, _endpoint: &str, _file_name: &str) -> Result<Response, reqwest::Error> {
        Self::mock_bytes_response()
    }

    async fn search_mangas(
        &self,
        _search_term: Option<SearchTerm>,
        _page: u32,
        _filters: Filters,
    ) -> Result<Response, reqwest::Error> {
        Self::mock_json_response(SearchMangaResponse::default())
    }

    async fn get_cover_for_manga(&self, _id_manga: &str, _file_name: &str) -> Result<Response, reqwest::Error> {
        Self::mock_bytes_response()
    }

    async fn get_cover_for_manga_lower_quality(&self, _id_manga: &str, _file_name: &str) -> Result<Response, reqwest::Error> {
        Self::mock_bytes_response()
    }

    async fn get_manga_chapters(
        &self,
        _id: &str,
        _page: u32,
        _language: Languages,
        _order: ChapterOrder,
    ) -> Result<Response, reqwest::Error> {
        Self::mock_json_response(ChapterResponse::default())
    }

    async fn get_chapter_pages(&self, _chapter_id: &str) -> Result<Response, reqwest::Error> {
        Self::mock_json_response(ChapterPagesResponse::default())
    }

    async fn get_manga_statistics(&self, _id_manga: &str) -> Result<Response, reqwest::Error> {
        Self::mock_json_response(MangaStatisticsResponse::default())
    }

    async fn get_popular_mangas(&self) -> Result<Response, reqwest::Error> {
        Self::mock_json_response(SearchMangaResponse::default())
    }

    async fn get_recently_added(&self) -> Result<Response, reqwest::Error> {
        Self::mock_json_response(SearchMangaResponse::default())
    }

    async fn get_one_manga(&self, _manga_id: &str) -> Result<Response, reqwest::Error> {
        Self::mock_json_response(OneMangaResponse::default())
    }

    async fn get_latest_chapters(&self, _manga_id: &str) -> Result<Response, reqwest::Error> {
        Self::mock_json_response(ChapterResponse::default())
    }

    async fn get_tags(&self) -> Result<Response, reqwest::Error> {
        Self::mock_json_response(TagsResponse::default())
    }

    async fn get_authors(&self, _name: SearchTerm) -> Result<Response, reqwest::Error> {
        Self::mock_json_response(AuthorsResponse::default())
    }

    async fn get_all_chapters_for_manga(&self, _id: &str, _language: Languages) -> Result<Response, reqwest::Error> {
        Self::mock_json_response(ChapterResponse::default())
    }
}

#[cfg(test)]
mod test {
    use httpmock::Method::GET;
    use httpmock::MockServer;
    use pretty_assertions::assert_eq;
    use reqwest::StatusCode;

    use self::authors::AuthorsResponse;
    use self::feed::OneMangaResponse;
    use self::tags::TagsResponse;
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
            .search_mangas(SearchTerm::trimmed_lowercased("some title"), 1, Filters::default())
            .await
            .expect("an issue ocurrend when calling search_mangas");

        request.assert_async().await;

        let response = response.json().await.expect("Could not deserialize search_mangas response");

        assert_eq!(expected, response);
    }

    #[tokio::test]
    async fn get_cover_image_works() {
        let server = MockServer::start_async().await;
        let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());

        let expected = "some_image_bytes".as_bytes();
        let cover_file_name = "cover_image.png";

        let request_high_quality_cover = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path_contains("id_manga")
                    .path_contains("cover_image.png.512.jpg")
                    .header_exists("User-Agent");

                then.status(200).header("content-type", "image/jpeg").body(expected);
            })
            .await;

        let response = client
            .get_cover_for_manga("id_manga", cover_file_name)
            .await
            .expect("could not get cover for a manga");

        request_high_quality_cover.assert_async().await;

        let image_bytes = response.bytes().await.expect("could not get the bytes of the cover");

        assert_eq!(expected, image_bytes);

        let request_lower_quality_cover = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path_contains("id_manga")
                    .path_contains("cover_image.png.256.jpg")
                    .header_exists("User-Agent");

                then.status(200).header("content-type", "image/jpeg").body(expected);
            })
            .await;

        let response = client
            .get_cover_for_manga_lower_quality("id_manga", cover_file_name)
            .await
            .expect("could not get cover for a manga");

        request_lower_quality_cover.assert_async().await;

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
        let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());

        let expected = "some_page_bytes";

        let request = server
            .mock_async(|when, then| {
                when.method(GET).header_exists("User-Agent").path_contains("chapter.png");

                then.status(200).body(expected.as_bytes());
            })
            .await;

        let response = client
            .get_chapter_page(&server.base_url(), "chapter.png")
            .await
            .expect("could not send request to get chapter page");

        request.assert_async().await;

        let response = response.bytes().await.expect("could not get manga page bytes");

        assert_eq!(expected, response)
    }

    #[tokio::test]
    async fn get_manga_statistics() {
        let server = MockServer::start_async().await;
        let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());
        let id_manga = "some_id";
        let expected = MangaStatisticsResponse::default();

        let request = server
            .mock_async(|when, then| {
                when.method(GET)
                    .header_exists("User-Agent")
                    .path_contains("statistics")
                    .path_contains("manga")
                    .path_contains(id_manga);
                then.status(200).json_body_obj(&expected);
            })
            .await;

        let response = client
            .get_manga_statistics(id_manga)
            .await
            .expect("Could not send request to get manga statistics");

        request.assert_async().await;

        let response: MangaStatisticsResponse = response.json().await.expect("Could not deserialize MangaStatisticsResponse");

        assert_eq!(response, expected);
    }

    #[tokio::test]
    async fn get_popular_mangas_mangadex() {
        let server = MockServer::start_async().await;
        let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());

        let expected = SearchMangaResponse::default();

        let request = server
            .mock_async(|when, then| {
                when.method(GET)
                    .header_exists("User-Agent")
                    .path_contains("/manga")
                    .query_param("includes[]", "cover_art")
                    .query_param("includes[]", "artist")
                    .query_param("includes[]", "author")
                    .query_param("order[followedCount]", "desc")
                    .query_param("contentRating[]", "safe")
                    .query_param("contentRating[]", "suggestive")
                    .query_param("availableTranslatedLanguage[]", Languages::default().as_iso_code())
                    .query_param_exists("createdAtSince");

                then.status(200).json_body_obj(&expected);
            })
            .await;

        let response = client.get_popular_mangas().await.expect("Could not send request to get manga statistics");

        request.assert_async().await;

        let response: SearchMangaResponse = response.json().await.expect("Could not deserialize get_popular_mangas response");

        assert_eq!(response, expected);
    }

    #[tokio::test]
    async fn get_recently_added_mangadex() {
        let server = MockServer::start_async().await;
        let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());

        let expected = SearchMangaResponse::default();

        let request = server
            .mock_async(|when, then| {
                when.method(GET)
                    .header_exists("User-Agent")
                    .path_contains("/manga")
                    .query_param("includes[]", "cover_art")
                    .query_param("includes[]", "artist")
                    .query_param("includes[]", "author")
                    .query_param("limit", "5")
                    .query_param("contentRating[]", "safe")
                    .query_param("contentRating[]", "suggestive")
                    .query_param("order[createdAt]", "desc")
                    .query_param("availableTranslatedLanguage[]", Languages::default().as_iso_code());

                then.status(200).json_body_obj(&expected);
            })
            .await;

        let response = client
            .get_recently_added()
            .await
            .expect("Could not send request to get recently added mangas");

        request.assert_async().await;

        let response: SearchMangaResponse = response.json().await.expect("Could not deserialize get_recently_added response");

        assert_eq!(response, expected);
    }

    #[tokio::test]
    async fn get_one_manga_mangadex() {
        let server = MockServer::start_async().await;
        let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());

        let expected = OneMangaResponse::default();
        let manga_id = "some_id";

        let request = server
            .mock_async(|when, then| {
                when.method(GET)
                    .header_exists("User-Agent")
                    .path_contains("/manga")
                    .path_contains(manga_id)
                    .query_param("includes[]", "cover_art")
                    .query_param("includes[]", "artist")
                    .query_param("includes[]", "author");

                then.status(200).json_body_obj(&expected);
            })
            .await;

        let response = client.get_one_manga(manga_id).await.expect("Could not send request to get one manga");

        request.assert_async().await;

        let response: OneMangaResponse = response.json().await.expect("Could not deserialize get_one_manga response");

        assert_eq!(response, expected);
    }

    #[tokio::test]
    async fn get_latest_chapters_mangadex() {
        let server = MockServer::start_async().await;
        let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());

        let expected = ChapterResponse::default();
        let manga_id = "some_id";

        let request = server
            .mock_async(|when, then| {
                when.method(GET)
                    .header_exists("User-Agent")
                    .path_contains("/manga")
                    .path_contains(manga_id)
                    .path_contains("feed")
                    .query_param("limit", ITEMS_PER_PAGE_LATEST_CHAPTERS.to_string())
                    .query_param("includes[]", "scanlation_group")
                    .query_param("offset", "0")
                    .query_param("contentRating[]", "safe")
                    .query_param("contentRating[]", "suggestive")
                    .query_param("contentRating[]", "erotica")
                    .query_param("contentRating[]", "pornographic")
                    .query_param("order[readableAt]", "desc");

                then.status(200).json_body_obj(&expected);
            })
            .await;

        let response = client
            .get_latest_chapters(manga_id)
            .await
            .expect("Could not send request to get latest chapter of a manga");

        request.assert_async().await;

        let response: ChapterResponse = response.json().await.expect("Could not deserialize get_latest_chapters response");

        assert_eq!(response, expected);
    }

    #[tokio::test]
    async fn get_tags_mangadex() {
        let server = MockServer::start_async().await;
        let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());

        let expected = TagsResponse::default();

        let request = server
            .mock_async(|when, then| {
                when.method(GET).header_exists("User-Agent").path_contains("/manga").path_contains("tag");

                then.status(200).json_body_obj(&expected);
            })
            .await;

        let response = client.get_tags().await.expect("Could not send request to get mangadex tags");

        request.assert_async().await;

        let response: TagsResponse = response.json().await.expect("Could not deserialize get_tags response");

        assert_eq!(response, expected);
    }

    #[tokio::test]
    async fn search_author_and_artist_mangadex() {
        let server = MockServer::start_async().await;
        let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());

        let expected = AuthorsResponse::default();
        let search_term = "some_author";

        let request = server
            .mock_async(|when, then| {
                when.method(GET)
                    .header_exists("User-Agent")
                    .path_contains("/author")
                    .query_param("name", search_term);

                then.status(200).json_body_obj(&expected);
            })
            .await;

        let response = client
            .get_authors(SearchTerm::trimmed_lowercased(search_term).unwrap())
            .await
            .expect("Could not send request to get mangadex author / artist");

        request.assert_async().await;

        let response: AuthorsResponse = response.json().await.expect("Could not deserialize get_authors response");

        assert_eq!(response, expected);
    }

    #[tokio::test]
    async fn get_all_chapters_for_manga_mangadex() {
        let server = MockServer::start_async().await;
        let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());

        let expected = ChapterResponse::default();
        let manga_id = "some_id";
        let language = Languages::default();

        let request = server
            .mock_async(|when, then| {
                when.method(GET)
                    .header_exists("User-Agent")
                    .path_contains("/manga")
                    .path_contains("feed")
                    .path_contains(manga_id)
                    .query_param("limit", "300")
                    .query_param("offset", "0")
                    .query_param("translatedLanguage[]", language.as_iso_code())
                    .query_param("includes[]", "scanlation_group")
                    .query_param("includeExternalUrl", "0")
                    .query_param("order[volume]", "asc")
                    .query_param("order[chapter]", "asc")
                    .query_param("contentRating[]", "safe")
                    .query_param("contentRating[]", "suggestive")
                    .query_param("contentRating[]", "erotica")
                    .query_param("contentRating[]", "pornographic");

                then.status(200).json_body_obj(&expected);
            })
            .await;

        let response = client
            .get_all_chapters_for_manga(manga_id, language)
            .await
            .expect("Could not send request to get all chapters of a manga");

        request.assert_async().await;

        let response: ChapterResponse = response.json().await.expect("Could not deserialize get_all_chapters_for_manga response");

        assert_eq!(response, expected);
    }

    #[tokio::test]
    async fn check_mangadex_status() {
        let server = MockServer::start_async().await;
        let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());

        let request = server
            .mock_async(|when, then| {
                when.method(GET).header_exists("User-Agent").path_contains("/ping");

                then.status(200);
            })
            .await;

        let response = client
            .check_status()
            .await
            .expect("Could not send request to check mangadex status of a manga");

        request.assert_async().await;

        assert_eq!(response.status(), StatusCode::OK);
    }
}
