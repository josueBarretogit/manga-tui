use super::filter::Languages;
use super::{ChapterPagesResponse, ChapterResponse, MangaStatisticsResponse, SearchMangaResponse};
use crate::backend::filter::{Filters, IntoParam};
use crate::view::pages::manga::ChapterOrder;
use bytes::Bytes;
use chrono::Months;
use once_cell::sync::OnceCell;
use reqwest::StatusCode;

#[derive(Clone, Debug)]
pub struct MangadexClient {
    client: reqwest::Client,
}

pub static MANGADEX_CLIENT_INSTANCE: OnceCell<MangadexClient> = once_cell::sync::OnceCell::new();

static API_URL_BASE: &str = "https://api.mangadex.org";
static COVER_IMG_URL_BASE: &str = "https://uploads.mangadex.org/covers";

pub static ITEMS_PER_PAGE_CHAPTERS: u32 = 16;

pub static ITEMS_PER_PAGE_LATEST_CHAPTERS: u32 = 5;

pub static ITEMS_PER_PAGE_SEARCH: u32 = 10;

impl MangadexClient {
    pub fn global() -> &'static MangadexClient {
        MANGADEX_CLIENT_INSTANCE
            .get()
            .expect("could not build mangadex client")
    }

    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    pub async fn search_mangas(
        &self,
        search_term: &str,
        page: u32,
        filters: Filters,
    ) -> Result<SearchMangaResponse, reqwest::Error> {
        let offset = (page - 1) * ITEMS_PER_PAGE_SEARCH;

        let search_by_title = if search_term.trim().is_empty() {
            "".to_string()
        } else {
            format!("title={search_term}")
        };

        let url = format!(
            "{}/manga?{}&includes[]=cover_art&includes[]=author&includes[]=artist&limit=10&offset={}{}&includedTagsMode=AND&excludedTagsMode=OR&hasAvailableChapters=true",
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
        let file_name = format!("{}.512.jpg", file_name);
        self.client
            .get(format!("{}/{}/{}", COVER_IMG_URL_BASE, id_manga, file_name))
            .send()
            .await?
            .bytes()
            .await
    }

    pub async fn get_cover_for_manga_lower_quality(
        &self,
        id_manga: &str,
        file_name: &str,
    ) -> Result<bytes::Bytes, reqwest::Error> {
        let file_name = format!("{}.256.jpg", file_name);
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

    pub async fn get_manga_chapters(
        &self,
        id: String,
        page: u32,
        language: Languages,
        order: ChapterOrder,
    ) -> Result<ChapterResponse, reqwest::Error> {
        let language = language.as_iso_code();
        let page = (page - 1) * ITEMS_PER_PAGE_CHAPTERS;

        let order = format!("order[volume]={order}&order[chapter]={order}");
        let endpoint = format!(
            "{}/manga/{}/feed?limit={ITEMS_PER_PAGE_CHAPTERS}&offset={}&{}&translatedLanguage[]={}&includes[]=scanlation_group&includeExternalUrl=0&contentRating[]=safe&contentRating[]=suggestive&contentRating[]=erotica&contentRating[]=pornographic",
            API_URL_BASE, id, page, order, language
        );

        self.client.get(endpoint).send().await?.json().await
    }

    pub async fn get_chapter_pages(
        &self,
        id: &str,
    ) -> Result<ChapterPagesResponse, reqwest::Error> {
        let endpoint = format!("{}/at-home/server/{}", API_URL_BASE, id);

        self.client.get(endpoint).send().await?.json().await
    }

    pub async fn get_manga_statistics(
        &self,
        id_manga: &str,
    ) -> Result<MangaStatisticsResponse, reqwest::Error> {
        let endpoint = format!("{}/statistics/manga/{}", API_URL_BASE, id_manga);

        self.client.get(endpoint).send().await?.json().await
    }

    pub async fn get_popular_mangas(&self) -> Result<SearchMangaResponse, reqwest::Error> {
        let current_date = chrono::offset::Local::now()
            .date_naive()
            .checked_sub_months(Months::new(1))
            .unwrap();

        let endpoint = format!("{}/manga?includes[]=cover_art&includes[]=artist&includes[]=author&order[followedCount]=desc&contentRating[]=safe&contentRating[]=suggestive&hasAvailableChapters=true&availableTranslatedLanguage[]={}&createdAtSince={}T00:00:00", API_URL_BASE, Languages::get_preferred_lang().as_iso_code(), current_date);

        self.client.get(endpoint).send().await?.json().await
    }

    pub async fn get_recently_added(&self) -> Result<SearchMangaResponse, reqwest::Error> {
        let endpoint = format!("{}/manga?limit=5&contentRating[]=safe&contentRating[]=suggestive&contentRating[]=erotica&order[createdAt]=desc&includes[]=cover_art&includes[]=artist&includes[]=author&hasAvailableChapters=true&availableTranslatedLanguage[]={}", API_URL_BASE, Languages::get_preferred_lang().as_iso_code());

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

    pub async fn get_one_manga(
        &self,
        manga_id: &str,
    ) -> Result<super::feed::OneMangaResponse, reqwest::Error> {
        let endpoint = format!(
            "{}/manga/{}?includes[]=cover_art&includes[]=author&includes[]=artist",
            API_URL_BASE, manga_id
        );
        self.client.get(endpoint).send().await?.json().await
    }

    pub async fn get_latest_chapters(
        &self,
        manga_id: &str,
    ) -> Result<ChapterResponse, reqwest::Error> {
        let endpoint = format!(
            "{}/manga/{}/feed?limit={}&includes[]=scanlation_group&offset=0&contentRating[]=safe&contentRating[]=suggestive&contentRating[]=erotica&contentRating[]=pornographic&order[readableAt]=desc",
            API_URL_BASE, manga_id, ITEMS_PER_PAGE_LATEST_CHAPTERS
        );
        self.client.get(endpoint).send().await?.json().await
    }

    pub async fn get_tags(&self) -> Result<super::tags::TagsResponse, reqwest::Error> {
        let endpoint = format!("{}/manga/tag", API_URL_BASE);

        self.client.get(endpoint).send().await?.json().await
    }

    pub async fn get_authors(
        &self,
        name: &str,
    ) -> Result<super::authors::AuthorsResponse, reqwest::Error> {
        let endpoint = format!("{}/author?name={}", API_URL_BASE, name);

        self.client.get(endpoint).send().await?.json().await
    }

    pub async fn check_status(&self) -> Result<StatusCode, reqwest::Error> {
        let endpoint = format!("{}/ping", API_URL_BASE);

        Ok(self.client.get(endpoint).send().await?.status())
    }
}
