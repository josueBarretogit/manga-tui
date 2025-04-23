use std::error::Error;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration as StdDuration;

use api_responses::*;
use bytes::Bytes;
use chrono::{Duration, Months};
use filter::{Filters, IntoParam, MangadexFilterProvider};
use filter_widget::MangadexFilterWidget;
use http::header::{ACCEPT, ACCEPT_ENCODING, CACHE_CONTROL};
use http::{HeaderMap, HeaderValue, StatusCode};
use manga_tui::SearchTerm;
use reqwest::{Client, Response, Url};
use serde_json::json;

use super::{
    Artist, Author, Chapter, ChapterPageUrl, ChapterToRead, DecodeBytesToImage, FeedPageProvider, FetchChapterBookmarked, Genres,
    GetChapterPages, GetChaptersResponse, GetMangasResponse, GetRawImage, GoToReadChapter, HomePageMangaProvider, Languages,
    ListOfChapters, Manga, MangaPageProvider, MangaProvider, MangaProviders, MangaStatus, PopularManga, ProviderIdentity, Rating,
    ReaderPageProvider, RecentlyAddedManga, SearchChapterById, SearchMangaById, SearchMangaPanel, SearchPageProvider,
};
use crate::backend::cache::{CacheDuration, Cacher, InsertEntry};
use crate::backend::database::ChapterBookmarked;
use crate::config::ImageQuality;
use crate::global::APP_USER_AGENT;
use crate::view::widgets::StatefulWidgetFrame;

pub mod api_responses;
pub mod filter;
pub mod filter_widget;

pub static API_URL_BASE: &str = "https://api.mangadex.org";

pub static COVER_IMG_URL_BASE: &str = "https://uploads.mangadex.org/covers";

/// Mangadex: `https://mangadex.org`
/// This is the first manga provider since the first versions of manga-tui, thats why it is the
/// default
/// the implementation of `MangaProvider` is mostly based on the api requests made on the website
/// which can be seen in the network tab in the dev-tools
/// documentation on how to use the mangadex api can be found [here](https://api.mangadex.org/docs)
/// Mangadex can:
/// - Provide manga translated in multiple languages
/// - Provide an option to fetch chapter pages with lower quality `https://api.mangadex.org/docs/04-chapter/retrieving-chapter/#2-construct-page-urls`
/// - Has, in my opinion the most advanced search with multiple genres and options
#[derive(Clone, Debug)]
pub struct MangadexClient {
    client: reqwest::Client,
    cache_provider: Arc<dyn Cacher>,
    api_url_base: Url,
    cover_img_url_base: Url,
    image_quality: ImageQuality,
}

impl MangadexClient {
    /// see: `https://api.mangadex.org/docs/04-chapter/retrieving-chapter/#2-construct-page-urls`
    fn make_cover_img_url_lower_quality(&self, manga_id: &str, file_name: &str) -> String {
        let file_name = format!("{}/{manga_id}/{file_name}.256.jpg", self.cover_img_url_base);
        file_name
    }

    /// see: `https://api.mangadex.org/docs/04-chapter/retrieving-chapter/#2-construct-page-urls`
    fn make_cover_img_url(&self, manga_id: &str, file_name: &str) -> String {
        let file_name = format!("{}/{manga_id}/{file_name}.512.jpg", self.cover_img_url_base);
        file_name
    }

