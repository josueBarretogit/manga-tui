pub struct MangadexClient {
    api_url: String,
    client: reqwest::Client,
}

impl MangadexClient {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            api_url: "https://api.mangadex.org".to_string(),
        }
    }

    pub async fn search_mangas(
        &self,
        search_term: &str,
    ) -> Result<reqwest::Response, reqwest::Error> {
        let search_mangas_response = self
            .client
            .get(format!(
                "{}manga?title='{}'&includes[]=cover_art",
                self.api_url, search_term
            ))
            .send()
            .await;
        search_mangas_response
    }
}
