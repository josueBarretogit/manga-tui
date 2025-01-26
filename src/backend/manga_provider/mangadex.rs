use std::error::Error;
use std::future::Future;
use std::time::Duration as StdDuration;

use api_responses::*;
use bytes::Bytes;
use chrono::Months;
use http::StatusCode;
use image::GenericImageView;
use manga_tui::SearchTerm;
use once_cell::sync::OnceCell;
use ratatui::widgets::{Block, StatefulWidget};
use reqwest::{Client, Response, Url};
use serde::Serialize;

use super::{
    Chapter, ChapterToRead, DecodeBytesToImage, EventHandler, FetchChapterBookmarked, FiltersProvider, FiltersWidget, Genres,
    GetChaptersResponse, GetMangasResponse, GetRawImage, GoToReadChapter, HomePageMangaProvider, Languages, Manga,
    MangaPageProvider, MangaPanel, MangaProvider, MangaStatus, PopularManga, Rating, ReaderPageProvider, RecentlyAddedManga,
    SearchChapterById, SearchMangaById, SearchMangaPanel, SearchPageProvider,
};
use crate::backend::database::ChapterBookmarked;
use crate::backend::tui::Events;
use crate::config::ImageQuality;
use crate::global::USER_AGENT;
use crate::view::pages::reader::ListOfChapters;
use crate::view::widgets::StatefulWidgetFrame;

pub mod api_responses;

pub mod filter;
pub mod filter_widget;

pub static MANGADEX_CLIENT_INSTANCE: OnceCell<MangadexClient> = once_cell::sync::OnceCell::new();

pub static API_URL_BASE: &str = "https://api.mangadex.org";

pub static COVER_IMG_URL_BASE: &str = "https://uploads.mangadex.org/covers";

pub static ITEMS_PER_PAGE_CHAPTERS: u32 = 16;

pub static ITEMS_PER_PAGE_LATEST_CHAPTERS: u32 = 5;

pub static ITEMS_PER_PAGE_SEARCH: u32 = 10;

// Todo! this trait should be split 💀💀
pub trait ApiClient: Clone + Send + 'static {
    fn get_cover_for_manga(&self, id_manga: &str, file_name: &str)
    -> impl Future<Output = Result<Response, reqwest::Error>> + Send;

    fn get_cover_for_manga_lower_quality(
        &self,
        id_manga: &str,
        file_name: &str,
    ) -> impl Future<Output = Result<Response, reqwest::Error>> + Send;

    fn get_chapter_pages(&self, chapter_id: &str) -> impl Future<Output = Result<Response, reqwest::Error>> + Send;

    fn get_one_manga(&self, manga_id: &str) -> impl Future<Output = Result<Response, reqwest::Error>> + Send;

    fn get_latest_chapters(&self, manga_id: &str) -> impl Future<Output = Result<Response, reqwest::Error>> + Send;

    fn get_tags(&self) -> impl Future<Output = Result<Response, reqwest::Error>> + Send;

    fn get_authors(&self, name_to_search: SearchTerm) -> impl Future<Output = Result<Response, reqwest::Error>> + Send;

    fn get_all_chapters_for_manga(
        &self,
        id: &str,
        language: Languages,
    ) -> impl Future<Output = Result<Response, reqwest::Error>> + Send;
}

#[derive(Clone, Debug)]
pub struct MangadexClient {
    client: reqwest::Client,
    api_url_base: Url,
    cover_img_url_base: Url,
    image_quality: ImageQuality,
}

impl GetRawImage for MangadexClient {
    async fn get_raw_image(&self, url: &str) -> Result<Bytes, Box<dyn Error>> {
        let response = self.client.get(url).send().await?;

        if response.status() != StatusCode::OK {
            return Err(format!("Could not get image with url : {url}").into());
        }

        Ok(response.bytes().await?)
    }
}

impl DecodeBytesToImage for MangadexClient {}

impl HomePageMangaProvider for MangadexClient {
    async fn get_recently_added_mangas(&self) -> Result<Vec<RecentlyAddedManga>, Box<dyn Error>> {
        let language = Languages::get_preferred_lang().as_iso_code();
        let endpoint = format!(
            "{}/manga?limit=5&contentRating[]=safe&contentRating[]=suggestive&order[createdAt]=desc&includes[]=cover_art&includes[]=artist&includes[]=author&hasAvailableChapters=true&availableTranslatedLanguage[]={language}",
            self.api_url_base,
        );

        let response: SearchMangaResponse = self.client.get(endpoint).send().await?.json().await?;

        Ok(response
            .data
            .into_iter()
            .map(|manga| {
                let mut cover_img_url: Option<String> = Option::default();

                for rel in &manga.relationships {
                    if let Some(attributes) = &rel.attributes {
                        match rel.type_field.as_str() {
                            "cover_art" => {
                                let file_name = attributes.file_name.as_ref().unwrap().to_string();
                                cover_img_url = Some(self.make_cover_img_url_lower_quality(&manga.id, &file_name));
                            },
                            _ => {},
                        }
                    }
                }
                RecentlyAddedManga {
                    id: manga.id,
                    title: manga.attributes.title.en.unwrap_or_default(),
                    description: manga.attributes.description.map(|desc| desc.en.unwrap_or_default()).unwrap_or_default(),
                    cover_img_url,
                }
            })
            .collect())
    }