    pub fn new(api_url_base: Url, cover_img_url_base: Url, cache_provider: Arc<dyn Cacher>) -> Self {
        let mut default_headers = HeaderMap::new();
        default_headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate"));
        default_headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        default_headers.insert(CACHE_CONTROL, HeaderValue::from_static("max-age=604800"));

        let client = Client::builder()
            .default_headers(default_headers)
            .timeout(StdDuration::from_secs(10))
            .user_agent(&*APP_USER_AGENT)
            .build()
            .unwrap();

        Self {
            client,
            api_url_base,
            cover_img_url_base,
            cache_provider,
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
    ) -> Result<AggregateChapterResponse, Box<dyn Error>> {
        let endpoint =
            format!("{}/manga/{}/aggregate?translatedLanguage[]={}", self.api_url_base, manga_id, language.as_iso_code());

        let cache = self.cache_provider.get(&endpoint)?;

        match cache {
            Some(cached) => Ok(serde_json::from_str(&cached.data)?),
            None => {
                let response: AggregateChapterResponse = self.client.get(&endpoint).send().await?.json().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &endpoint,
                        data: json!(response).to_string().as_str(),
                        duration: CacheDuration::Long,
                    })
                    .ok();

                Ok(response)
            },
        }
    }

    /// Used in `manga` page to request the the amount of follows and stars a manga has
    async fn get_manga_statistics(&self, id_manga: &str) -> Result<MangaStatisticsResponse, reqwest::Error> {
        let endpoint = format!("{}/statistics/manga/{id_manga}", self.api_url_base);

        let response: MangaStatisticsResponse =
            self.client.get(endpoint).timeout(StdDuration::from_secs(1)).send().await?.json().await?;

        Ok(response)
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

    fn map_popular_mangas(&self) -> impl Fn(Data) -> PopularManga + '_ {
        |manga| {
            let mut cover_img_url = String::new();

            for rel in &manga.relationships {
                if let Some(attributes) = &rel.attributes {
                    match rel.type_field.as_str() {
                        "cover_art" => {
                            let file_name = attributes.file_name.as_ref().unwrap().to_string();
                            cover_img_url = self.make_cover_img_url(&manga.id, &file_name);
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

            let mut genres: Vec<Genres> = manga.attributes.tags.into_iter().map(Genres::from).collect();

            let content_rating = match manga.attributes.content_rating.as_str() {
                "suggestive" => Rating::Moderate,
                "pornographic" | "erotica" => Rating::Nsfw,
                _ => Rating::Normal,
            };

            genres.push(Genres::new(manga.attributes.content_rating, content_rating));

            if let Some(pb) = manga.attributes.publication_demographic {
                genres.push(Genres::new(pb, Rating::Normal));
            }

            PopularManga {
                id: manga.id,
                description: manga.attributes.description.map(|desc| desc.en.unwrap_or_default()).unwrap_or_default(),
                genres,
                title: manga.attributes.title.into(),
                status: Some(status),
                cover_img_url,
            }
        }
    }

    async fn map_manga_found_by_id(&self, manga: GetMangaByIdResponse) -> Manga {
        let id = manga.data.id;

        let title = manga.data.attributes.title.into();

        let mut genres: Vec<Genres> = manga.data.attributes.tags.into_iter().map(Genres::from).collect();

        let content_rating = match manga.data.attributes.content_rating.as_str() {
            "suggestive" => Rating::Moderate,
            "pornographic" | "erotica" => Rating::Nsfw,
            _ => Rating::Normal,
        };

        genres.push(Genres::new(manga.data.attributes.content_rating, content_rating));

        if let Some(pb) = manga.data.attributes.publication_demograpchic {
            genres.push(Genres::new(pb, Rating::Normal));
        }

        let description = manga
            .data
            .attributes
            .description
            .map(|desc| desc.en.unwrap_or_default())
            .unwrap_or_default();

        let status = match manga.data.attributes.status.as_str() {
            "ongoing" => MangaStatus::Ongoing,
            "hiatus" => MangaStatus::Hiatus,
            "completed" => MangaStatus::Completed,
            "cancelled" => MangaStatus::Cancelled,
            _ => MangaStatus::default(),
        };

        let mut cover_img_url: String = String::new();
        let mut author: Option<Author> = Option::default();
        let mut artist: Option<Artist> = Option::default();

        for rel in &manga.data.relationships {
            if let Some(attributes) = &rel.attributes {
                match rel.type_field.as_str() {
                    "cover_art" => {
                        let file_name = attributes.file_name.as_ref().unwrap().to_string();
                        cover_img_url = self.make_cover_img_url_lower_quality(&id, &file_name);
                    },
                    "author" => {
                        let name = rel.attributes.as_ref().unwrap().name.as_ref().cloned().unwrap_or_default();
                        author = Some(Author {
                            id: rel.id.clone(),
                            name,
                        })
                    },
                    "artist" => {
                        let name = rel.attributes.as_ref().unwrap().name.as_ref().cloned().unwrap_or_default();
                        artist = Some(Artist {
                            id: rel.id.clone(),
                            name,
                        })
                    },
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
        let rating = rating
            .statistics
            .get(&id)
            .cloned()
            .unwrap_or_default()
            .rating
            .average
            .unwrap_or_default()
            .ceil();

        let rating = format!("{rating} out of 10");
        Manga {
            rating,
            id: id.clone(),
            id_safe_for_download: id,
            title,
            genres,
            description,
            status,
            cover_img_url,
            languages,
            artist,
            author,
        }
    }

    fn map_response_to_pages_url(&self, response: ChapterPagesResponse, image_quality: ImageQuality) -> Vec<ChapterPageUrl> {
        let base_url = response.get_image_url_endpoint(image_quality);

        let image_file_names = match image_quality {
            ImageQuality::Low => response.chapter.data_saver,
            ImageQuality::High => response.chapter.data,
        };

        let mut pages_url: Vec<ChapterPageUrl> = vec![];

        for file_name in image_file_names {
            // No panics should ocurr with these unwraps if the response is Ok
            let url = Url::parse(&format!("{base_url}/{file_name}")).unwrap();
            let extension = Path::new(&file_name).extension().unwrap().to_str().unwrap().to_string();
            pages_url.push(ChapterPageUrl { url, extension });
        }

        pages_url
    }

    fn map_chapter_response(&self, manga_id: String) -> impl Fn(ChapterData) -> Chapter + '_ {
        move |chapter| {
            let id = chapter.id;
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
                id: id.clone(),
                id_safe_for_download: id,
                manga_id: manga_id.clone(),
                title,
                language,
                chapter_number,
                volume_number,
                scanlator,
                publication_date: chrono::DateTime::parse_from_rfc3339(&publication_date).unwrap_or_default().date_naive(),
            }
        }
    }

    fn map_latest_chapter(&self, manga_id: String) -> impl Fn(ChapterData) -> super::LatestChapter {
        move |chap| {
            let id = chap.id;

            let language = Languages::try_from_iso_code(chap.attributes.translated_language.as_str()).unwrap_or_default();

            let chapter_number = chap.attributes.chapter.unwrap_or("0".to_string());
            let title = chap.attributes.title.unwrap_or("No title".to_string());
            let publication_date = chap.attributes.readable_at;
            let volume_number = chap.attributes.volume;

            super::LatestChapter {
                id,
                title,
                manga_id: manga_id.clone(),
                language,
                chapter_number,
                publication_date: chrono::DateTime::parse_from_rfc3339(&publication_date).unwrap_or_default().date_naive(),
                volume_number,
            }
        }
    }
}

impl GetRawImage for MangadexClient {
    async fn get_raw_image(&self, url: &str) -> Result<Bytes, Box<dyn Error>> {
        let response = self.client.get(url).timeout(StdDuration::from_secs(10)).send().await?;

        if response.status() != StatusCode::OK {
            return Err(format!("Could not get image on mangadex with url : {url}").into());
        }

        Ok(response.bytes().await?)
    }
}

impl DecodeBytesToImage for MangadexClient {}

impl HomePageMangaProvider for MangadexClient {
    async fn get_recently_added_mangas(&self) -> Result<Vec<RecentlyAddedManga>, Box<dyn Error>> {
        let language = Languages::get_preferred_lang().as_iso_code();

        let endpoint = format!("{}/manga", self.api_url_base);

        let response = self
            .client
            .get(endpoint)
            .query(&[
                ("limit", "5"),
                ("contentRating[]", "safe"),
                ("order[createdAt]", "desc"),
                ("includes[]", "cover_art"),
                ("includes[]", "artist"),
                ("includes[]", "author"),
                ("hasAvailableChapters", "true"),
                ("availableTranslatedLanguage[]", language),
            ])
            .send()
            .await?;

        if response.status() != StatusCode::OK {
            return Err(format!(
                "Could not get recently added mangas on mangadex, more details about the request : {:#?}",
                response
            )
            .into());
        }

        let response: SearchMangaResponse = response.json().await?;

        Ok(response
            .data
            .into_iter()
            .map(|manga| {
                let mut cover_img_url = String::new();

                for rel in &manga.relationships {
                    if let Some(attributes) = &rel.attributes {
                        match rel.type_field.as_str() {
                            "cover_art" => {
                                let file_name = attributes.file_name.as_ref().unwrap().to_string();
                                cover_img_url = self.make_cover_img_url_lower_quality(&manga.id, &file_name);
                            },
                            _ => {},
                        }
                    }
                }
                RecentlyAddedManga {
                    id: manga.id,
                    title: manga.attributes.title.into(),
                    description: manga
                        .attributes
                        .description
                        .map(|desc| desc.en.unwrap_or("No description".to_string()))
                        .unwrap_or("No description".to_string()),
                    cover_img_url,
                }
            })
            .collect())
    }

    async fn get_popular_mangas(&self) -> Result<Vec<PopularManga>, Box<dyn Error>> {
        let current_date = chrono::offset::Local::now().date_naive().checked_sub_months(Months::new(1)).unwrap();
        let language = Languages::get_preferred_lang().as_iso_code();

        let endpoint = format!("{}/manga", self.api_url_base);

        let from_date = format!("{current_date}T00:00:00");

        let id_popular_manga_cache = format!("{endpoint}{from_date}-popular_manga");

        let cache = self.cache_provider.get(&id_popular_manga_cache)?;

        match cache {
            Some(cached) => {
                let response: SearchMangaResponse = serde_json::from_str(&cached.data)?;

                Ok(response.data.into_iter().map(self.map_popular_mangas()).collect())
            },
            None => {
                let response = self
                    .client
                    .get(endpoint)
                    .query(&[
                        ("includes[]", "cover_art"),
                        ("includes[]", "artist"),
                        ("includes[]", "author"),
                        ("order[followedCount]", "desc"),
                        ("contentRating[]", "safe"),
                        ("hasAvailableChapters", "true"),
                        ("availableTranslatedLanguage[]", language),
                        ("createdAtSince", from_date.as_str()),
                    ])
                    .send()
                    .await?;

                if response.status() != StatusCode::OK {
                    return Err(format!(
                        "Could not get popular mangas on mangadex, more details about the request: {:#?}",
                        response
                    )
                    .into());
                }

                let response: SearchMangaResponse = response.json().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &id_popular_manga_cache,
                        data: &json!(response).to_string(),
                        duration: CacheDuration::Short,
                    })
                    .ok();

                Ok(response.data.into_iter().map(self.map_popular_mangas()).collect())
            },
        }
    }
}

impl SearchMangaById for MangadexClient {
    async fn get_manga_by_id(&self, manga_id: &str) -> Result<super::Manga, Box<dyn Error>> {
        let endpoint = format!("{}/manga/{manga_id}", self.api_url_base);
        let id_cache = format!("{endpoint}-manga-by-id");
        let cache = self.cache_provider.get(&id_cache)?;

        match cache {
            Some(cached) => {
                let manga: GetMangaByIdResponse = serde_json::from_str(&cached.data)?;

                Ok(self.map_manga_found_by_id(manga).await)
            },
            None => {
                let response = self
                    .client
                    .get(endpoint)
                    .query(&[("includes[]", "cover_art"), ("includes[]", "author"), ("includes[]", "artist")])
                    .send()
                    .await?;

                if response.status() != StatusCode::OK {
                    return Err(format!(
                        "failed to get manga of id {manga_id} on mangadex, more details about the response: \n {:#?}",
                        response
                    )
                    .into());
                }

                let manga: GetMangaByIdResponse = response.json().await?;
                self.cache_provider
                    .cache(InsertEntry {
                        id: &id_cache,
                        data: &json!(manga).to_string(),
                        duration: CacheDuration::LongLong,
                    })
                    .ok();

                Ok(self.map_manga_found_by_id(manga).await)
            },
        }
    }
}

impl GoToReadChapter for MangadexClient {
    async fn read_chapter(&self, chapter_id: &str, manga_id: &str) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
        let chapter = self.search_chapter(chapter_id, manga_id).await?;

        let list_of_chapters = self.search_list_of_chapters(manga_id, chapter.language).await?;

        Ok((chapter, list_of_chapters.into()))
    }
}

impl GetChapterPages for MangadexClient {
    async fn get_chapter_pages_url_with_extension(
        &self,
        chapter_id: &str,
        _manga_id: &str,
        image_quality: ImageQuality,
    ) -> Result<Vec<ChapterPageUrl>, Box<dyn Error>> {
        let endpoint = format!("{}/at-home/server/{chapter_id}", self.api_url_base);
        let cache = self.cache_provider.get(&endpoint)?;
        match cache {
            Some(cached) => {
                let response: ChapterPagesResponse = serde_json::from_str(&cached.data)?;

                Ok(self.map_response_to_pages_url(response, self.image_quality))
            },
            None => {
                let response = self.client.get(&endpoint).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!(
                                "Could not get the pages url for chapter with id {chapter_id} on mangadex, details about the response : {:#?}",
                response
                )
                    .into());
                }

                let response: ChapterPagesResponse = response.json().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &endpoint,
                        data: json!(response).to_string().as_str(),
                        duration: CacheDuration::Long,
                    })
                    .ok();

                Ok(self.map_response_to_pages_url(response, self.image_quality))
            },
        }
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

        let id_cache = format!("{endpoint}{offset}{order}get-chapters-cache");

        let cache = self.cache_provider.get(&id_cache)?;

        match cache {
            Some(cached) => {
                let response: ChapterResponse = serde_json::from_str(&cached.data)?;

                let total_items = response.total;

                let response: Vec<super::Chapter> =
                    response.data.into_iter().map(self.map_chapter_response(manga_id.to_string())).collect();

                Ok(GetChaptersResponse {
                    chapters: response,
                    total_chapters: total_items as u32,
                })
            },
            None => {
                let response = self
                    .client
                    .get(endpoint)
                    .query(&[
                        ("includes[]", "scanlation_group"),
                        ("limit", &pagination.items_per_page.to_string()),
                        ("offset", &offset.to_string()),
                        ("order", order),
                        ("contentRating[]", "safe"),
                        ("contentRating[]", "suggestive"),
                        ("contentRating[]", "erotica"),
                        ("contentRating[]", "pornographic"),
                        ("order[volume]", order),
                        ("order[chapter]", order),
                        ("includeExternalUrl", "0"),
                        ("translatedLanguage[]", filters.language.as_iso_code()),
                    ])
                    .send()
                    .await?;

                if response.status() != StatusCode::OK {
                    return Err(format!("Could not get chapters for manga with id : {manga_id}").into());
                }

                let response: ChapterResponse = response.json().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &id_cache,
                        data: json!(response).to_string().as_str(),
                        duration: CacheDuration::Long,
                    })
                    .ok();

                let total_items = response.total;

                let response: Vec<super::Chapter> =
                    response.data.into_iter().map(self.map_chapter_response(manga_id.to_string())).collect();

                Ok(GetChaptersResponse {
                    chapters: response,
                    total_chapters: total_items as u32,
                })
            },
        }
    }

    async fn get_all_chapters(&self, manga_id: &str, language: Languages) -> Result<Vec<Chapter>, Box<dyn Error>> {
        let language = language.as_iso_code();

        let endpoint = format!("{}/manga/{manga_id}/feed", self.api_url_base);

        let id_cache = format!("{endpoint}get-all-chapters");
        let cache = self.cache_provider.get(&id_cache)?;

        match cache {
            Some(cached) => {
                let response: ChapterResponse = serde_json::from_str(&cached.data)?;

                let response: Vec<Chapter> =
                    response.data.into_iter().map(self.map_chapter_response(manga_id.to_string())).collect();

                Ok(response)
            },
            None => {
                let response = self
                    .client
                    .get(endpoint)
                    .query(&[
                        // According to https://api.mangadex.org/docs/2-limitations limit is restricted to
                        // 500 max
                        ("limit", "500"),
                        ("offset", "0"),
                        ("order[volume]", "asc"),
                        ("order[chapter]", "asc"),
                        ("translatedLanguage[]", language),
                        ("includes[]", "scanlation_group"),
                        ("includeExternalUrl", "0"),
                        ("contentRating[]", "safe"),
                        ("contentRating[]", "suggestive"),
                        ("contentRating[]", "erotica"),
                        ("contentRating[]", "pornographic"),
                    ])
                    .timeout(StdDuration::from_secs(10))
                    .send()
                    .await?;

                if response.status() != StatusCode::OK {
                    return Err(format!(
                        "Could not get all chapters for manga with id {manga_id} on mangadex, details about the response: {:#?}",
                        response
                    )
                    .into());
                }

                let response: ChapterResponse = response.json().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &id_cache,
                        data: json!(response).to_string().as_str(),
                        duration: CacheDuration::Long,
                    })
                    .ok();

                let response: Vec<Chapter> =
                    response.data.into_iter().map(self.map_chapter_response(manga_id.to_string())).collect();

                Ok(response)
            },
        }
    }
}

