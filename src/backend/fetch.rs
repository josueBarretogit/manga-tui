use std::error::Error;

use color_eyre::owo_colors::style;

use super::{ChapterResponse, SearchMangaResponse};

#[derive(Clone)]
pub struct MangadexClient {
    api_url_base: String,
    cover_img_url_base: String,
    client: reqwest::Client,
}

impl MangadexClient {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            api_url_base: "https://api.mangadex.dev".to_string(),
            cover_img_url_base: "https://uploads.mangadex.dev/covers".to_string(),
        }
    }

    pub async fn search_mangas(
        &self,
        search_term: &str,
        page: i32,
    ) -> Result<SearchMangaResponse, reqwest::Error> {
        let offset = (page - 1) * 10;
        let url = format!(
            "{}/manga?title='{}'&includes[]=cover_art&limit=10&offset={}&order[relevance]=desc",
            self.api_url_base,
            search_term.trim(),
            offset,
        );

        self.client.get(url).send().await?.json().await
    }

    pub async fn get_cover_for_manga(
        &self,
        id_manga: &str,
        file_name: &str,
    ) -> Result<bytes::Bytes, reqwest::Error> {
        self.client
            .get(format!(
                "{}/{}/{}",
                self.cover_img_url_base, id_manga, file_name
            ))
            .send()
            .await?
            .bytes()
            .await
    }

    pub async fn get_manga_chapters(&self, id: String) -> Result<ChapterResponse, reqwest::Error> {
        let endpoint = format!("{}/manga/{}/feed?limit=10", self.api_url_base, id);

        let reponse = self.client.get(endpoint).send().await?.text().await?;
        Ok(serde_json::from_str(&reponse).unwrap_or_else(|e| panic!("{e}")))
    }
}