    async fn get_popular_mangas(&self) -> Result<Vec<PopularManga>, Box<dyn Error>> {
        let current_date = chrono::offset::Local::now().date_naive().checked_sub_months(Months::new(1)).unwrap();
        let language = Languages::get_preferred_lang().as_iso_code();

        let endpoint = format!(
            "{}/manga?includes[]=cover_art&includes[]=artist&includes[]=author&order[followedCount]=desc&contentRating[]=safe&contentRating[]=suggestive&hasAvailableChapters=true&availableTranslatedLanguage[]={language}&createdAtSince={current_date}T00:00:00",
            self.api_url_base,
        );

        let response: SearchMangaResponse = self.client.get(endpoint).send().await?.json().await?;

        Ok(response
            .data
            .into_iter()
            .map(|manga| {
                let mut cover_img_url: Option<String> = Option::default();

                for rel in &manga.relationships {
                    if let Some(attributes) = &rel.attributes {
                        match rel.type_field.as_str() {
                            "cover_art" => {
                                let file_name = attributes.file_name.as_ref().unwrap().to_string();
                                cover_img_url = Some(self.make_cover_img_url(&manga.id, &file_name));
                            },
                            _ => {},
                        }
                    }
                }

                let status = match manga.attributes.status.as_str() {
                    "ongoing" => MangaStatus::Ongoing,
                    "hiatus" => MangaStatus::Hiatus,
                    "completed" => MangaStatus::Completed,
                    "cancelled" => MangaStatus::Cancelled,
                    _ => MangaStatus::default(),
                };

                let mut genres: Vec<Genres> = manga
                    .attributes
                    .tags
                    .into_iter()
                    .map(|tag| {
                        let rating = match tag.attributes.name.en.as_str() {
                            "sexual violence" | "gore" => Rating::Nsfw,
                            _ => Rating::default(),
                        };
                        Genres::new(tag.attributes.name.en, rating)
                    })
                    .collect();

                let content_rating = match manga.attributes.content_rating.as_str() {
                    "suggestive" => Rating::Moderate,
                    "pornographic" | "erotica" => Rating::Nsfw,
                    _ => Rating::Normal,
                };

                genres.push(Genres::new(manga.attributes.content_rating, content_rating));

                PopularManga {
                    id: manga.id,
                    description: manga.attributes.description.map(|desc| desc.en.unwrap_or_default()).unwrap_or_default(),
                    genres,
                    title: manga.attributes.title.en.unwrap_or_default(),
                    status,
                    cover_img_url,
                }
            })
            .collect())
    }
}

impl SearchMangaById for MangadexClient {
    async fn get_manga_by_id(&self, manga_id: &str) -> Result<super::Manga, Box<dyn Error>> {
        let response = self
            .client
            .get(format!("{}/manga/{manga_id}", self.api_url_base))
            .query(&[("includes[]", "cover_art"), ("includes[]", "author"), ("includes[]", "artist")])
            .send()
            .await?;

        if response.status() != StatusCode::OK {
            return Err(format!("failed to get manga of id {manga_id}").into());
        }

        let manga: GetMangaByIdResponse = response.json().await?;

        let id = manga.data.id;

        let title = manga.data.attributes.title.en.unwrap_or_default();

        let genres: Vec<Genres> = manga
            .data
            .attributes
            .tags
            .into_iter()
            .map(|tag| Genres::new(tag.attributes.name.en, Rating::default()))
            .collect();

        let description = manga
            .data
            .attributes
            .description
            .map(|desc| desc.en.unwrap_or_default())
            .unwrap_or_default();

        let status = match manga.data.attributes.status.as_str() {
            "ongoing" => MangaStatus::Ongoing,
            _ => MangaStatus::default(),
        };

        let mut cover_img_url: Option<String> = Option::default();
        let mut cover_img_url_lower_quality: Option<String> = Option::default();

        for rel in &manga.data.relationships {
            if let Some(attributes) = &rel.attributes {
                match rel.type_field.as_str() {
                    "cover_art" => {
                        let file_name = attributes.file_name.as_ref().unwrap().to_string();
                        cover_img_url = Some(self.make_cover_img_url(&id, &file_name));
                        cover_img_url_lower_quality = Some(self.make_cover_img_url_lower_quality(&id, &file_name));
                    },
                    "author" => {},
                    _ => {},
                }
            }
        }

        let languages: Vec<Languages> = manga
            .data
            .attributes
            .available_translated_languages
            .into_iter()
            .flatten()
            .flat_map(|lang| Languages::try_from_iso_code(&lang))
            .collect();

        let rating = self.get_manga_statistics(&id).await.ok().unwrap_or_default();
        let rating = rating.statistics.get(&id).cloned().unwrap_or_default().rating.average.unwrap_or_default();

        Ok(super::Manga {
            rating,
            id,
            title,
            genres,
            description,
            status,
            cover_img_url,
            cover_img_url_lower_quality,
            languages,
            artist: None,
            author: None,
        })
    }
}