impl SearchChapterById for MangadexClient {
    async fn search_chapter(&self, chapter_id: &str, _manga_id: &str) -> Result<ChapterToRead, Box<dyn std::error::Error>> {
        let endpoint = format!("{}/chapter/{chapter_id}", self.api_url_base);
        let cache = self.cache_provider.get(&endpoint)?;

        match cache {
            Some(cached) => {
                let response: OneChapterResponse = serde_json::from_str(&cached.data)?;

                let pages_url: Vec<Url> = self
                    .get_chapter_pages_url_with_extension(chapter_id, "", self.image_quality)
                    .await?
                    .into_iter()
                    .map(|page| page.url)
                    .collect();

                let language =
                    Languages::try_from_iso_code(response.data.attributes.translated_language.as_str()).unwrap_or_default();

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
                    pages_url,
                })
            },
            None => {
                let response = self.client.get(&endpoint).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!(
                        "Could not get chapter of id {chapter_id} on mangadex, details about the request: {:#?}",
                        response
                    )
                    .into());
                }

                let response: OneChapterResponse = response.json().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &endpoint,
                        data: json!(response).to_string().as_str(),
                        duration: CacheDuration::Long,
                    })
                    .ok();

                let pages_url: Vec<Url> = self
                    .get_chapter_pages_url_with_extension(chapter_id, "", self.image_quality)
                    .await?
                    .into_iter()
                    .map(|page| page.url)
                    .collect();

                let language =
                    Languages::try_from_iso_code(response.data.attributes.translated_language.as_str()).unwrap_or_default();

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
                    pages_url,
                })
            },
        }
    }
}

