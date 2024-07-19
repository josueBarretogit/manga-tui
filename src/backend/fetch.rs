use bytes::Bytes;
use chrono::Months;
use once_cell::sync::OnceCell;

use crate::filter::{Filters, IntoParam};
use crate::view::pages::manga::ChapterOrder;

use super::{
    ChapterPagesResponse, ChapterResponse, Languages, MangaStatisticsResponse, SearchMangaResponse,
};

#[derive(Clone, Debug)]
pub struct MangadexClient {
    client: reqwest::Client,
}

pub static MANGADEX_CLIENT_INSTANCE: OnceCell<MangadexClient> = once_cell::sync::OnceCell::new();

static API_URL_BASE: &str = "https://api.mangadex.org";
static COVER_IMG_URL_BASE: &str = "https://uploads.mangadex.org/covers";

impl MangadexClient {
    pub fn global() -> &'static MangadexClient {
        MANGADEX_CLIENT_INSTANCE
            .get()
            .expect("could not get mangadex client")
    }

    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    // Todo! implement more advanced filters
    pub async fn search_mangas(
        &self,
        search_term: &str,
        page: i32,
        filters: Filters,
    ) -> Result<SearchMangaResponse, reqwest::Error> {
        let content_rating =
            "contentRating[]=safe&contentRating[]=suggestive&contentRating[]=erotica";

        let offset = (page - 1) * 10;

        let search_by_title = if search_term.trim().is_empty() {
            "".to_string()
        } else {
            format!("title={search_term}")
        };

        let url = format!(
            "{}/manga?{}&includes[]=cover_art&includes[]=author&includes[]=artist&limit=10&offset={}&{}&includedTagsMode=AND&excludedTagsMode=OR",
            API_URL_BASE,
            search_by_title,
            offset,
            filters.into_param(),
        );

        self.client.get(url).send().await?.json().await
    }

    pub async fn get_cover_for_manga(
        &self,
        id_manga: &str,
        file_name: &str,
    ) -> Result<bytes::Bytes, reqwest::Error> {
        self.client
            .get(format!("{}/{}/{}", COVER_IMG_URL_BASE, id_manga, file_name))
            .send()
            .await?
            .bytes()
            .await
    }

    pub async fn get_chapter_page(
        &self,
        endpoint: &str,
        file_name: &str,
    ) -> Result<Bytes, reqwest::Error> {
        self.client
            .get(format!("{}/{}", endpoint, file_name))
            .send()
            .await?
            .bytes()
            .await
    }

    // Todo! implement filter by language and pagination
    pub async fn get_manga_chapters(
        &self,
        id: String,
        page: i32,
        language: Languages,
        order: ChapterOrder,
    ) -> Result<ChapterResponse, reqwest::Error> {
        let language: &str = language.into();
        // let page = (page - 1) * 50;

        let order = format!("order[volume]={order}&order[chapter]={order}");
        let endpoint = format!(
            "{}/manga/{}/feed?limit=50&{}&translatedLanguage[]={}&includes[]=scanlation_group&offset=0&includeExternalUrl=0",
            API_URL_BASE, id, order, language
        );

        let reponse = self.client.get(endpoint).send().await?.text().await?;
        Ok(serde_json::from_str(&reponse).unwrap_or_else(|e| panic!("{e}")))
    }

    pub async fn get_chapter_pages(
        &self,
        id: &str,
    ) -> Result<ChapterPagesResponse, reqwest::Error> {
        let endpoint = format!("{}/at-home/server/{}", API_URL_BASE, id);

        let text_response = self.client.get(endpoint).send().await?.text().await?;

        let response: ChapterPagesResponse = serde_json::from_str(&text_response).unwrap();

        Ok(response)
    }

    pub async fn get_manga_statistics(
        &self,
        id_manga: &str,
    ) -> Result<MangaStatisticsResponse, reqwest::Error> {
        let endpoint = format!("{}/statistics/manga/{}", API_URL_BASE, id_manga);

        let response = self.client.get(endpoint).send().await?.text().await;

        let data: MangaStatisticsResponse = serde_json::from_str(&response.unwrap()).unwrap();

        Ok(data)
    }

    pub async fn get_popular_mangas(&self) -> Result<SearchMangaResponse, reqwest::Error> {
        let current_date = chrono::offset::Local::now()
            .date_naive()
            .checked_sub_months(Months::new(1))
            .unwrap();

        let endpoint = format!("{}/manga?includes[]=cover_art&includes[]=artist&includes[]=author&order[followedCount]=desc&contentRating[]=safe&contentRating[]=suggestive&hasAvailableChapters=true&createdAtSince={}T00:00:00", API_URL_BASE, current_date);

        let response = self.client.get(endpoint).send().await?;

        let text = response.text().await?;

        let data: SearchMangaResponse = serde_json::from_str(&text).unwrap();

        Ok(data)
    }

    pub async fn get_recently_added(&self) -> Result<SearchMangaResponse, reqwest::Error> {
        let endpoint = format!("{}/manga?limit=5&contentRating[]=safe&contentRating[]=suggestive&contentRating[]=erotica&order[createdAt]=desc&includes[]=cover_art&includes[]=artist&includes[]=author&hasAvailableChapters=true", API_URL_BASE);

        let response = self.client.get(endpoint).send().await?;

        let data: SearchMangaResponse = response.json().await?;

        Ok(data)
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

    pub async fn get_one_manga(
        &self,
        manga_id: &str,
    ) -> Result<super::feed::OneMangaResponse, reqwest::Error> {
        let endpoint = format!(
            "{}/manga/{}?includes[]=cover_art&includes[]=author&includes[]=artist",
            API_URL_BASE, manga_id
        );

        let response = self.client.get(endpoint).send().await?.text().await?;

        let data: super::feed::OneMangaResponse = serde_json::from_str(&response).unwrap();

        Ok(data)
    }

    pub async fn get_latest_chapters(
        &self,
        manga_id: &str,
    ) -> Result<ChapterResponse, reqwest::Error> {
        let order = "order[volume]=desc&order[chapter]=desc";

        let endpoint = format!(
            "{}/manga/{}/feed?limit=3&{}&translatedLanguage[]=en&includes[]=scanlation_group&offset=0",
            API_URL_BASE, manga_id, order
        );

        let response = self.client.get(endpoint).send().await?.text().await?;

        let data: ChapterResponse = serde_json::from_str(&response).unwrap();

        Ok(data)
    }
}