impl GoToReadChapter for MangadexClient {
    async fn read_chapter(&self, chapter_id: &str, manga_id: &str) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
        let chapter = self.search_chapter(chapter_id, manga_id).await?;

        let list_of_chapters = self.search_list_of_chapters(manga_id, chapter.language).await?;

        Ok((chapter, list_of_chapters.into()))
    }
}

impl MangaPageProvider for MangadexClient {
    async fn get_chapters(
        &self,
        manga_id: &str,
        filters: super::ChapterFilters,
        pagination: super::Pagination,
    ) -> Result<GetChaptersResponse, Box<dyn Error>> {
        let endpoint = format!("{}/manga/{manga_id}/feed", self.api_url_base);

        let offset = (pagination.current_page.saturating_sub(1)) * pagination.items_per_page;

        let order = match filters.order {
            super::ChapterOrderBy::Ascending => "asc",
            super::ChapterOrderBy::Descending => "desc",
        };

        let response = self
            .client
            .get(endpoint)
            .query(&[
                ("includes[]", "scanlation_group"),
                ("limit", &pagination.items_per_page.to_string()),
                ("offset", &offset.to_string()),
                ("order", &order),
                ("contentRating[]", "safe"),
                ("contentRating[]", "suggestive"),
                ("contentRating[]", "erotica"),
                ("contentRating[]", "pornographic"),
                ("order[volume]", order),
                ("order[chapter]", order),
                ("translatedLanguage[]", filters.language.as_iso_code()),
            ])
            .send()
            .await?;

        if response.status() != StatusCode::OK {
            return Err(format!("Could not get chapters for manga with id : {manga_id}").into());
        }

        let response: ChapterResponse = response.json().await?;

        let total_items = response.total;

        let response: Vec<super::Chapter> = response
            .data
            .into_iter()
            .map(|chapter| {
                let id = chapter.id;
                let manga_id = manga_id.to_string();
                let title = chapter.attributes.title.unwrap_or("No title".to_string());
                let language = Languages::try_from_iso_code(&chapter.attributes.translated_language).unwrap_or_default();
                let chapter_number = chapter.attributes.chapter.unwrap_or("0".to_string());
                let volume_number = chapter.attributes.volume;

                let scanlator = chapter
                    .relationships
                    .iter()
                    .find(|rel| rel.type_field == "scanlation_group")
                    .map(|rel| rel.attributes.as_ref().unwrap().name.to_string());

                let publication_date = chapter.attributes.readable_at;

                Chapter {
                    id,
                    manga_id,
                    title,
                    language,
                    chapter_number,
                    volume_number,
                    scanlator,
                    publication_date,
                }
            })
            .collect();

        Ok(GetChaptersResponse {
            chapters: response,
            total_chapters: total_items as u32,
        })
    }
}

impl MangadexClient {
    pub fn global() -> &'static MangadexClient {
        MANGADEX_CLIENT_INSTANCE.get().expect("could not build mangadex client")
    }

    fn make_cover_img_url_lower_quality(&self, manga_id: &str, file_name: &str) -> String {
        let file_name = format!("{}/{manga_id}/{file_name}.256.jpg", self.cover_img_url_base);
        file_name
    }

    fn make_cover_img_url(&self, manga_id: &str, file_name: &str) -> String {
        let file_name = format!("{}/{manga_id}/{file_name}.512.jpg", self.cover_img_url_base);
        file_name
    }

    pub fn new(api_url_base: Url, cover_img_url_base: Url) -> Self {
        let client = Client::builder()
            .timeout(StdDuration::from_secs(10))
            .user_agent(&*USER_AGENT)
            .build()
            .unwrap();

        Self {
            client,
            api_url_base,
            cover_img_url_base,
            image_quality: ImageQuality::default(),
        }
    }

    pub fn with_image_quality(mut self, image_quality: ImageQuality) -> Self {
        self.image_quality = image_quality;
        self
    }

    /// Check if mangadex is available
    pub async fn check_status(&self) -> Result<Response, reqwest::Error> {
        let endpoint = format!("{}/ping", self.api_url_base);
        self.client.get(endpoint).send().await
    }

    pub async fn search_list_of_chapters(
        &self,
        manga_id: &str,
        language: Languages,
    ) -> Result<AggregateChapterResponse, reqwest::Error> {
        let endpoint =
            format!("{}/manga/{}/aggregate?translatedLanguage[]={}", self.api_url_base, manga_id, language.as_iso_code());
        let response: AggregateChapterResponse = self.client.get(endpoint).send().await?.json().await?;

        Ok(response)
    }

    /// Used in `manga` page to request the the amount of follows and stars a manga has
    async fn get_manga_statistics(&self, id_manga: &str) -> Result<MangaStatisticsResponse, reqwest::Error> {
        let endpoint = format!("{}/statistics/manga/{id_manga}", self.api_url_base);

        let response: MangaStatisticsResponse =
            self.client.get(endpoint).timeout(StdDuration::from_secs(5)).send().await?.json().await?;

        Ok(response)
    }
}

