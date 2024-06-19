use core::panic;

use reqwest::header::USER_AGENT;

use super::SearchMangaResponse;

#[derive(Clone)]
pub struct MangadexClient {
    api_url: String,
    cover_img_source: String,
    client: reqwest::Client,
}

impl MangadexClient {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            api_url: "https://api.mangadex.org".to_string(),
            cover_img_source: "https://uploads.mangadex.org/covers".to_string(),
        }
    }

    pub async fn search_mangas(
        &self,
        search_term: &str,
    ) -> Result<SearchMangaResponse, reqwest::Error> {
        let url = format!("{}/manga?title='{}'&includes[]=cover_art", self.api_url, search_term);

        let response = self.client.get(url).send().await?;

        let res: SearchMangaResponse = response.json().await?;

        Ok(res)
    }

    pub async fn get_cover_for_manga(
        &self,
        id_manga: &str,
        file_name: &str,
    ) -> Result<bytes::Bytes, reqwest::Error> {
        self.client
            .get(format!(
                "{}/{}/{}",
                self.cover_img_source, id_manga, file_name
            ))
            .send()
            .await?
            .bytes()
            .await
    }
}
