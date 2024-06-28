use super::SearchMangaResponse;

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
            api_url_base: "https://api.mangadex.org".to_string(),
            cover_img_url_base: "https://uploads.mangadex.org/covers".to_string(),
        }
    }

    pub async fn search_mangas(
        &self,
        search_term: &str,
        page: i32,
    ) -> Result<SearchMangaResponse, reqwest::Error> {
        let url = format!(
            "{}/manga?title='{}'&includes[]=cover_art&limit=10&offset={}",
            self.api_url_base,
            search_term,
            (page - 1) * 10
        );

        self.client
            .get(url)
            .send()
            .await?
            .json::<SearchMangaResponse>()
            .await
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
}