impl ApiClient for MangadexClient {
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

    /// Used to get the list of endpoints which provide the url to get a chapter's pages / panels
    async fn get_chapter_pages(&self, chapter_id: &str) -> Result<Response, reqwest::Error> {
        let endpoint = format!("{}/at-home/server/{chapter_id}", self.api_url_base);

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

impl SearchChapterById for MangadexClient {
    async fn search_chapter(&self, chapter_id: &str, _manga_id: &str) -> Result<ChapterToRead, Box<dyn std::error::Error>> {
        let endpoint = format!("{}/chapter/{chapter_id}", self.api_url_base);
        let response: OneChapterResponse = self.client.get(endpoint).send().await?.json().await?;
        let pages_response: ChapterPagesResponse = self.get_chapter_pages(chapter_id).await?.json().await?;

        let language = Languages::try_from_iso_code(response.data.attributes.translated_language.as_str()).unwrap_or_default();

        Ok(ChapterToRead {
            id: response.data.id,
            title: response.data.attributes.title.unwrap_or("No title".to_string()),
            number: response
                .data
                .attributes
                .chapter
                .map(|num| num.parse().unwrap_or_default())
                .unwrap_or_default(),
            volume_number: response.data.attributes.volume,
            num_page_bookmarked: None,
            language,
            pages_url: pages_response.get_files_based_on_quality_as_url(self.image_quality),
        })
    }
}

impl SearchMangaPanel for MangadexClient {
    async fn search_manga_panel(&self, endpoint: Url) -> Result<MangaPanel, Box<dyn Error>> {
        let response = self.get_image(endpoint.as_str()).await?;

        let dimensions = response.dimensions();

        Ok(MangaPanel {
            image_decoded: response,
            dimensions,
        })
    }
}

impl FetchChapterBookmarked for MangadexClient {
    async fn fetch_chapter_bookmarked(
        &self,
        chapter: ChapterBookmarked,
    ) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
        let chapter_found = self.search_chapter(&chapter.id, &chapter.manga_id).await?;
        let pages_response: ChapterPagesResponse = self.get_chapter_pages(&chapter.id).await?.json().await?;

        let list_of_chapters: AggregateChapterResponse =
            self.search_list_of_chapters(&chapter.manga_id, chapter_found.language).await?;

        let chapter_to_read: ChapterToRead = ChapterToRead {
            id: chapter.id,
            title: chapter_found.title,
            number: chapter_found.number,
            volume_number: chapter_found.volume_number,
            num_page_bookmarked: chapter.number_page_bookmarked,
            language: chapter_found.language,
            pages_url: pages_response.get_files_based_on_quality_as_url(self.image_quality),
        };

        Ok((chapter_to_read, ListOfChapters::from(list_of_chapters)))
    }
}

impl ReaderPageProvider for MangadexClient {}

#[derive(Debug, Clone)]
pub struct MangadexFiltersWidget {}

#[derive(Debug, Clone)]
pub struct MangadexFiltersState {}

impl EventHandler for MangadexFiltersState {
    fn handle_events(&mut self, events: Events) {
        match events {
            Events::Key(key) => {},
            _ => {},
        }
    }
}

impl FiltersProvider for MangadexFiltersState {
    fn is_open(&self) -> bool {
        false
    }

    fn toggle(&mut self) {}

    fn is_typing(&self) -> bool {
        false
    }
}
impl StatefulWidgetFrame for MangadexFiltersWidget {
    type State = MangadexFiltersState;

    fn render(&mut self, area: ratatui::prelude::Rect, frame: &mut ratatui::Frame<'_>, state: &mut Self::State) {}
}

impl FiltersWidget for MangadexFiltersWidget {
    type FilterState = MangadexFiltersState;
}

impl SearchPageProvider for MangadexClient {
    type FiltersState = MangadexFiltersState;
    type Widget = MangadexFiltersWidget;

