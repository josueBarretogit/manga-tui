use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use filter_state::{InmangaFilterState, InmangaFiltersProvider};
use filter_widget::InmangaFilterWidget;
use http::header::{ACCEPT, ACCEPT_LANGUAGE, CACHE_CONTROL, REFERER, USER_AGENT};
use http::{HeaderMap, HeaderValue, StatusCode};
use manga_tui::SearchTerm;
use reqwest::Client;
use response::*;

use super::{
    Chapter, ChapterFilters, ChapterOrderBy, ChapterPageUrl, DecodeBytesToImage, FeedPageProvider, FetchChapterBookmarked,
    GetChapterPages, GetChaptersResponse, GetMangasResponse, GetRawImage, GoToReadChapter, HomePageMangaProvider, Languages,
    LatestChapter, ListOfChapters, Manga, MangaPageProvider, MangaProvider, MangaProviders, Pagination, PopularManga,
    ProviderIdentity, ReaderPageProvider, RecentlyAddedManga, SearchChapterById, SearchManga, SearchMangaById, SearchMangaPanel,
    SearchPageProvider, SortedChapters, SortedVolumes, Volumes,
};
use crate::backend::cache::{CacheDuration, Cacher, InsertEntry};
use crate::backend::database::ChapterBookmarked;
use crate::backend::manga_provider::ChapterToRead;
use crate::config::ImageQuality;

pub mod filter_state;
pub mod filter_widget;
pub mod response;

/// InManga: `https://inmanga.com/`
/// Premier Spanish manga provider.
/// - Caters to Spanish (Español / Castellano / Latino) translations
/// - Chapters list is returned as JSON via `/chapter/getall?mangaIdentification={id}`
/// - Page links and image CDN are directly accessible without Cloudflare Turnstile blocks
#[derive(Clone, Debug)]
pub struct InmangaProvider {
    client: Client,
    cache_provider: Arc<dyn Cacher>,
}

impl InmangaProvider {
    const CHAPTER_PAGE_CACHE_DURATION: CacheDuration = CacheDuration::Long;
    const HOME_PAGE_CACHE_DURATION: CacheDuration = CacheDuration::Short;
    const MANGA_PAGE_CACHE_DURATION: CacheDuration = CacheDuration::LongLong;
    const SEARCH_PAGE_CACHE_DURATION: CacheDuration = CacheDuration::VeryShort;

    pub fn new(cache_provider: Arc<dyn Cacher>) -> Self {
        let mut default_headers = HeaderMap::new();

        default_headers.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36",
            ),
        );
        default_headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
        default_headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("es-ES,es;q=0.9,en;q=0.8"));
        default_headers.insert(CACHE_CONTROL, HeaderValue::from_static("max-age=604800"));

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .default_headers(default_headers)
            .build()
            .unwrap();

        Self {
            client,
            cache_provider,
        }
    }

    async fn fetch_raw_chapters(&self, manga_id: &str) -> Result<Vec<InmangaChapterDto>, Box<dyn Error>> {
        let url = format!("{INMANGA_BASE_URL}/chapter/getall?mangaIdentification={manga_id}");
        let cache = self.cache_provider.get(&url)?;

        match cache {
            Some(cached) => {
                let raw: RawInmangaChaptersResponse = serde_json::from_slice(&cached.data)?;
                let inner: InnerInmangaChaptersResult = serde_json::from_str(&raw.data)?;
                Ok(inner.result)
            },
            None => {
                let response = self.client.get(&url).header(REFERER, INMANGA_BASE_URL).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!("Could not get chapters for manga: {manga_id}").into());
                }

                let bytes = response.bytes().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &url,
                        data: &bytes,
                        duration: Self::CHAPTER_PAGE_CACHE_DURATION,
                    })
                    .ok();

                let raw: RawInmangaChaptersResponse = serde_json::from_slice(&bytes)?;
                let inner: InnerInmangaChaptersResult = serde_json::from_str(&raw.data)?;
                Ok(inner.result)
            },
        }
    }

    async fn fetch_chapter_pages_data(&self, chapter_id: &str, manga_id: &str) -> Result<InmangaChapterPagesData, Box<dyn Error>> {
        let url = format!("{INMANGA_BASE_URL}/chapter/chapterIndexControls?identification={chapter_id}");
        let cache = self.cache_provider.get(&url)?;

        match cache {
            Some(cached) => {
                let doc = String::from_utf8(cached.data)?;
                let pages_data = InmangaChapterPagesData::parse(&doc, manga_id, chapter_id)?;
                Ok(pages_data)
            },
            None => {
                let response = self.client.get(&url).header(REFERER, INMANGA_BASE_URL).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!("Could not get chapter pages for chapter: {chapter_id}").into());
                }

                let doc = response.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &url,
                        data: doc.as_bytes(),
                        duration: Self::CHAPTER_PAGE_CACHE_DURATION,
                    })
                    .ok();

                let pages_data = InmangaChapterPagesData::parse(&doc, manga_id, chapter_id)?;
                Ok(pages_data)
            },
        }
    }

    async fn get_list_of_chapters(&self, manga_id: &str) -> Result<ListOfChapters, Box<dyn Error>> {
        let mut dtos = self.fetch_raw_chapters(manga_id).await?;
        dtos.sort_by(|a, b| a.number.unwrap_or(0.0).total_cmp(&b.number.unwrap_or(0.0)));

        let total_chapters = dtos.len() as u32;
        let chapters: Vec<super::ChapterReader> = dtos.iter().map(|d| d.to_chapter_reader()).collect();
        let sorted_chapters = SortedChapters::new(chapters);
        let volume = Volumes {
            volume: "none".to_string(),
            chapters: sorted_chapters,
        };
        let sorted_volumes = SortedVolumes::new(vec![volume]);

        Ok(ListOfChapters {
            volumes: sorted_volumes,
        })
    }

    async fn get_chapter_to_read(&self, chapter_id: &str, manga_id: &str) -> Result<ChapterToRead, Box<dyn Error>> {
        let pages_data = self.fetch_chapter_pages_data(chapter_id, manga_id).await?;
        let number = pages_data.chapter_number.parse::<f64>().unwrap_or(0.0);
        let title = format!("Capítulo {}", pages_data.chapter_number);
        let pages_url = pages_data.pages.into_iter().map(|p| p.url).collect();

        Ok(ChapterToRead {
            id: chapter_id.to_string(),
            title,
            number,
            volume_number: None,
            num_page_bookmarked: None,
            language: Languages::Spanish,
            pages_url,
        })
    }
}

