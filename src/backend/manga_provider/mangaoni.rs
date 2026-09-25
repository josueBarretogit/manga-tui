use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use filter_state::{MangaoniFilterState, MangaoniFiltersProvider};
use filter_widget::MangaoniFilterWidget;
use http::header::{ACCEPT, ACCEPT_LANGUAGE, CACHE_CONTROL, CONTENT_TYPE, REFERER, USER_AGENT};
use http::{HeaderMap, HeaderValue, StatusCode};
use manga_tui::SearchTerm;
use reqwest::Client;
use response::*;

use super::{
    Author, Chapter, ChapterFilters, ChapterOrderBy, ChapterPageUrl, ChapterReader, DecodeBytesToImage, FeedPageProvider,
    FetchChapterBookmarked, GetChapterPages, GetChaptersResponse, GetMangasResponse, GetRawImage, GoToReadChapter,
    HomePageMangaProvider, Languages, LatestChapter, ListOfChapters, Manga, MangaPageProvider, MangaProvider, MangaProviders,
    MangaStatus, Pagination, PopularManga, ProviderIdentity, ReaderPageProvider, RecentlyAddedManga, SearchChapterById,
    SearchManga, SearchMangaById, SearchMangaPanel, SearchPageProvider, SortedChapters, SortedVolumes, Volumes,
};
use crate::backend::cache::{CacheDuration, Cacher, InsertEntry};
use crate::backend::database::ChapterBookmarked;
use crate::backend::manga_provider::ChapterToRead;
use crate::config::ImageQuality;

pub mod filter_state;
pub mod filter_widget;
pub mod response;

/// MangaOni: `https://manga-oni.com`
/// Popular Spanish manga and webtoon provider.
/// - Chapters and manga in Spanish
/// - Fast direct search API via POST `/buscar` with CSRF token
/// - Reader pages with base64-encoded `unicap` image lists
#[derive(Clone, Debug)]
pub struct MangaoniProvider {
    client: Client,
    cache_provider: Arc<dyn Cacher>,
}

impl MangaoniProvider {
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
            .cookie_store(true)
            .default_headers(default_headers)
            .build()
            .unwrap();

