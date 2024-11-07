static BASE_ANILIST_API_URL: &str = "https://graphql.anilist.co";

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use http::header::{ACCEPT, CONTENT_TYPE};
    use http::{HeaderMap, HeaderValue, StatusCode};
    use httpmock::Method::POST;
    use httpmock::MockServer;
    use manga_tui::SearchTerm;
    use pretty_assertions::{assert_eq, assert_str_eq};
    use reqwest::{Client, Url};
    use serde::{Deserialize, Serialize};
    use serde_json::json;
    use uuid::Uuid;

    use super::*;
    use crate::backend::tracker::{MangaToTrack, MangaTracker, MarkAsRead};

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

    #[derive(Debug, Deserialize, Serialize)]
    struct Manga {
        id: String,
        title: String,
    }

    #[derive(Debug, Deserialize, Serialize)]
    struct GetMangaByTitleQuery<'a> {
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
              Media (search: $search, type: MANGA) { 
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

    /// set as reading,
    /// mark chapter progress number
    /// mark start date
    /// mark volume progress as well
    #[derive(Debug, Deserialize, Serialize)]
    struct MarkMangaAsReadQuery {
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
    struct GetMangaByTitleResponse {
        data: GetMangaByTitleData,
    }

    #[derive(Debug, Deserialize, Serialize, Default)]
    struct GetMangaByTitleData {
        #[serde(rename = "Media")]
        media: GetMangaByTitleMedia,
    }

    #[derive(Debug, Deserialize, Serialize, Default)]
    struct GetMangaByTitleMedia {
        id: u32,
    }

    #[derive(Debug)]
    struct Anilist {
        base_url: Url,
        account_token: String,
        client: Client,
    }

    #[derive(Debug)]
    struct AnilistToken {
        id: String,
        secret: String,
        jwt: String,
    }

    //https://anilist.co/api/v2/oauth/authorize?client_id={client_id}&redirect_uri={redirect_uri}&response_type=code"

    impl Anilist {
        pub fn new(base_url: Url) -> Self {
            let mut default_headers = HeaderMap::new();

            default_headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            default_headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

            let client = Client::builder()
                .default_headers(default_headers)
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap();

            Self {
                base_url,
                client,
                account_token: "".to_string(),
            }
        }

        pub fn with_token(mut self, token: String) -> Self {
            self.account_token = token;
            self
        }
    }

    // it should:
    // find which manga is reading
    // find which chapter is reading
    // update which manga is reading
    // update the reading progress
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

        async fn mark_manga_as_read_with_chapter_count(&self, manga: MarkAsRead<'_>) -> Result<(), Box<dyn std::error::Error>> {
            let query =
                MarkMangaAsReadQuery::new(manga.id.parse().unwrap_or(0), manga.chapter_number, manga.volume_number.unwrap_or(0));

            self.client.post(self.base_url.clone()).body(query.into_body()).send().await?;

            Ok(())
        }
    }

    #[test]
    fn get_manga_by_title_query_is_built_as_expected() {
        let expected = json!({
            "query" : r#"
                query ($search: String) { 
                  Media (search: $search, type: MANGA) { 
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

    #[tokio::test]
    async fn anilist_searches_a_manga_by_its_title() {
        let server = MockServer::start_async().await;
        let anilist = Anilist::new(server.base_url().parse().unwrap());

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
        let anilist = Anilist::new(server.base_url().parse().unwrap());

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

    //#[tokio::test]
    //async fn anilist_get_authorization_token() {
    //    let server = MockServer::start_async().await;
    //    let token = Uuid::new_v4().to_string();
    //    let anilist = Anilist::new(server.base_url().parse().unwrap()).with_token(token);
    //}

    // Todo! include authorization
    #[tokio::test]
    async fn anilist_marks_manga_as_reading_with_chapter_and_volume_count() {
        let server = MockServer::start_async().await;
        let anilist = Anilist::new(server.base_url().parse().unwrap());

        let expected_body_sent = MarkMangaAsReadQuery::new(100, 2, 1).into_json();

        let request = server
            .mock_async(|when, then| {
                when.method(POST).json_body_obj(&expected_body_sent);
                then.status(200);
            })
            .await;

        anilist
            .mark_manga_as_read_with_chapter_count(MarkAsRead {
                id: "100",
                chapter_number: 2,
                volume_number: Some(1),
            })
            .await
            .expect("should be marked as read");

        request.assert_async().await;
    }
}