impl ProviderIdentity for InmangaProvider {
    fn name(&self) -> MangaProviders {
        MangaProviders::Inmanga
    }
}

impl GetRawImage for InmangaProvider {
    async fn get_raw_image(&self, url: &str) -> Result<Bytes, Box<dyn Error>> {
        let cache = self.cache_provider.get(url)?;

        match cache {
            Some(cached) => Ok(cached.data.into()),
            None => {
                let response = self.client.get(url).header(REFERER, INMANGA_BASE_URL).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!("Could not get image from InManga: {url}").into());
                }

                let bytes = response.bytes().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: url,
                        data: &bytes,
                        duration: CacheDuration::Long,
                    })
                    .ok();

                Ok(bytes)
            },
        }
    }
}

impl DecodeBytesToImage for InmangaProvider {}

impl SearchMangaPanel for InmangaProvider {}

impl HomePageMangaProvider for InmangaProvider {
    async fn get_popular_mangas(&self) -> Result<Vec<PopularManga>, Box<dyn Error>> {
        let cache_key = "inmanga_home_popular";
        let cache = self.cache_provider.get(cache_key)?;

        match cache {
            Some(cached) => {
                let html = String::from_utf8(cached.data)?;
                let items = InmangaMangaItem::parse_consult_results(&html);
                Ok(items.into_iter().map(PopularManga::from).collect())
            },
            None => {
                let params = [
                    ("filter[generes][]", "-1"),
                    ("filter[queryString]", ""),
                    ("filter[skip]", "0"),
                    ("filter[take]", "10"),
                    ("filter[sortby]", "1"),
                    ("filter[broadcastStatus]", "0"),
                    ("filter[onlyFavorites]", "false"),
                    ("d", ""),
                ];
                let response = self
                    .client
                    .post(format!("{INMANGA_BASE_URL}/manga/getMangasConsultResult"))
                    .header("X-Requested-With", "XMLHttpRequest")
                    .form(&params)
                    .send()
                    .await?;

                let html = response.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: cache_key,
                        data: html.as_bytes(),
                        duration: Self::HOME_PAGE_CACHE_DURATION,
                    })
                    .ok();

                let items = InmangaMangaItem::parse_consult_results(&html);
                Ok(items.into_iter().map(PopularManga::from).collect())
            },
        }
    }

    async fn get_recently_added_mangas(&self) -> Result<Vec<RecentlyAddedManga>, Box<dyn Error>> {
        let cache_key = "inmanga_home_recent";
        let cache = self.cache_provider.get(cache_key)?;

        match cache {
            Some(cached) => {
                let html = String::from_utf8(cached.data)?;
                let items = InmangaMangaItem::parse_consult_results(&html);
                Ok(items.into_iter().map(RecentlyAddedManga::from).collect())
            },
            None => {
                let params = [
                    ("filter[generes][]", "-1"),
                    ("filter[queryString]", ""),
                    ("filter[skip]", "0"),
                    ("filter[take]", "10"),
                    ("filter[sortby]", "3"),
                    ("filter[broadcastStatus]", "0"),
                    ("filter[onlyFavorites]", "false"),
                    ("d", ""),
                ];
                let response = self
                    .client
                    .post(format!("{INMANGA_BASE_URL}/manga/getMangasConsultResult"))
                    .header("X-Requested-With", "XMLHttpRequest")
                    .form(&params)
                    .send()
                    .await?;

                let html = response.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: cache_key,
                        data: html.as_bytes(),
                        duration: Self::HOME_PAGE_CACHE_DURATION,
                    })
                    .ok();

                let items = InmangaMangaItem::parse_consult_results(&html);
                Ok(items.into_iter().map(RecentlyAddedManga::from).collect())
            },
        }
    }
}