impl SearchMangaPanel for MangadexClient {}

impl FetchChapterBookmarked for MangadexClient {
    async fn fetch_chapter_bookmarked(
        &self,
        chapter: ChapterBookmarked,
    ) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
        let chapter_found = self.search_chapter(&chapter.id, "").await?;

        let list_of_chapters: AggregateChapterResponse =
            self.search_list_of_chapters(&chapter.manga_id, chapter_found.language).await?;

        Ok((chapter_found, ListOfChapters::from(list_of_chapters)))
    }
}

impl ReaderPageProvider for MangadexClient {}

impl SearchPageProvider for MangadexClient {
    type FiltersHandler = MangadexFilterProvider;
    type InnerState = Filters;
    type Widget = MangadexFilterWidget;

    async fn search_mangas(
        &self,
        search_term: Option<SearchTerm>,
        filters: Self::InnerState,
        pagination: super::Pagination,
    ) -> Result<GetMangasResponse, Box<dyn Error>> {
        let offset = (pagination.current_page - 1) * pagination.items_per_page;

        let search_by_title = match search_term {
            Some(search) => format!("title={}", search),
            None => "".to_string(),
        };

        let filters = filters.into_param();
        let items_per_page = pagination.items_per_page;

        let url = format!(
            "{}/manga?{search_by_title}&includes[]=cover_art&includes[]=author&includes[]=artist&limit={items_per_page}&offset={offset}{filters}&includedTagsMode=AND&excludedTagsMode=OR&hasAvailableChapters=true",
            self.api_url_base,
        );

        let response: SearchMangaResponse = self.client.get(url).send().await?.json().await?;

        let total_mangas = response.total;

        let mangas: Vec<super::SearchManga> = response
            .data
            .into_iter()
            .map(|manga| {
                let id = manga.id;

                let title = manga.attributes.title.en.unwrap_or("No title".to_string());

                let mut genres: Vec<Genres> = manga.attributes.tags.into_iter().map(Genres::from).collect();

                let content_rating = match manga.attributes.content_rating.as_str() {
                    "suggestive" => Rating::Moderate,
                    "pornographic" | "erotica" => Rating::Nsfw,
                    _ => Rating::Normal,
                };

                genres.push(Genres::new(manga.attributes.content_rating, content_rating));

                let description = manga.attributes.description.map(|desc| desc.en.unwrap_or_default()).unwrap_or_default();

                let status = match manga.attributes.status.as_str() {
                    "ongoing" => MangaStatus::Ongoing,
                    "hiatus" => MangaStatus::Hiatus,
                    "completed" => MangaStatus::Completed,
                    "cancelled" => MangaStatus::Cancelled,
                    _ => MangaStatus::default(),
                };

                let mut cover_img_url = String::new();
                let mut author: Option<Author> = Option::default();
                let mut artist: Option<Artist> = Option::default();
                for rel in &manga.relationships {
                    if let Some(attributes) = &rel.attributes {
                        match rel.type_field.as_str() {
                            "cover_art" => {
                                let file_name = attributes.file_name.as_ref().unwrap().to_string();
                                cover_img_url = self.make_cover_img_url_lower_quality(&id, &file_name);
                            },
                            "author" => {
                                let name = rel.attributes.as_ref().unwrap().name.as_ref().cloned().unwrap_or_default();
                                author = Some(Author {
                                    id: rel.id.clone(),
                                    name,
                                })
                            },
                            "artist" => {
                                let name = rel.attributes.as_ref().unwrap().name.as_ref().cloned().unwrap_or_default();
                                artist = Some(Artist {
                                    id: rel.id.clone(),
                                    name,
                                })
                            },
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
                    artist,
                    author,
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

impl FeedPageProvider for MangadexClient {
    async fn get_latest_chapters(&self, manga_id: &str) -> Result<Vec<super::LatestChapter>, Box<dyn Error>> {
        let endpoint = format!("{}/manga/{manga_id}/feed", self.api_url_base);
        let id_cache = format!("{endpoint}latest-chapters");

        let cache = self.cache_provider.get(&id_cache)?;

        match cache {
            Some(cached) => {
                let response: ChapterResponse = serde_json::from_str(&cached.data)?;

                Ok(response.data.into_iter().map(self.map_latest_chapter(manga_id.to_string())).collect())
            },
            None => {
                let response: ChapterResponse = self
                    .client
                    .get(endpoint)
                    .query(&[
                        ("limit", "5"),
                        ("offset", "0"),
                        ("includes[]", "scanlation_group"),
                        ("order[readableAt]", "desc"),
                        ("contentRating[]", "safe"),
                        ("contentRating[]", "suggestive"),
                        ("contentRating[]", "erotica"),
                        ("contentRating[]", "pornographic"),
                    ])
                    .send()
                    .await?
                    .json()
                    .await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &id_cache,
                        data: json!(response).to_string().as_str(),
                        duration: CacheDuration::Short,
                    })
                    .ok();

                Ok(response.data.into_iter().map(self.map_latest_chapter(manga_id.to_string())).collect())
            },
        }
    }
}

impl ProviderIdentity for MangadexClient {
    fn name(&self) -> MangaProviders {
        MangaProviders::Mangadex
    }
}

impl MangaProvider for MangadexClient {}

#[cfg(test)]
mod test {
    use cache::mock::EmptyCache;
    use httpmock::Method::GET;
    use httpmock::MockServer;
    use manga_provider::{ChapterFilters, LatestChapter, Manga, Pagination, SearchManga};
    use pretty_assertions::assert_eq;
    use reqwest::StatusCode;
    use uuid::Uuid;

    use self::api_responses::authors::AuthorsResponse;
    use self::api_responses::feed::OneMangaResponse;
    use self::api_responses::tags::TagsResponse;
    use self::api_responses::{AggregateChapterResponse, ChapterResponse, SearchMangaResponse};
    use super::*;
    use crate::backend::*;

    #[test]
    fn expected_mangadex_endpoints() {
        assert_eq!("https://api.mangadex.org", API_URL_BASE);
        assert_eq!("https://uploads.mangadex.org/covers", COVER_IMG_URL_BASE);
    }

    #[tokio::test]
    async fn it_search_mangas() {
        let server = MockServer::start_async().await;

        let expected = GetMangasResponse {
            mangas: vec![SearchManga {
                id: "some_id".to_string(),
                title: "No title".to_string(),
                genres: vec![Genres::default()],
                description: Some("".to_string()),
                status: Some(MangaStatus::Ongoing),
                ..Default::default()
            }],
            total_mangas: 5,
        };

        let response = SearchMangaResponse {
            total: 5,
            data: vec![Data {
                id: "some_id".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };

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

                then.status(200).header("content-type", "application/json").json_body_obj(&response);
            })
            .await;

        let client =
            MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap(), EmptyCache::new_arc());

        let response = client
            .search_mangas(SearchTerm::trimmed_lowercased("some title"), Filters::default(), Pagination::from_first_page(1))
            .await
            .expect("an issue ocurrend when calling search_mangas");

        request.assert_async().await;

        assert_eq!(expected, response);
    }

    #[tokio::test]
    async fn get_cover_image_works() {
        let server = MockServer::start_async().await;
        let client =
            MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap(), EmptyCache::new_arc());

        let expected = "some_image_bytes".as_bytes();

        let request = server
            .mock_async(|when, then| {
                when.method(GET).path_contains("id_manga").header_exists("User-Agent");

                then.status(200).header("content-type", "image/jpeg").body(expected);
            })
            .await;

        let response = client
            .get_raw_image(&server.url("/id_manga.jpg"))
            .await
            .expect("could not get cover for a manga");

        request.assert_async().await;

        assert_eq!(expected, response);
    }

    #[tokio::test]
    async fn get_manga_chapters() {
        let server = MockServer::start_async().await;

        let expected = GetChaptersResponse {
            chapters: vec![Chapter {
                manga_id: "id_manga".to_string(),
                title: "No title".to_string(),
                language: Languages::Unkown,
                chapter_number: "0".to_string(),
                ..Default::default()
            }],
            total_chapters: 1,
        };

        let response = ChapterResponse {
            total: 1,
            data: vec![ChapterData::default()],
            ..Default::default()
        };

        let request = server
            .mock_async(|when, then| {
                when.method(GET)
                    .header_exists("User-Agent")
                    .path_contains("id_manga")
                    .path_contains("feed")
                    .query_param("offset", "0")
                    .query_param("translatedLanguage[]", "en")
                    .query_param("order[volume]", "desc")
                    .query_param("order[chapter]", "desc");

                then.status(200).header("content-type", "application/json").json_body_obj(&response);
            })
            .await;

        let client =
            MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap(), EmptyCache::new_arc());

        let response = client
            .get_chapters("id_manga", ChapterFilters::default(), Pagination::default())
            .await
            .expect("could not get manga chapters");

        request.assert_async().await;

        assert_eq!(expected, response);
    }

    #[tokio::test]
    async fn get_popular_mangas() {
        let server = MockServer::start_async().await;
        let client =
            MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap(), EmptyCache::new_arc());

        let expected: Vec<PopularManga> = vec![PopularManga {
            id: "some_id".to_string(),
            title: "No title".to_string(),
            genres: vec![Genres::default()],
            status: Some(MangaStatus::Ongoing),
            ..Default::default()
        }];

        let response = SearchMangaResponse {
            data: vec![Data {
                id: "some_id".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };

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
                    .query_param("availableTranslatedLanguage[]", "en")
                    .query_param_exists("createdAtSince");

                then.status(200).json_body_obj(&response);
            })
            .await;

        let response = client.get_popular_mangas().await.expect("Could not send request to get manga statistics");

        request.assert_async().await;

        assert_eq!(expected, response);
    }

    #[tokio::test]
    async fn get_recently_added_mangadex() {
        let server = MockServer::start_async().await;
        let client =
            MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap(), EmptyCache::new_arc());

        let expected = vec![RecentlyAddedManga {
            id: "some_id".to_string(),
            title: "No title".to_string(),
            description: "No description".to_string(),
            ..Default::default()
        }];
        let response = SearchMangaResponse {
            data: vec![Data {
                id: "some_id".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };

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
                    .query_param("order[createdAt]", "desc")
                    .query_param("availableTranslatedLanguage[]", "en");

                then.status(200).json_body_obj(&response);
            })
            .await;

        let response = client
            .get_recently_added_mangas()
            .await
            .expect("Could not send request to get recently added mangas");

        request.assert_async().await;

        assert_eq!(expected, response);
    }