        Self {
            client,
            cache_provider,
        }
    }

    /// Fetches the CSRF token from MangaOni homepage
    async fn fetch_csrf_token(&self) -> Result<String, Box<dyn Error>> {
        let response = self.client.get(MANGAONI_BASE_URL).header(REFERER, MANGAONI_BASE_URL).send().await?;

        let html = response.text().await?;
        let token_pattern = r#"<meta[^>]*name=["']csrf-token["'][^>]*content=["']([^"']+)["']"#;
        if let Ok(re) = regex::Regex::new(token_pattern) {
            if let Some(cap) = re.captures(&html) {
                if let Some(token) = cap.get(1) {
                    return Ok(token.as_str().to_string());
                }
            }
        }
        Err("Could not extract CSRF token from MangaOni".into())
    }

    /// Fetches and parses a manga's details page
    async fn fetch_manga_details(&self, slug: &str) -> Result<MangaoniMangaDetails, Box<dyn Error>> {
        let url = format!("{MANGAONI_BASE_URL}/manga/{slug}/");
        let cache = self.cache_provider.get(&url)?;

        let html = match cache {
            Some(cached) => String::from_utf8(cached.data)?,
            None => {
                let response = self.client.get(&url).header(REFERER, MANGAONI_BASE_URL).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!("Could not fetch manga page: {slug}").into());
                }

                let text = response.text().await?;
                self.cache_provider
                    .cache(InsertEntry {
                        id: &url,
                        data: text.as_bytes(),
                        duration: Self::MANGA_PAGE_CACHE_DURATION,
                    })
                    .ok();
                text
            },
        };

        Ok(parse_manga_details(&html, slug))
    }

    /// Fetches chapter pages
    async fn fetch_chapter_pages(&self, slug: &str, chapter_id: &str) -> Result<Vec<ChapterPageUrl>, Box<dyn Error>> {
        let url = format!("{MANGAONI_BASE_URL}/lector/{slug}/{chapter_id}/");
        let cache = self.cache_provider.get(&url)?;

        let html = match cache {
            Some(cached) => String::from_utf8(cached.data)?,
            None => {
                let response = self
                    .client
                    .get(&url)
                    .header(REFERER, format!("{MANGAONI_BASE_URL}/manga/{slug}/"))
                    .send()
                    .await?;

                if response.status() != StatusCode::OK {
                    return Err(format!("Could not fetch chapter reader: {url}").into());
                }

                let text = response.text().await?;
                self.cache_provider
                    .cache(InsertEntry {
                        id: &url,
                        data: text.as_bytes(),
                        duration: Self::CHAPTER_PAGE_CACHE_DURATION,
                    })
                    .ok();
                text
            },
        };

        parse_unicap(&html)
    }

    /// Builds a `ChapterToRead`
    async fn get_chapter_to_read(&self, chapter_id_compound: &str) -> Result<ChapterToRead, Box<dyn Error>> {
        let (slug, chap_id) = chapter_id_compound.split_once('|').unwrap_or(("", chapter_id_compound));

        let pages = self.fetch_chapter_pages(slug, chap_id).await?;
        let pages_url = pages.into_iter().map(|p| p.url).collect();

        Ok(ChapterToRead {
            id: chapter_id_compound.to_string(),
            title: format!("Capítulo {chap_id}"),
            number: chap_id.parse::<f64>().unwrap_or(0.0),
            volume_number: None,
            num_page_bookmarked: None,
            language: Languages::Spanish,
            pages_url,
        })
    }

    async fn fetch_raw_chapters(&self, slug: &str) -> Result<Vec<MangaoniChapterItem>, Box<dyn Error>> {
        let details = self.fetch_manga_details(slug).await?;
        Ok(details.chapters)
    }

    /// Builds a `ListOfChapters` from manga details
    async fn get_list_of_chapters(&self, slug: &str) -> Result<ListOfChapters, Box<dyn Error>> {
        let chapters = self.fetch_raw_chapters(slug).await?;
        let reader_chapters: Vec<ChapterReader> = chapters.iter().map(|c| c.to_chapter_reader()).collect();
        let sorted_chapters = SortedChapters::new(reader_chapters);
        let volume = Volumes {
            volume: "none".to_string(),
            chapters: sorted_chapters,
        };
        Ok(ListOfChapters {
            volumes: SortedVolumes::new(vec![volume]),
        })
    }
}

impl ProviderIdentity for MangaoniProvider {
    fn name(&self) -> MangaProviders {
        MangaProviders::Mangaoni
    }
}

