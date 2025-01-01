pub static BASE_ANILIST_API_URL: &str = "https://graphql.anilist.co";
static REDIRECT_URI: &str = "https://anilist.co/api/v2/oauth/pin";
static GET_ACCESS_TOKEN_URL: &str = "https://anilist.co/api/v2/oauth/token";
//https://anilist.co/api/v2/oauth/authorize?client_id={client_id}&redirect_uri={redirect_uri}&response_type=code"

use std::error::Error;
use std::time::Duration;

use http::{HeaderMap, HeaderValue};
use manga_tui::SearchTerm;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use reqwest::{Body, Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::backend::tracker::{MangaToTrack, MangaTracker, MarkAsRead};
use crate::cli::AnilistTokenChecker;
use crate::global::USER_AGENT;

#[derive(Debug, Deserialize, Serialize)]
pub struct GetMangaByTitleQuery<'a> {
    title: &'a str,
}

/// The body that must be sent via POST request to anilist API
/// Composed of the `query` which models what to request
/// and `variables` to indicate the data that must be sent
pub trait GraphqlBody: Sized {
    fn query(&self) -> &'static str;
    fn variables(&self) -> serde_json::Value;
    fn into_json(self) -> serde_json::Value {
        json!(
            {
                "query" : self.query(),
                "variables" : self.variables()
            }
        )
    }

    fn into_body(self) -> String {
        self.into_json().to_string()
    }
}

impl<'a> GetMangaByTitleQuery<'a> {
    fn new(title: &'a str) -> Self {
        Self { title }
    }
}