impl SearchMangaById for InmangaProvider {
    async fn get_manga_by_id(&self, manga_id: &str) -> Result<Manga, Box<dyn Error>> {
        let url = format!("{INMANGA_BASE_URL}/ver/manga/any/{manga_id}");
        let cache = self.cache_provider.get(&url)?;

        match cache {
            Some(cached) => {
                let doc = String::from_utf8(cached.data)?;
                let details = InmangaMangaDetails::parse(&doc, manga_id)?;
                Ok(Manga::from(details))
            },
            None => {
                let response = self.client.get(&url).header(REFERER, INMANGA_BASE_URL).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!("Could not get details for manga: {manga_id}").into());
                }

                let doc = response.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &url,
                        data: doc.as_bytes(),
                        duration: Self::MANGA_PAGE_CACHE_DURATION,
                    })
                    .ok();

                let details = InmangaMangaDetails::parse(&doc, manga_id)?;
                Ok(Manga::from(details))
            },
        }
    }
}

impl GoToReadChapter for InmangaProvider {
    async fn read_chapter(&self, chapter_id: &str, manga_id: &str) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
        let chapter = self.get_chapter_to_read(chapter_id, manga_id).await?;
        let pages_data = self.fetch_chapter_pages_data(chapter_id, manga_id).await?;
        let resolved_manga_id = if !manga_id.trim().is_empty() { manga_id } else { &pages_data.manga_id };
        let list_of_chapters = self.get_list_of_chapters(resolved_manga_id).await?;
        Ok((chapter, list_of_chapters))
    }
}

impl GetChapterPages for InmangaProvider {
    async fn get_chapter_pages_url_with_extension(
        &self,
        chapter_id: &str,
        manga_id: &str,
        _image_quality: ImageQuality,
    ) -> Result<Vec<ChapterPageUrl>, Box<dyn Error>> {
        let data = self.fetch_chapter_pages_data(chapter_id, manga_id).await?;
        Ok(data.pages)
    }
}

impl FetchChapterBookmarked for InmangaProvider {
    async fn fetch_chapter_bookmarked(
        &self,
        chapter: ChapterBookmarked,
    ) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
        self.read_chapter(&chapter.id, &chapter.manga_id).await
    }
}

impl MangaPageProvider for InmangaProvider {
    async fn get_chapters(
        &self,
        manga_id: &str,
        filters: ChapterFilters,
        pagination: Pagination,
    ) -> Result<GetChaptersResponse, Box<dyn Error>> {
        let mut dtos = self.fetch_raw_chapters(manga_id).await?;

        match filters.order {
            ChapterOrderBy::Ascending => {
                dtos.sort_by(|a, b| a.number.unwrap_or(0.0).total_cmp(&b.number.unwrap_or(0.0)));
            },
            ChapterOrderBy::Descending => {
                dtos.sort_by(|a, b| b.number.unwrap_or(0.0).total_cmp(&a.number.unwrap_or(0.0)));
            },
        }

        let total_chapters = dtos.len() as u32;
        let start = (pagination.current_page.saturating_sub(1) * pagination.items_per_page) as usize;
        let end = (start + pagination.items_per_page as usize).min(dtos.len());

        let chapters = if start < dtos.len() { dtos[start..end].iter().map(|d| d.to_chapter(manga_id)).collect() } else { vec![] };

        Ok(GetChaptersResponse {
            chapters,
            total_chapters,
        })
    }

    async fn get_all_chapters(&self, manga_id: &str, _language: Languages) -> Result<Vec<Chapter>, Box<dyn Error>> {
        let dtos = self.fetch_raw_chapters(manga_id).await?;
        Ok(dtos.iter().map(|d| d.to_chapter(manga_id)).collect())
    }
}

impl SearchChapterById for InmangaProvider {
    async fn search_chapter(&self, chapter_id: &str, manga_id: &str) -> Result<ChapterToRead, Box<dyn Error>> {
        self.get_chapter_to_read(chapter_id, manga_id).await
    }
}