impl GetRawImage for MangaoniProvider {
    async fn get_raw_image(&self, url: &str) -> Result<Bytes, Box<dyn Error>> {
        let cache = self.cache_provider.get(url)?;

        match cache {
            Some(cached) => Ok(cached.data.into()),
            None => {
                let response = self.client.get(url).header(REFERER, MANGAONI_BASE_URL).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!("Could not get image from MangaOni: {url}").into());
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

impl DecodeBytesToImage for MangaoniProvider {}

impl SearchMangaPanel for MangaoniProvider {}

impl HomePageMangaProvider for MangaoniProvider {
    async fn get_popular_mangas(&self) -> Result<Vec<PopularManga>, Box<dyn Error>> {
        let cache_key = "mangaoni_home_popular";
        let cache = self.cache_provider.get(cache_key)?;

        let html = match cache {
            Some(cached) => String::from_utf8(cached.data)?,
            None => {
                let response = self.client.get(MANGAONI_BASE_URL).send().await?;
                let text = response.text().await?;
                self.cache_provider
                    .cache(InsertEntry {
                        id: cache_key,
                        data: text.as_bytes(),
                        duration: Self::HOME_PAGE_CACHE_DURATION,
                    })
                    .ok();
                text
            },
        };

        Ok(parse_home_popular(&html))
    }

    async fn get_recently_added_mangas(&self) -> Result<Vec<RecentlyAddedManga>, Box<dyn Error>> {
        let cache_key = "mangaoni_home_recent";
        let cache = self.cache_provider.get(cache_key)?;

        let html = match cache {
            Some(cached) => String::from_utf8(cached.data)?,
            None => {
                let response = self.client.get(MANGAONI_BASE_URL).send().await?;
                let text = response.text().await?;
                self.cache_provider
                    .cache(InsertEntry {
                        id: cache_key,
                        data: text.as_bytes(),
                        duration: Self::HOME_PAGE_CACHE_DURATION,
                    })
                    .ok();
                text
            },
        };

        Ok(parse_home_recent(&html))
    }
}

impl SearchMangaById for MangaoniProvider {
    async fn get_manga_by_id(&self, manga_id: &str) -> Result<Manga, Box<dyn Error>> {
        let details = self.fetch_manga_details(manga_id).await?;

        Ok(Manga {
            id: manga_id.to_string(),
            id_safe_for_download: manga_id.to_string(),
            title: details.title,
            genres: details.genres,
            description: details.description,
            status: details.status,
            cover_img_url: details.cover_img_url,
            languages: vec![Languages::Spanish],
            rating: String::new(),
            artist: None,
            author: None,
        })
    }
}

impl GetChapterPages for MangaoniProvider {
    async fn get_chapter_pages_url_with_extension(
        &self,
        chapter_id: &str,
        manga_id: &str,
        _image_quality: ImageQuality,
    ) -> Result<Vec<ChapterPageUrl>, Box<dyn Error>> {
        let (slug, chap_id) = chapter_id.split_once('|').unwrap_or((manga_id, chapter_id));

        self.fetch_chapter_pages(slug, chap_id).await
    }
}

impl SearchChapterById for MangaoniProvider {
    async fn search_chapter(&self, chapter_id: &str, _manga_id: &str) -> Result<ChapterToRead, Box<dyn Error>> {
        self.get_chapter_to_read(chapter_id).await
    }
}

impl FetchChapterBookmarked for MangaoniProvider {
    async fn fetch_chapter_bookmarked(
        &self,
        chapter: ChapterBookmarked,
    ) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
        self.read_chapter(&chapter.id, &chapter.manga_id).await
    }
}

impl GoToReadChapter for MangaoniProvider {
    async fn read_chapter(&self, chapter_id: &str, manga_id: &str) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
        let (slug, _) = chapter_id.split_once('|').unwrap_or((manga_id, chapter_id));

        let chapter_to_read = self.get_chapter_to_read(chapter_id).await?;
        let list_of_chapters = self.get_list_of_chapters(slug).await?;

        Ok((chapter_to_read, list_of_chapters))
    }
}

impl MangaPageProvider for MangaoniProvider {
    async fn get_chapters(
        &self,
        manga_id: &str,
        filters: ChapterFilters,
        pagination: Pagination,
    ) -> Result<GetChaptersResponse, Box<dyn Error>> {
        let mut raw = self.fetch_raw_chapters(manga_id).await?;

        match filters.order {
            ChapterOrderBy::Ascending => {
                raw.sort_by(|a, b| a.number.total_cmp(&b.number));
            },
            ChapterOrderBy::Descending => {
                raw.sort_by(|a, b| b.number.total_cmp(&a.number));
            },
        }

        let total_chapters = raw.len() as u32;
        let start = (pagination.current_page.saturating_sub(1) * pagination.items_per_page) as usize;
        let end = (start + pagination.items_per_page as usize).min(raw.len());

        let chapters = if start < raw.len() { raw[start..end].iter().map(|ch| ch.to_chapter(manga_id)).collect() } else { vec![] };

        Ok(GetChaptersResponse {
            chapters,
            total_chapters,
        })
    }

    async fn get_all_chapters(&self, manga_id: &str, _language: Languages) -> Result<Vec<Chapter>, Box<dyn Error>> {
        let raw = self.fetch_raw_chapters(manga_id).await?;
        Ok(raw.iter().map(|ch| ch.to_chapter(manga_id)).collect())
    }
}

impl SearchPageProvider for MangaoniProvider {
    type FiltersHandler = MangaoniFiltersProvider;
    type InnerState = MangaoniFilterState;
    type Widget = MangaoniFilterWidget;

    async fn search_mangas(
        &self,
        search_term: Option<SearchTerm>,
        _filters: Self::InnerState,
        _pagination: Pagination,
    ) -> Result<GetMangasResponse, Box<dyn Error>> {
        let query = search_term.map(|t| t.to_string()).unwrap_or_default();
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(GetMangasResponse {
                mangas: Vec::new(),
                total_mangas: 0,
                next_page: false,
            });
        }

        let cache_key = format!("mangaoni_search_{trimmed}");
        let cache = self.cache_provider.get(&cache_key)?;

