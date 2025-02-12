use std::error::Error;
use std::path::Path;
use std::time::Duration as StdDuration;

use api_responses::*;
use bytes::Bytes;
use chrono::Months;
use filter::{Filters, IntoParam, MangadexFilterProvider};
use filter_widget::MangadexFilterWidget;
use http::StatusCode;
use manga_tui::SearchTerm;
use reqwest::{Client, Response, Url};

use super::{
    Artist, Author, Chapter, ChapterPageUrl, ChapterToRead, DecodeBytesToImage, FeedPageProvider, FetchChapterBookmarked, Genres,
    GetChapterPages, GetChaptersResponse, GetMangasResponse, GetRawImage, GoToReadChapter, HomePageMangaProvider, Languages,
    ListOfChapters, MangaPageProvider, MangaProvider, MangaProviders, MangaStatus, PopularManga, ProviderIdentity, Rating,
    ReaderPageProvider, RecentlyAddedManga, SearchChapterById, SearchMangaById, SearchMangaPanel, SearchPageProvider,
};
use crate::backend::database::ChapterBookmarked;
use crate::config::ImageQuality;
use crate::global::APP_USER_AGENT;
use crate::view::widgets::StatefulWidgetFrame;

pub mod api_responses;
pub mod filter;
pub mod filter_widget;

pub static API_URL_BASE: &str = "https://api.mangadex.org";

pub static COVER_IMG_URL_BASE: &str = "https://uploads.mangadex.org/covers";

#[derive(Clone, Debug)]
pub struct MangadexClient {
    client: reqwest::Client,
    api_url_base: Url,
    cover_img_url_base: Url,
    image_quality: ImageQuality,
}

impl MangadexClient {
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
            .user_agent(&*APP_USER_AGENT)
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
}

impl GetRawImage for MangadexClient {
    async fn get_raw_image(&self, url: &str) -> Result<Bytes, Box<dyn Error>> {
        let response = self.client.get(url).timeout(StdDuration::from_secs(3)).send().await?;

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
                ("contentRating[]", "suggestive"),
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

        let response = self
            .client
            .get(endpoint)
            .query(&[
                ("includes[]", "cover_art"),
                ("includes[]", "artist"),
                ("includes[]", "author"),
                ("order[followedCount]", "desc"),
                ("contentRating[]", "safe"),
                ("contentRating[]", "suggestive"),
                ("hasAvailableChapters", "true"),
                ("availableTranslatedLanguage[]", language),
                ("createdAtSince", from_date.as_str()),
            ])
            .send()
            .await?;

        if response.status() != StatusCode::OK {
            return Err(format!("Could not get popular mangas on mangadex, more details about the request: {:#?}", response).into());
        }

        let response: SearchMangaResponse = response.json().await?;

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

                let mut genres: Vec<Genres> = manga.attributes.tags.into_iter().map(Genres::from).collect();

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
                    title: manga.attributes.title.into(),
                    status: Some(status),
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
            return Err(format!(
                "failed to get manga of id {manga_id} on mangadex, more details about the response: \n {:#?}",
                response
            )
            .into());
        }

        let manga: GetMangaByIdResponse = response.json().await?;

        let id = manga.data.id;

        let title = manga.data.attributes.title.into();

        let mut genres: Vec<Genres> = manga.data.attributes.tags.into_iter().map(Genres::from).collect();

        let content_rating = match manga.data.attributes.content_rating.as_str() {
            "suggestive" => Rating::Moderate,
            "pornographic" | "erotica" => Rating::Nsfw,
            _ => Rating::Normal,
        };

        genres.push(Genres::new(manga.data.attributes.content_rating, content_rating));

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

        Ok(super::Manga {
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

impl GetChapterPages for MangadexClient {
    async fn get_chapter_pages_url_with_extension(
        &self,
        chapter_id: &str,
        _manga_id: &str,
        image_quality: ImageQuality,
    ) -> Result<Vec<ChapterPageUrl>, Box<dyn Error>> {
        let endpoint = format!("{}/at-home/server/{chapter_id}", self.api_url_base);
        let response = self.client.get(endpoint).send().await?;

        if response.status() != StatusCode::OK {
            return Err(format!(
                "Could not get the pages url for chapter with id {chapter_id} on mangadex, details about the response : {:#?}",
                response
            )
            .into());
        }

        let response: ChapterPagesResponse = response.json().await?;

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

        Ok(pages_url)
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
                ("includeExternalUrl", "0"),
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
                    id: id.clone(),
                    id_safe_for_download: id,
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

    async fn get_all_chapters(&self, manga_id: &str, language: Languages) -> Result<Vec<Chapter>, Box<dyn Error>> {
        let language = language.as_iso_code();

        let endpoint = format!("{}/manga/{manga_id}/feed", self.api_url_base);

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

        let response: Vec<Chapter> = response
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
                    id: id.clone(),
                    id_safe_for_download: id,
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

        Ok(response)
    }
}

impl SearchChapterById for MangadexClient {
    async fn search_chapter(&self, chapter_id: &str, _manga_id: &str) -> Result<ChapterToRead, Box<dyn std::error::Error>> {
        let endpoint = format!("{}/chapter/{chapter_id}", self.api_url_base);

        let response = self.client.get(endpoint).send().await?;

        if response.status() != StatusCode::OK {
            return Err(format!(
                "Could not get chapter of id {chapter_id} on mangadex, details about the request: {:#?}",
                response
            )
            .into());
        }

        let response: OneChapterResponse = response.json().await?;

        let pages_url: Vec<Url> = self
            .get_chapter_pages_url_with_extension(chapter_id, "", self.image_quality)
            .await?
            .into_iter()
            .map(|page| page.url)
            .collect();

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
            pages_url,
        })
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

        let response: ChapterResponse = self
            .client
            .get(endpoint)
            .query(&[
                ("limit", "5"),
                ("offset", "0"),
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

        let chapters: Vec<super::LatestChapter> = response
            .data
            .into_iter()
            .map(|chap| {
                let id = chap.id;

                let language = Languages::try_from_iso_code(chap.attributes.translated_language.as_str()).unwrap_or_default();

                let chapter_number = chap.attributes.chapter.unwrap_or("0".to_string());
                let title = chap.attributes.title.unwrap_or("No title".to_string());
                let publication_date = chap.attributes.readable_at;
                let volume_number = chap.attributes.volume;

                super::LatestChapter {
                    id,
                    title,
                    manga_id: manga_id.to_string(),
                    language,
                    chapter_number,
                    publication_date,
                    volume_number,
                }
            })
            .collect();
        Ok(chapters)
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