impl ReaderPageProvider for InmangaProvider {}

impl SearchPageProvider for InmangaProvider {
    type FiltersHandler = InmangaFiltersProvider;
    type InnerState = InmangaFilterState;
    type Widget = InmangaFilterWidget;

    async fn search_mangas(
        &self,
        search_term: Option<SearchTerm>,
        _filters: Self::InnerState,
        pagination: Pagination,
    ) -> Result<GetMangasResponse, Box<dyn Error>> {
        let query = search_term.map(|t| t.to_string()).unwrap_or_default();
        let skip = (pagination.current_page.saturating_sub(1)) * pagination.items_per_page;
        let take = pagination.items_per_page;

        let skip_str = skip.to_string();
        let take_str = take.to_string();
        let params = [
            ("filter[generes][]", "-1"),
            ("filter[queryString]", query.as_str()),
            ("filter[skip]", skip_str.as_str()),
            ("filter[take]", take_str.as_str()),
            ("filter[sortby]", "1"),
            ("filter[broadcastStatus]", "0"),
            ("filter[onlyFavorites]", "false"),
            ("d", ""),
        ];

        let cache_key = format!("inmanga_search_{query}_{skip}_{take}");
        let cache = self.cache_provider.get(&cache_key)?;

        let html = match cache {
            Some(cached) => String::from_utf8(cached.data)?,
            None => {
                let response = self
                    .client
                    .post(format!("{INMANGA_BASE_URL}/manga/getMangasConsultResult"))
                    .header("X-Requested-With", "XMLHttpRequest")
                    .form(&params)
                    .send()
                    .await?;

                let text = response.text().await?;

                self.cache_provider
                    .cache(InsertEntry {
                        id: &cache_key,
                        data: text.as_bytes(),
                        duration: Self::SEARCH_PAGE_CACHE_DURATION,
                    })
                    .ok();

                text
            },
        };

        let items = InmangaMangaItem::parse_consult_results(&html);
        let amount = items.len();
        let next_page = amount == take as usize;
        let total_mangas = if next_page { 1_000_000 } else { skip + amount as u32 };

        Ok(GetMangasResponse {
            mangas: items.into_iter().map(SearchManga::from).collect(),
            total_mangas,
            next_page,
        })
    }
}

impl FeedPageProvider for InmangaProvider {
    async fn get_latest_chapters(&self, manga_id: &str) -> Result<Vec<LatestChapter>, Box<dyn Error>> {
        let mut dtos = self.fetch_raw_chapters(manga_id).await?;
        dtos.sort_by(|a, b| b.number.unwrap_or(0.0).total_cmp(&a.number.unwrap_or(0.0)));

        let latest: Vec<LatestChapter> = dtos
            .into_iter()
            .take(5)
            .map(|d| {
                let friendly_num = d
                    .friendly_chapter_number
                    .clone()
                    .unwrap_or_else(|| d.number.map(|n| n.to_string()).unwrap_or_default());
                LatestChapter {
                    id: d.identification.clone(),
                    manga_id: manga_id.to_string(),
                    title: format!("Capítulo {friendly_num}"),
                    language: Languages::Spanish,
                    chapter_number: friendly_num,
                    volume_number: None,
                    publication_date: d.parse_publication_date(),
                }
            })
            .collect();

        Ok(latest)
    }
}

impl MangaProvider for InmangaProvider {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::cache::in_memory::InMemoryCache;

    #[tokio::test]
    async fn test_inmanga_live_popular() {
        let cache = InMemoryCache::init(8);
        let provider = InmangaProvider::new(cache);
        let popular = provider.get_popular_mangas().await.expect("Failed to get popular");
        assert!(!popular.is_empty(), "Popular mangas should not be empty");
    }

    #[tokio::test]
    async fn test_inmanga_live_search_and_chapters() {
        let cache = InMemoryCache::init(8);
        let provider = InmangaProvider::new(cache);
        let res = provider
            .search_mangas(SearchTerm::trimmed("Frieren"), InmangaFilterState::default(), Pagination::from_first_page(10))
            .await
            .expect("Failed to search");
        assert!(!res.mangas.is_empty(), "Search results should not be empty");
        let frieren = &res.mangas[0];
        assert!(frieren.title.to_lowercase().contains("frieren"));

        let chapters = provider
            .get_chapters(&frieren.id, ChapterFilters::default(), Pagination::from_first_page(10))
            .await
            .expect("Failed to get chapters");
        assert!(!chapters.chapters.is_empty(), "Chapters should not be empty");
        assert!(chapters.total_chapters > 100, "Frieren should have over 100 chapters");
    }
}