    #[tokio::test]
    async fn get_manga_page() {
        let server = MockServer::start_async().await;
        let client =
            MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap(), EmptyCache::new_arc());

        let expected = Manga {
            id: "some_id".to_string(),
            rating: "0 out of 10".to_string(),
            id_safe_for_download: "some_id".to_string(),
            genres: vec![Genres::default()],
            title: "No title".to_string(),
            ..Default::default()
        };

        let response = OneMangaResponse {
            data: Data {
                id: "some_id".to_string(),
                attributes: Attributes {
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

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

                then.status(200).json_body_obj(&response);
            })
            .await;

        let response = client.get_manga_by_id("some_id").await.unwrap();

        request.assert_async().await;

        assert_eq!(expected, response);
    }

    #[tokio::test]
    async fn get_latest_chapters() {
        let server = MockServer::start_async().await;
        let client =
            MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap(), EmptyCache::new_arc());

        let expected = vec![LatestChapter {
            manga_id: "some_id".to_string(),
            title: "No title".to_string(),
            language: Languages::Unkown,
            chapter_number: "0".to_string(),
            ..Default::default()
        }];

        let response = ChapterResponse {
            data: vec![ChapterData::default()],
            ..Default::default()
        };
        let manga_id = "some_id";

        let request = server
            .mock_async(|when, then| {
                when.method(GET)
                    .header_exists("User-Agent")
                    .path_contains("/manga")
                    .path_contains(manga_id)
                    .path_contains("feed")
                    .query_param("limit", "5")
                    .query_param("includes[]", "scanlation_group")
                    .query_param("offset", "0")
                    .query_param("contentRating[]", "safe")
                    .query_param("contentRating[]", "suggestive")
                    .query_param("contentRating[]", "erotica")
                    .query_param("contentRating[]", "pornographic")
                    .query_param("order[readableAt]", "desc");

                then.status(200).json_body_obj(&response);
            })
            .await;