impl<'a> GraphqlBody for GetMangaByTitleQuery<'a> {
    fn query(&self) -> &'static str {
        r#"
            query ($search: String) { 
              Media (search: $search, type: MANGA, sort : SEARCH_MATCH) { 
                id
              }
            }
            "#
    }

    fn variables(&self) -> serde_json::Value {
        json!({
            "search" : self.title
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MarkMangaAsReadQuery {
    id: u32,
    chapter_count: u32,
    volume_number: u32,
}

impl MarkMangaAsReadQuery {
    fn new(id: u32, chapter_count: u32, volume_number: u32) -> Self {
        Self {
            id,
            chapter_count,
            volume_number,
        }
    }
}

impl GraphqlBody for MarkMangaAsReadQuery {
    fn query(&self) -> &'static str {
        r#"
                mutation ($id: Int, $progress: Int, $progressVolumes : Int) {
                  SaveMediaListEntry(mediaId: $id, progress: $progress, progressVolumes : $progressVolumes, status: CURRENT) {
                    id
                  }
                }
            "#
    }

    fn variables(&self) -> serde_json::Value {
        json!({
            "id" : self.id,
            "progress" : self.chapter_count,
            "progressVolumes" : self.volume_number
        })
    }
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct GetMangaByTitleResponse {
    data: GetMangaByTitleData,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct GetMangaByTitleData {
    #[serde(rename = "Media")]
    media: GetMangaByTitleMedia,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct GetMangaByTitleMedia {
    id: u32,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct GetUserIdQueryResponse {
    data: GetUserIdQueryData,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct GetUserIdQueryData {
    #[serde(rename = "User")]
    user: UserId,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct UserId {
    id: u32,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct GetUserIdBody {
    id: String,
}

impl GetUserIdBody {
    pub fn new(id: String) -> Self {
        Self { id }
    }
}

impl GraphqlBody for GetUserIdBody {
    fn query(&self) -> &'static str {
        r#"
                query User($id: Int) {
                  User(id: $id) {
                    id
                  }
                }
        "#
    }

    fn variables(&self) -> serde_json::Value {
        json!({
        "id" : self.id
        })
    }
}

struct MarkMangaAsPlanToRead(u32);

impl MarkMangaAsPlanToRead {
    pub fn new(id: u32) -> Self {
        Self(id)
    }
}

impl GraphqlBody for MarkMangaAsPlanToRead {
    fn query(&self) -> &'static str {
        r#"
            mutation ($id: Int) {
              SaveMediaListEntry(
                mediaId: $id
                status: PLANNING
              ) {
                id
              }
            }
        "#
    }

    fn variables(&self) -> serde_json::Value {
        json!({ "id" : self.0 })
    }
}

#[derive(Debug, Clone)]
pub struct Anilist {
    base_url: Url,
    access_token: String,
    client_id: String,
    client: Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetAnilistAccessTokenBody {
    id: String,
    secret: String,
    code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnilistAccessTokenResponse {
    access_token: String,
}

impl GetAnilistAccessTokenBody {
    fn new(id: &str, secret: &str, code: &str) -> Self {
        Self {
            id: id.to_string(),
            secret: secret.to_string(),
            code: code.to_string(),
        }
    }
}

impl GetAnilistAccessTokenBody {
    fn into_json(self) -> serde_json::Value {
        json!({
            "grant_type": "authorization_code",
            "client_id": self.id,
            "client_secret": self.secret,
            "redirect_uri": REDIRECT_URI,
            "code": self.code,
        })
    }
}

impl From<GetAnilistAccessTokenBody> for Body {
    fn from(val: GetAnilistAccessTokenBody) -> Self {
        val.into_json().to_string().into()
    }
}

impl Anilist {
    pub fn new(base_url: Url) -> Self {
        let mut default_headers = HeaderMap::new();

        default_headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        default_headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

        let client = Client::builder()
            .default_headers(default_headers)
            .timeout(Duration::from_secs(10))
            .user_agent(&*USER_AGENT)
            .build()
            .unwrap();

        Self {
            base_url,
            client,
            client_id: String::default(),
            access_token: "".to_string(),
        }
    }

    pub fn with_token(mut self, token: String) -> Self {
        self.access_token = token;
        self
    }

    pub fn with_client_id(mut self, client_id: String) -> Self {
        self.client_id = client_id;
        self
    }

    async fn check_credentials_are_valid(&self) -> Result<bool, Box<dyn Error>> {
        let body = GetUserIdBody::new(self.client_id.clone());

        let body = body.into_body();

        let response = self
            .client
            .post(self.base_url.clone())
            .body(body)
            .header(AUTHORIZATION, self.access_token.clone())
            .send()
            .await?;

        if response.status() == StatusCode::UNAUTHORIZED || response.status() == StatusCode::BAD_REQUEST {
            return Ok(false);
        }

        Ok(true)
    }
}

impl From<GetMangaByTitleResponse> for MangaToTrack {
    fn from(value: GetMangaByTitleResponse) -> Self {
        Self {
            id: value.data.media.id.to_string(),
        }
    }
}

impl MangaTracker for Anilist {
    async fn search_manga_by_title(&self, title: SearchTerm) -> Result<Option<MangaToTrack>, Box<dyn std::error::Error>> {
        let query = GetMangaByTitleQuery::new(title.get());

        let response = self.client.post(self.base_url.clone()).body(query.into_body()).send().await?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let response: GetMangaByTitleResponse = response.json().await?;

        Ok(Some(MangaToTrack::from(response)))
    }

    async fn mark_manga_as_read_with_chapter_count(&self, manga: MarkAsRead<'_>) -> Result<(), Box<dyn Error>> {
        let query =
            MarkMangaAsReadQuery::new(manga.id.parse().unwrap_or(0), manga.chapter_number, manga.volume_number.unwrap_or(0));

        let response = self
            .client
            .post(self.base_url.clone())
            .body(query.into_body())
            .header(AUTHORIZATION, self.access_token.clone())
            .send()
            .await?;

        if response.status() != StatusCode::OK {
            return Err(
                format!("could not sync reading status with anilist, more details of the response : \n {:#?}  ", response).into()
            );
        }

        Ok(())
    }

    async fn mark_manga_as_plan_to_read(&self, manga_to_plan_to_read: super::PlanToReadArgs<'_>) -> Result<(), Box<dyn Error>> {
        let query = MarkMangaAsPlanToRead::new(manga_to_plan_to_read.id.parse()?);

        let response = self
            .client
            .post(self.base_url.clone())
            .body(query.into_body())
            .header(AUTHORIZATION, self.access_token.clone())
            .send()
            .await?;

        if response.status() != StatusCode::OK {
            return Err(format!(
                "could not mark manga as plan to read in anilist, more details of the response : \n {:#?}  ",
                response
            )
            .into());
        }

        Ok(())
    }
}

impl AnilistTokenChecker for Anilist {
    async fn verify_token(&self, _token: String) -> Result<bool, Box<dyn Error>> {
        self.check_credentials_are_valid().await
    }
}

#[cfg(test)]
mod tests {
    use httpmock::Method::POST;
    use httpmock::MockServer;
    use pretty_assertions::{assert_eq, assert_str_eq};
    use uuid::Uuid;

    use super::*;
    use crate::backend::tracker::PlanToReadArgs;

    trait RemoveWhitespace {
        /// Util trait for comparing two string without taking into account whitespaces and tabs (don't know a
        /// better, smarter way xd)
        fn remove_whitespace(&self) -> String;
    }

    impl RemoveWhitespace for serde_json::Value {
        fn remove_whitespace(&self) -> String {
            self.to_string().split_whitespace().map(|line| line.trim()).collect()
        }
    }

    #[test]
    fn get_manga_by_title_query_is_built_as_expected() {
        let expected = json!({
            "query" : r#"
                query ($search: String) { 
                  Media (search: $search, type: MANGA, sort : SEARCH_MATCH) { 
                    id
                  }
                }
            "#,
            "variables" : {
                "search" : "some_title"
            }
        });

        let query = GetMangaByTitleQuery::new("some_title");

        let as_json = query.into_json();

        assert_str_eq!(expected.get("query").unwrap().remove_whitespace(), as_json.get("query").unwrap().remove_whitespace());
        assert_eq!(expected.get("variables"), as_json.get("variables"));
    }

    #[test]
    fn get_user_id_query_is_built_as_expected() {
        let expected = json!({
            "query" : r#"
                query User($id: Int) {
                  User(id: $id) {
                    id
                  }
                }
            "#,
            "variables" : {
                "id" : 123.to_string()
            }
        });

        let query = GetUserIdBody::new("123".to_string());

        let as_json = query.into_json();

        assert_str_eq!(expected.get("query").unwrap().remove_whitespace(), as_json.get("query").unwrap().remove_whitespace());
        assert_eq!(expected.get("variables"), as_json.get("variables"));
    }

    #[tokio::test]
    async fn anilist_searches_a_manga_by_its_title() {
        let server = MockServer::start_async().await;
        let base_url: Url = server.base_url().parse().unwrap();
        let anilist = Anilist::new(base_url.clone());

        let expected_manga = MangaToTrack {
            id: "123123".to_string(),
        };

        let expected_server_response: GetMangaByTitleResponse = GetMangaByTitleResponse {
            data: GetMangaByTitleData {
                media: GetMangaByTitleMedia {
                    id: expected_manga.id.parse().unwrap(),
                },
            },
        };

        let expected_body_sent = GetMangaByTitleQuery::new("some_title").into_json();

        let request = server
            .mock_async(|when, then| {
                when.method(POST).json_body_obj(&expected_body_sent);

                then.status(200).json_body_obj(&expected_server_response);
            })
            .await;

        let response = anilist
            .search_manga_by_title(SearchTerm::trimmed_lowercased("some_title").unwrap())
            .await
            .expect("should search manga by title");

        request.assert_async().await;

        assert_eq!(expected_manga, response.expect("should not be none"))
    }

    #[tokio::test]
    async fn anilist_searches_a_manga_by_its_title_and_returns_none_if_not_found() {
        let server = MockServer::start_async().await;
        let base_url: Url = server.base_url().parse().unwrap();
        let anilist = Anilist::new(base_url.clone());

        let expected_body_sent = GetMangaByTitleQuery::new("some_title").into_json();

        let request = server
            .mock_async(|when, then| {
                when.method(POST).json_body_obj(&expected_body_sent);
                then.status(404);
            })
            .await;

        let response = anilist
            .search_manga_by_title(SearchTerm::trimmed_lowercased("some_title").unwrap())
            .await
            .expect("should search manga by title");

        request.assert_async().await;
        assert!(response.is_none())
    }

    #[test]
    fn mark_as_read_query_is_built_as_expected() {
        let expected = json!({
            "query" : r#"
                mutation ($id: Int, $progress: Int, $progressVolumes : Int) {
                  SaveMediaListEntry(mediaId: $id, progress: $progress, progressVolumes : $progressVolumes, status: CURRENT) {
                    id
                  }
                }
            "#,
            "variables" : {
                 "id" : 123,
                  "progress" : 2,
                  "progressVolumes" : 1

            }
        });

        let mark_as_read_query = MarkMangaAsReadQuery::new(123, 2, 1);

        let as_json = mark_as_read_query.into_json();

        assert_str_eq!(expected.get("query").unwrap().remove_whitespace(), as_json.get("query").unwrap().remove_whitespace());

        assert_eq!(expected.get("variables"), as_json.get("variables"));
    }

    #[test]
    fn mark_as_plan_to_read_query_is_built_as_expected() {
        let expected = json!({
            "query" : r#"
                mutation ($id: Int) {
                  SaveMediaListEntry(
                    mediaId: $id
                    status: PLANNING
                  ) {
                    id
                  }
                }
            "#,
            "variables" : {
                 "id" : 123,
            }
        });

        let mark_as_plan_to_read_query = MarkMangaAsPlanToRead::new(123);

        let as_json = mark_as_plan_to_read_query.into_json();

        assert_str_eq!(expected.get("query").unwrap().remove_whitespace(), as_json.get("query").unwrap().remove_whitespace());

        assert_eq!(expected.get("variables"), as_json.get("variables"));
    }

    #[test]
    fn get_access_token_query_is_built_correctly() {
        let expected = json!({
            "grant_type": "authorization_code",
            "client_id": "22248",
            "client_secret": "some_secret",
            "redirect_uri": "https://anilist.co/api/v2/oauth/pin",
            "code": "some_code"
        });

        let query = GetAnilistAccessTokenBody::new("22248", "some_secret", "some_code");

        assert_eq!(expected, query.into_json());
    }

    #[tokio::test]
    async fn anilist_checks_its_access_token_is_valid() {
        let server = MockServer::start_async().await;

        let token = Uuid::new_v4().to_string();
        let user_id = 123.to_string();

        let base_url: Url = server.base_url().parse().unwrap();
        let anilist = Anilist::new(base_url).with_token(token.clone()).with_client_id(user_id.clone());
        //let mut anilist = Anilist::new(BASE_ANILIST_API_URL.parse().unwrap(), GET_ACCESS_TOKEN_URL.parse().unwrap());

        let expected_body_sent = GetUserIdBody::new(user_id.clone());

        let request = server
            .mock_async(|when, then| {
                when.method(POST)
                    .header("Authorization", token)
                    .json_body_obj(&expected_body_sent.clone().into_json());
                then.status(200);
            })
            .await;

        let is_valid = anilist.check_credentials_are_valid().await.expect("should not fail");

        request.assert_async().await;

        assert!(is_valid);
    }

    #[tokio::test]
    async fn anilist_marks_manga_as_reading_with_chapter_and_volume_count() {
        let server = MockServer::start_async().await;

        let access_token = Uuid::new_v4().to_string();
        let base_url: Url = server.base_url().parse().unwrap();
        let anilist = Anilist::new(base_url.clone()).with_token(access_token.clone());
        //let anilist = Anilist::new(BASE_ANILIST_API_URL.parse().unwrap(), base_url).with_token(access_token.clone());

        let manga_id = 86635;
        let chapter = 10;
        let volume_number = 1;

        let expected_body_sent = MarkMangaAsReadQuery::new(manga_id, chapter, volume_number).into_json();

        let request = server
            .mock_async(|when, then| {
                when.method(POST).header("Authorization", access_token).json_body_obj(&expected_body_sent);
                then.status(200);
            })
            .await;

        anilist
            .mark_manga_as_read_with_chapter_count(MarkAsRead {
                id: &manga_id.to_string(),
                chapter_number: chapter,
                volume_number: Some(volume_number),
            })
            .await
            .expect("should be marked as read");

        request.assert_async().await;
    }

    #[tokio::test]
    async fn anilist_marks_manga_as_plan_to_read() {
        let server = MockServer::start_async().await;

        let access_token = Uuid::new_v4().to_string();
        let base_url: Url = server.base_url().parse().unwrap();
        let anilist = Anilist::new(base_url.clone()).with_token(access_token.clone());
        let manga_id = "86635";

        let expected_body_sent = MarkMangaAsPlanToRead::new(manga_id.parse().unwrap()).into_json();

        let request = server
            .mock_async(|when, then| {
                when.method(POST).header("Authorization", access_token).json_body_obj(&expected_body_sent);
                then.status(200);
            })
            .await;

        anilist
            .mark_manga_as_plan_to_read(PlanToReadArgs { id: &manga_id })
            .await
            .expect("should not error");

        request.assert_async().await;
    }
}