    async fn search_mangas(
        &self,
        search_term: Option<SearchTerm>,
        filters: Self::FiltersState,
        pagination: super::Pagination,
    ) -> Result<GetMangasResponse, Box<dyn Error>> {
        let offset = (pagination.current_page - 1) * pagination.items_per_page;

        let endpoint = format!("{}/manga", self.api_url_base);

        let mut search = String::default();

        if let Some(sea) = search_term {
            search = sea.get().to_string()
        }

        let response: SearchMangaResponse = self
            .client
            .get(endpoint)
            .query(&[
                ("includes[]", "cover_art"),
                ("includes[]", "author"),
                ("includes[]", "artist"),
                ("limit", &pagination.items_per_page.to_string()),
                ("offset", &offset.to_string()),
                ("hasAvailableChapters", "true"),
                ("includedTagsMode", "AND"),
                if search.is_empty() { ("", "") } else { ("title", &search) },
            ])
            .send()
            .await?
            .json()
            .await?;

        let total_mangas = response.total;

        let mangas: Vec<super::SearchManga> = response
            .data
            .into_iter()
            .map(|manga| {
                let id = manga.id;

                let title = manga.attributes.title.en.unwrap_or_default();

                let genres: Vec<Genres> = manga
                    .attributes
                    .tags
                    .into_iter()
                    .map(|tag| Genres::new(tag.attributes.name.en, Rating::default()))
                    .collect();

                let description = manga.attributes.description.map(|desc| desc.en.unwrap_or_default()).unwrap_or_default();

                let status = match manga.attributes.status.as_str() {
                    "ongoing" => MangaStatus::Ongoing,
                    _ => MangaStatus::default(),
                };

                let mut cover_img_url: Option<String> = Option::default();

                for rel in &manga.relationships {
                    if let Some(attributes) = &rel.attributes {
                        match rel.type_field.as_str() {
                            "cover_art" => {
                                let file_name = attributes.file_name.as_ref().unwrap().to_string();
                                cover_img_url = Some(self.make_cover_img_url_lower_quality(&id, &file_name));
                            },
                            "author" => {},
                            _ => {},
                        }
                    }
                }

                let languages: Vec<Languages> = manga
                    .attributes
                    .available_translated_languages
                    .into_iter()
                    .flatten()
                    .flat_map(|lang| Languages::try_from_iso_code(&lang))
                    .collect();

                super::SearchManga {
                    id,
                    title,
                    description: Some(description),
                    genres,
                    artist: None,
                    author: None,
                    cover_img_url,
                    languages,
                    status: Some(status),
                }
            })
            .collect();

        Ok(GetMangasResponse {
            mangas,
            total_mangas,
        })
    }
}

impl MangaProvider for MangadexClient {}

#[cfg(test)]
mod test {
    //use httpmock::Method::GET;
    //use httpmock::MockServer;
    //use pretty_assertions::assert_eq;
    //use reqwest::StatusCode;
    //use uuid::Uuid;
    //
    //use self::api_responses::authors::AuthorsResponse;
    //use self::api_responses::feed::OneMangaResponse;
    //use self::api_responses::tags::TagsResponse;
    //use self::api_responses::{
    //    AggregateChapterResponse, ChapterPagesResponse, ChapterResponse, MangaStatisticsResponse, OneChapterResponse,
    //    SearchMangaResponse,
    //};
    //use super::*;
    //use crate::backend::*;
    //
    //#[test]
    //fn expected_mangadex_endpoints() {
    //    assert_eq!("https://api.mangadex.org", API_URL_BASE);
    //    assert_eq!("https://uploads.mangadex.org/covers", COVER_IMG_URL_BASE);
    //}
    //
    //#[tokio::test]
    //async fn search_mangas_mangadex_works() {
    //    let server = MockServer::start_async().await;
    //
    //    let expected = SearchMangaResponse::default();
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET)
    //                .path_contains("/manga")
    //                .query_param("title", "some title")
    //                .header_exists("User-Agent")
    //                .query_param("includes[]", "cover_art")
    //                .query_param("includes[]", "artist")
    //                .query_param("includes[]", "author")
    //                .query_param_exists("limit")
    //                .query_param_exists("offset");
    //
    //            then.status(200).header("content-type", "application/json").json_body_obj(&expected);
    //        })
    //        .await;
    //
    //    let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());
    //
    //    let response = client
    //        .search_mangas(SearchTerm::trimmed_lowercased("some title"), 1, Filters::default())
    //        .await
    //        .expect("an issue ocurrend when calling search_mangas");
    //
    //    request.assert_async().await;
    //
    //    let response = response.json().await.expect("Could not deserialize search_mangas response");
    //
    //    assert_eq!(expected, response);
    //}
    //
    //#[tokio::test]
    //async fn get_cover_image_works() {
    //    let server = MockServer::start_async().await;
    //    let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());
    //
    //    let expected = "some_image_bytes".as_bytes();
    //    let cover_file_name = "cover_image.png";
    //
    //    let request_high_quality_cover = server
    //        .mock_async(|when, then| {
    //            when.method(GET)
    //                .path_contains("id_manga")
    //                .path_contains("cover_image.png.512.jpg")
    //                .header_exists("User-Agent");
    //
    //            then.status(200).header("content-type", "image/jpeg").body(expected);
    //        })
    //        .await;
    //
    //    let response = client
    //        .get_cover_for_manga("id_manga", cover_file_name)
    //        .await
    //        .expect("could not get cover for a manga");
    //
    //    request_high_quality_cover.assert_async().await;
    //
    //    let image_bytes = response.bytes().await.expect("could not get the bytes of the cover");
    //
    //    assert_eq!(expected, image_bytes);
    //
    //    let request_lower_quality_cover = server
    //        .mock_async(|when, then| {
    //            when.method(GET)
    //                .path_contains("id_manga")
    //                .path_contains("cover_image.png.256.jpg")
    //                .header_exists("User-Agent");
    //
    //            then.status(200).header("content-type", "image/jpeg").body(expected);
    //        })
    //        .await;
    //
    //    let response = client
    //        .get_cover_for_manga_lower_quality("id_manga", cover_file_name)
    //        .await
    //        .expect("could not get cover for a manga");
    //
    //    request_lower_quality_cover.assert_async().await;
    //
    //    let image_bytes = response.bytes().await.expect("could not get the bytes of the cover");
    //    assert_eq!(expected, image_bytes);
    //}
    //
    //#[tokio::test]
    //async fn get_manga_chapters_mangadex() {
    //    let server = MockServer::start_async().await;
    //    let expected = ChapterResponse::default();
    //    let default_language = Languages::default();
    //    let default_chapter_order = ChapterOrder::default();
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET)
    //                .header_exists("User-Agent")
    //                .path_contains("id_manga")
    //                .path_contains("feed")
    //                .query_param("offset", "0")
    //                .query_param("translatedLanguage[]", default_language.as_iso_code())
    //                .query_param("order[volume]", default_chapter_order.to_string())
    //                .query_param("order[chapter]", default_chapter_order.to_string());
    //
    //            then.status(200).header("content-type", "application/json").json_body_obj(&expected);
    //        })
    //        .await;
    //
    //    let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());
    //
    //    let response = client
    //        .get_manga_chapters("id_manga", 1, Languages::default(), ChapterOrder::default())
    //        .await
    //        .expect("could not get manga chapters");
    //
    //    request.assert_async().await;
    //
    //    let response: ChapterResponse = response.json().await.expect("Could not deserialize response");
    //
    //    assert_eq!(response, expected);
    //}
    //
    //#[tokio::test]
    //async fn get_chapter_pages_response() {
    //    let server = MockServer::start_async().await;
    //    let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());
    //
    //    let expected = ChapterPagesResponse::default();
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET)
    //                .header_exists("User-Agent")
    //                .path_contains("id_manga")
    //                .path_contains("at-home")
    //                .path_contains("server");
    //
    //            then.status(200).header("content-type", "application/json").json_body_obj(&expected);
    //        })
    //        .await;
    //
    //    let response = client.get_chapter_pages("id_manga").await.expect("Error calling get_chapter_pages");
    //
    //    request.assert_async().await;
    //
    //    let response: ChapterPagesResponse = response.json().await.expect("Could not deserialize ChapterPagesResponse");
    //
    //    assert_eq!(response, expected);
    //}
    //
    //#[tokio::test]
    //async fn get_chapter_page() {
    //    let server = MockServer::start_async().await;
    //    let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());
    //
    //    let expected = "some_page_bytes";
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET).header_exists("User-Agent").path_contains("chapter.png");
    //
    //            then.status(200).body(expected.as_bytes());
    //        })
    //        .await;
    //
    //    let endpoint: Url = format!("{}/{}", server.base_url(), "chapter.png").parse().unwrap();
    //
    //    let response = client
    //        .get_chapter_page(endpoint)
    //        .await
    //        .expect("could not send request to get chapter page");
    //
    //    request.assert_async().await;
    //
    //    let response = response.bytes().await.expect("could not get manga page bytes");
    //
    //    assert_eq!(expected, response)
    //}
    //
    //#[tokio::test]
    //async fn get_manga_statistics() {
    //    let server = MockServer::start_async().await;
    //    let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());
    //    let id_manga = "some_id";
    //    let expected = MangaStatisticsResponse::default();
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET)
    //                .header_exists("User-Agent")
    //                .path_contains("statistics")
    //                .path_contains("manga")
    //                .path_contains(id_manga);
    //            then.status(200).json_body_obj(&expected);
    //        })
    //        .await;
    //
    //    let response = client
    //        .get_manga_statistics(id_manga)
    //        .await
    //        .expect("Could not send request to get manga statistics");
    //
    //    request.assert_async().await;
    //
    //    let response: MangaStatisticsResponse = response.json().await.expect("Could not deserialize MangaStatisticsResponse");
    //
    //    assert_eq!(response, expected);
    //}
    //
    //#[tokio::test]
    //async fn get_popular_mangas_mangadex() {
    //    let server = MockServer::start_async().await;
    //    let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());
    //
    //    let expected = SearchMangaResponse::default();
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET)
    //                .header_exists("User-Agent")
    //                .path_contains("/manga")
    //                .query_param("includes[]", "cover_art")
    //                .query_param("includes[]", "artist")
    //                .query_param("includes[]", "author")
    //                .query_param("order[followedCount]", "desc")
    //                .query_param("contentRating[]", "safe")
    //                .query_param("contentRating[]", "suggestive")
    //                .query_param("availableTranslatedLanguage[]", Languages::default().as_iso_code())
    //                .query_param_exists("createdAtSince");
    //
    //            then.status(200).json_body_obj(&expected);
    //        })
    //        .await;
    //
    //    let response = client
    //        .get_popular_mangas2()
    //        .await
    //        .expect("Could not send request to get manga statistics");
    //
    //    request.assert_async().await;
    //
    //    let response: SearchMangaResponse = response.json().await.expect("Could not deserialize get_popular_mangas response");
    //
    //    assert_eq!(response, expected);
    //}
    //
    //#[tokio::test]
    //async fn get_recently_added_mangadex() {
    //    let server = MockServer::start_async().await;
    //    let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());
    //
    //    let expected = SearchMangaResponse::default();
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET)
    //                .header_exists("User-Agent")
    //                .path_contains("/manga")
    //                .query_param("includes[]", "cover_art")
    //                .query_param("includes[]", "artist")
    //                .query_param("includes[]", "author")
    //                .query_param("limit", "5")
    //                .query_param("contentRating[]", "safe")
    //                .query_param("contentRating[]", "suggestive")
    //                .query_param("order[createdAt]", "desc")
    //                .query_param("availableTranslatedLanguage[]", Languages::default().as_iso_code());
    //
    //            then.status(200).json_body_obj(&expected);
    //        })
    //        .await;
    //
    //    let response = client
    //        .get_recently_added()
    //        .await
    //        .expect("Could not send request to get recently added mangas");
    //
    //    request.assert_async().await;
    //
    //    let response: SearchMangaResponse = response.json().await.expect("Could not deserialize get_recently_added response");
    //
    //    assert_eq!(response, expected);
    //}
    //
    //#[tokio::test]
    //async fn get_one_manga_mangadex() {
    //    let server = MockServer::start_async().await;
    //    let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());
    //
    //    let expected = OneMangaResponse::default();
    //    let manga_id = "some_id";
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET)
    //                .header_exists("User-Agent")
    //                .path_contains("/manga")
    //                .path_contains(manga_id)
    //                .query_param("includes[]", "cover_art")
    //                .query_param("includes[]", "artist")
    //                .query_param("includes[]", "author");
    //
    //            then.status(200).json_body_obj(&expected);
    //        })
    //        .await;
    //
    //    let response = client.get_one_manga(manga_id).await.expect("Could not send request to get one manga");
    //
    //    request.assert_async().await;
    //
    //    let response: OneMangaResponse = response.json().await.expect("Could not deserialize get_one_manga response");
    //
    //    assert_eq!(response, expected);
    //}
    //
    //#[tokio::test]
    //async fn get_latest_chapters_mangadex() {
    //    let server = MockServer::start_async().await;
    //    let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());
    //
    //    let expected = ChapterResponse::default();
    //    let manga_id = "some_id";
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET)
    //                .header_exists("User-Agent")
    //                .path_contains("/manga")
    //                .path_contains(manga_id)
    //                .path_contains("feed")
    //                .query_param("limit", ITEMS_PER_PAGE_LATEST_CHAPTERS.to_string())
    //                .query_param("includes[]", "scanlation_group")
    //                .query_param("offset", "0")
    //                .query_param("contentRating[]", "safe")
    //                .query_param("contentRating[]", "suggestive")
    //                .query_param("contentRating[]", "erotica")
    //                .query_param("contentRating[]", "pornographic")
    //                .query_param("order[readableAt]", "desc");
    //
    //            then.status(200).json_body_obj(&expected);
    //        })
    //        .await;
    //
    //    let response = client
    //        .get_latest_chapters(manga_id)
    //        .await
    //        .expect("Could not send request to get latest chapter of a manga");
    //
    //    request.assert_async().await;
    //
    //    let response: ChapterResponse = response.json().await.expect("Could not deserialize get_latest_chapters response");
    //
    //    assert_eq!(response, expected);
    //}
    //
    //#[tokio::test]
    //async fn get_tags_mangadex() {
    //    let server = MockServer::start_async().await;
    //    let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());
    //
    //    let expected = TagsResponse::default();
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET).header_exists("User-Agent").path_contains("/manga").path_contains("tag");
    //
    //            then.status(200).json_body_obj(&expected);
    //        })
    //        .await;
    //
    //    let response = client.get_tags().await.expect("Could not send request to get mangadex tags");
    //
    //    request.assert_async().await;
    //
    //    let response: TagsResponse = response.json().await.expect("Could not deserialize get_tags response");
    //
    //    assert_eq!(response, expected);
    //}
    //
    //#[tokio::test]
    //async fn search_author_and_artist_mangadex() {
    //    let server = MockServer::start_async().await;
    //    let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());
    //
    //    let expected = AuthorsResponse::default();
    //    let search_term = "some_author";
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET)
    //                .header_exists("User-Agent")
    //                .path_contains("/author")
    //                .query_param("name", search_term);
    //
    //            then.status(200).json_body_obj(&expected);
    //        })
    //        .await;
    //
    //    let response = client
    //        .get_authors(SearchTerm::trimmed_lowercased(search_term).unwrap())
    //        .await
    //        .expect("Could not send request to get mangadex author / artist");
    //
    //    request.assert_async().await;
    //
    //    let response: AuthorsResponse = response.json().await.expect("Could not deserialize get_authors response");
    //
    //    assert_eq!(response, expected);
    //}
    //
    //#[tokio::test]
    //async fn get_all_chapters_for_manga_mangadex() {
    //    let server = MockServer::start_async().await;
    //    let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());
    //
    //    let expected = ChapterResponse::default();
    //    let manga_id = "some_id";
    //    let language = Languages::default();
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET)
    //                .header_exists("User-Agent")
    //                .path_contains("/manga")
    //                .path_contains("feed")
    //                .path_contains(manga_id)
    //                .query_param("limit", "300")
    //                .query_param("offset", "0")
    //                .query_param("translatedLanguage[]", language.as_iso_code())
    //                .query_param("includes[]", "scanlation_group")
    //                .query_param("includeExternalUrl", "0")
    //                .query_param("order[volume]", "asc")
    //                .query_param("order[chapter]", "asc")
    //                .query_param("contentRating[]", "safe")
    //                .query_param("contentRating[]", "suggestive")
    //                .query_param("contentRating[]", "erotica")
    //                .query_param("contentRating[]", "pornographic");
    //
    //            then.status(200).json_body_obj(&expected);
    //        })
    //        .await;
    //
    //    let response = client
    //        .get_all_chapters_for_manga(manga_id, language)
    //        .await
    //        .expect("Could not send request to get all chapters of a manga");
    //
    //    request.assert_async().await;
    //
    //    let response: ChapterResponse = response.json().await.expect("Could not deserialize get_all_chapters_for_manga response");
    //
    //    assert_eq!(response, expected);
    //}
    //
    //#[tokio::test]
    //async fn check_mangadex_status() {
    //    let server = MockServer::start_async().await;
    //    let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET).header_exists("User-Agent").path_contains("/ping");
    //
    //            then.status(200);
    //        })
    //        .await;
    //
    //    let response = client
    //        .check_status()
    //        .await
    //        .expect("Could not send request to check mangadex status of a manga");
    //
    //    request.assert_async().await;
    //
    //    assert_eq!(response.status(), StatusCode::OK);
    //}
    //
    //#[tokio::test]
    //async fn it_searches_all_chapters_in_sequence() {
    //    let server = MockServer::start_async().await;
    //    let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());
    //
    //    let manga_id = Uuid::new_v4().to_string();
    //
    //    let expected_response = AggregateChapterResponse::default();
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET)
    //                .header_exists("User-Agent")
    //                .path_contains("manga")
    //                .path_contains(&manga_id)
    //                .path_contains("/aggregate")
    //                .query_param("translatedLanguage[]", "en");
    //
    //            then.status(200).json_body_obj(&expected_response);
    //        })
    //        .await;
    //
    //    let response = client.search_chapters_aggregate(&manga_id, Languages::default()).await.unwrap();
    //
    //    request.assert_async().await;
    //
    //    let data_sent: AggregateChapterResponse = response.json().await.expect("error deserializing response");
    //
    //    assert_eq!(expected_response, data_sent);
    //}
    //
    //#[tokio::test]
    //async fn mangadex_client_searches_chapter_by_id() {
    //    let server = MockServer::start_async().await;
    //    let client = MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap());
    //
    //    let chapter_id = Uuid::new_v4().to_string();
    //
    //    let expected_response = OneChapterResponse::default();
    //
    //    let request = server
    //        .mock_async(|when, then| {
    //            when.method(GET)
    //                .header_exists("User-Agent")
    //                .path_contains("/chapter")
    //                .path_contains(&chapter_id);
    //
    //            then.status(200).json_body_obj(&expected_response);
    //        })
    //        .await;
    //
    //    let response = client.search_chapters_by_id(&chapter_id).await.expect("error sending request");
    //
    //    request.assert_async().await;
    //
    //    let data_sent: OneChapterResponse = response.json().await.expect("error deserializing response");
    //
    //    assert_eq!(expected_response, data_sent);
    //}

    //#[tokio::test]
    //async fn test_mangadex() {
    //    let client = MangadexClient::new(API_URL_BASE.parse().unwrap(), COVER_IMG_URL_BASE.parse().unwrap());
    //
    //    //let chapter_id = "296cbc31-af1a-4b5b-a34b-fee2b4cad542";
    //    //let chapter_id = "046746c9-8872-4797-a112-318642fdb272";
    //    let chapter_id = "1bb7932c-ca3f-4827-9513-44bd9f0a75e9";
    //
    //    let response = client.search_chapters_aggregate(&chapter_id, Languages::SimplifiedChinese).await.unwrap();
    //
    //    let data_sent: AggregateChapterResponse = response.json().await.expect("error deserializing response");
    //
    //    dbg!(data_sent);
    //}
}