        let response = client
            .get_latest_chapters(manga_id)
            .await
            .expect("Could not send request to get latest chapter of a manga");

        request.assert_async().await;

        assert_eq!(expected, response);
    }

    #[tokio::test]
    async fn get_tags_mangadex() {
        let server = MockServer::start_async().await;
        let client =
            MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap(), EmptyCache::new_arc());

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
        let client =
            MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap(), EmptyCache::new_arc());

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
    async fn get_all_chapters_for_manga() {
        let server = MockServer::start_async().await;
        let client =
            MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap(), EmptyCache::new_arc());

        let expected = vec![Chapter {
            manga_id: "some_id".to_string(),
            title: "No title".to_string(),
            language: Languages::Unkown,
            chapter_number: "0".to_string(),
            ..Default::default()
        }];

        let response = ChapterResponse {
            data: vec![ChapterData::default()],
            ..Default::default()
        };
        let manga_id = "some_id";

        let request = server
            .mock_async(|when, then| {
                when.method(GET)
                    .header_exists("User-Agent")
                    .path_contains("/manga")
                    .path_contains("feed")
                    .path_contains(manga_id)
                    .query_param("limit", "500")
                    .query_param("offset", "0")
                    .query_param("translatedLanguage[]", "en")
                    .query_param("includes[]", "scanlation_group")
                    .query_param("includeExternalUrl", "0")
                    .query_param("order[volume]", "asc")
                    .query_param("order[chapter]", "asc")
                    .query_param("contentRating[]", "safe")
                    .query_param("contentRating[]", "suggestive")
                    .query_param("contentRating[]", "erotica")
                    .query_param("contentRating[]", "pornographic");

                then.status(200).json_body_obj(&response);
            })
            .await;

        let response = client
            .get_all_chapters(manga_id, Languages::English)
            .await
            .expect("Could not send request to get all chapters of a manga");

        request.assert_async().await;

        assert_eq!(expected, response);
    }

    #[tokio::test]
    async fn check_mangadex_status() {
        let server = MockServer::start_async().await;
        let client =
            MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap(), EmptyCache::new_arc());

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

    #[tokio::test]
    async fn it_searches_all_chapters_in_sequence() {
        let server = MockServer::start_async().await;
        let client =
            MangadexClient::new(server.base_url().parse().unwrap(), server.base_url().parse().unwrap(), EmptyCache::new_arc());

        let manga_id = Uuid::new_v4().to_string();

        let expected_response = AggregateChapterResponse::default();

        let request = server
            .mock_async(|when, then| {
                when.method(GET)
                    .header_exists("User-Agent")
                    .path_contains("manga")
                    .path_contains(&manga_id)
                    .path_contains("/aggregate")
                    .query_param("translatedLanguage[]", "en");

                then.status(200).json_body_obj(&expected_response);
            })
            .await;

        let response = client.search_list_of_chapters(&manga_id, Languages::English).await.unwrap();

        request.assert_async().await;

        assert_eq!(expected_response, response);
    }
}