        let search_response: MangaoniSearchResponse = match cache {
            Some(cached) => serde_json::from_slice(&cached.data)?,
            None => {
                let csrf_token = self.fetch_csrf_token().await?;
                let search_url = format!("{MANGAONI_BASE_URL}/buscar");

                let response = self
                    .client
                    .post(&search_url)
                    .header(REFERER, format!("{MANGAONI_BASE_URL}/"))
                    .header("X-CSRF-TOKEN", &csrf_token)
                    .header("X-Requested-With", "XMLHttpRequest")
                    .header(CONTENT_TYPE, "application/x-www-form-urlencoded; charset=UTF-8")
                    .form(&[("buscar", trimmed), ("_token", &csrf_token)])
                    .send()
                    .await?;

                if response.status() != StatusCode::OK {
                    return Err(format!("MangaOni search failed with status {}", response.status()).into());
                }

                let bytes = response.bytes().await?;
                self.cache_provider
                    .cache(InsertEntry {
                        id: &cache_key,
                        data: &bytes,
                        duration: Self::SEARCH_PAGE_CACHE_DURATION,
                    })
                    .ok();

                serde_json::from_slice(&bytes)?
            },
        };

        let mangas: Vec<SearchManga> = search_response
            .mangas
            .into_iter()
            .map(|item| {
                let slug = if !item.slug.is_empty() {
                    item.slug
                } else if let Some(url) = &item.url {
                    url.trim_end_matches('/').rsplit('/').next().unwrap_or("").to_string()
                } else {
                    item.nombre.to_lowercase().replace(' ', "-")
                };

                let cover = item.img.unwrap_or_default();

                SearchManga {
                    id: slug,
                    title: item.nombre,
                    genres: Vec::new(),
                    description: None,
                    status: Some(MangaStatus::Ongoing),
                    cover_img_url: cover,
                    languages: vec![Languages::Spanish],
                    artist: None,
                    author: item.autor.map(|name| Author {
                        id: String::new(),
                        name,
                    }),
                }
            })
            .collect();

        let total_mangas = mangas.len() as u32;

        Ok(GetMangasResponse {
            mangas,
            total_mangas,
            next_page: false,
        })
    }
}

impl FeedPageProvider for MangaoniProvider {
    async fn get_latest_chapters(&self, manga_id: &str) -> Result<Vec<LatestChapter>, Box<dyn Error>> {
        let mut raw = self.fetch_raw_chapters(manga_id).await?;
        raw.sort_by(|a, b| b.number.total_cmp(&a.number));
        let latest: Vec<LatestChapter> = raw
            .into_iter()
            .take(5)
            .map(|ch| LatestChapter {
                id: ch.id,
                manga_id: manga_id.to_string(),
                title: ch.title,
                language: Languages::Spanish,
                chapter_number: ch.number_str,
                volume_number: None,
                publication_date: ch.publication_date,
            })
            .collect();
        Ok(latest)
    }
}

impl ReaderPageProvider for MangaoniProvider {}
impl MangaProvider for MangaoniProvider {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::cache::in_memory::InMemoryCache;

    #[tokio::test]
    #[ignore]
    async fn test_mangaoni_live_search() {
        let cache = InMemoryCache::init(10);
        let provider = MangaoniProvider::new(cache);
        let result = provider
            .search_mangas(SearchTerm::trimmed("one piece"), MangaoniFilterState::default(), Pagination::from_first_page(30))
            .await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(!res.mangas.is_empty());
        println!("MangaOni search found: {}", res.mangas.len());
    }

    #[tokio::test]
    #[ignore]
    async fn test_mangaoni_live_chapters() {
        let cache = InMemoryCache::init(10);
        let provider = MangaoniProvider::new(cache);
        let chapters = provider.get_all_chapters("one-piece", Languages::Spanish).await;
        assert!(chapters.is_ok());
        let list = chapters.unwrap();
        assert!(!list.is_empty());
        println!("MangaOni One Piece chapters: {}", list.len());

        let pages = provider
            .get_chapter_pages_url_with_extension(&list[0].id, "one-piece", ImageQuality::default())
            .await;
        assert!(pages.is_ok());
        let page_urls = pages.unwrap();
        assert!(!page_urls.is_empty());
        println!("MangaOni chapter pages: {}", page_urls.len());

        let img = provider.get_raw_image(page_urls[0].url.as_str()).await;
        assert!(img.is_ok());
        assert!(!img.unwrap().is_empty());
    }
}
