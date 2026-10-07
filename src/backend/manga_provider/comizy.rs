use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use filter_state::{ComizyFilterState, ComizyFiltersProvider};
use filter_widget::ComizyFilterWidget;
use http::header::{ACCEPT, ACCEPT_LANGUAGE, CACHE_CONTROL, REFERER, USER_AGENT};
use http::{HeaderMap, HeaderValue, StatusCode};
use manga_tui::SearchTerm;
use reqwest::Client;
use response::*;

use super::{
    Chapter, ChapterFilters, ChapterOrderBy, ChapterPageUrl, ChapterReader, DecodeBytesToImage, FeedPageProvider,
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

/// Comizy: `https://comizy.io`
/// Comprehensive English manga reader platform.
/// - REST JSON API hosted at `https://api.comizy.io`
/// - High-speed WebP CDN at `cmzcdn.org`
/// - Full chapter listings, search, and home updates
#[derive(Clone, Debug)]
pub struct ComizyProvider {
    client: Client,
    cache_provider: Arc<dyn Cacher>,
}

impl ComizyProvider {
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
        default_headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
        default_headers.insert(CACHE_CONTROL, HeaderValue::from_static("max-age=604800"));
        default_headers.insert(REFERER, HeaderValue::from_static(COMIZY_BASE_URL));

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

    /// Resolves manga ID from either an sqid or a slug/URL
    async fn resolve_manga_id(&self, manga_or_slug: &str) -> Result<String, Box<dyn Error>> {
        let clean = manga_or_slug.trim().trim_matches('/');
        let slug = clean.rsplit('/').next().unwrap_or(clean);

        // If it's already an 8-character alphanumeric sqid, use it directly
        if slug.len() == 8 && slug.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Ok(slug.to_string());
        }

        // Check cache for slug -> id mapping
        let cache_key = format!("comizy_slug_to_id_{slug}");
        if let Ok(Some(cached)) = self.cache_provider.get(&cache_key) {
            if let Ok(id_str) = String::from_utf8(cached.data) {
                return Ok(id_str);
            }
        }

        // Fallback: fetch HTML from comizy.io/{slug} to extract ID from __NEXT_DATA__
        let web_url = format!("{COMIZY_BASE_URL}/{slug}");
        let response = self.client.get(&web_url).send().await?;
        if response.status() == StatusCode::OK {
            let html = response.text().await?;
            if let Some(id) = extract_id_from_next_data(&html) {
                self.cache_provider
                    .cache(InsertEntry {
                        id: &cache_key,
                        data: id.as_bytes(),
                        duration: CacheDuration::LongLong,
                    })
                    .ok();
                return Ok(id);
            }
        }

        Ok(slug.to_string())
    }

    /// Fetches manga details from `https://api.comizy.io/titles/{manga_id}`
    async fn fetch_manga_details(&self, manga_or_slug: &str) -> Result<ComizyTitleDetails, Box<dyn Error>> {
        let resolved_id = self.resolve_manga_id(manga_or_slug).await?;
        let url = format!("{COMIZY_API_URL}/titles/{resolved_id}");
        let cache = self.cache_provider.get(&url)?;

        let details: ComizyTitleDetails = match cache {
            Some(cached) => serde_json::from_slice(&cached.data)?,
            None => {
                let response = self.client.get(&url).send().await?;
                if response.status() != StatusCode::OK {
                    return Err(format!("Could not fetch Comizy manga details for: {resolved_id}").into());
                }

                let bytes = response.bytes().await?;
                let api_resp: ComizyApiResponse<ComizyTitleDetailsData> = serde_json::from_slice(&bytes)?;
                let title = api_resp
                    .data
                    .map(|d| d.title)
                    .ok_or_else(|| format!("No title data returned for: {resolved_id}"))?;

                let encoded = serde_json::to_vec(&title)?;
                self.cache_provider
                    .cache(InsertEntry {
                        id: &url,
                        data: &encoded,
                        duration: Self::MANGA_PAGE_CACHE_DURATION,
                    })
                    .ok();
                title
            },
        };

        Ok(details)
    }

    /// Fetches all chapters from `https://api.comizy.io/titles/{manga_id}/chapters`
    async fn fetch_raw_chapters(&self, manga_id: &str) -> Result<Vec<ComizyChapterItem>, Box<dyn Error>> {
        let resolved_id = self.resolve_manga_id(manga_id).await?;
        let url = format!("{COMIZY_API_URL}/titles/{resolved_id}/chapters");
        let cache = self.cache_provider.get(&url)?;

        let chapters: Vec<ComizyChapterItem> = match cache {
            Some(cached) => serde_json::from_slice(&cached.data)?,
            None => {
                let response = self.client.get(&url).send().await?;
                if response.status() != StatusCode::OK {
                    return Err(format!("Could not fetch Comizy chapters for: {resolved_id}").into());
                }

                let bytes = response.bytes().await?;
                let api_resp: ComizyApiResponse<ComizyChaptersData> = serde_json::from_slice(&bytes)?;
                let chapters = api_resp.data.map(|d| d.chapters).unwrap_or_default();

                let encoded = serde_json::to_vec(&chapters)?;
                self.cache_provider
                    .cache(InsertEntry {
                        id: &url,
                        data: &encoded,
                        duration: Self::MANGA_PAGE_CACHE_DURATION,
                    })
                    .ok();
                chapters
            },
        };

        Ok(chapters)
    }

    /// Fetches home page data
    async fn fetch_home_data(&self) -> Result<ComizyHomeData, Box<dyn Error>> {
        let cache_key = "comizy_home_data";
        let cache = self.cache_provider.get(cache_key)?;

        let home_data: ComizyHomeData = match cache {
            Some(cached) => serde_json::from_slice(&cached.data)?,
            None => {
                let url = format!("{COMIZY_API_URL}/titles/home");
                let response = self.client.get(&url).send().await?;
                if response.status() != StatusCode::OK {
                    return Err(format!("Could not fetch Comizy home page: status {}", response.status()).into());
                }

                let bytes = response.bytes().await?;
                let api_resp: ComizyApiResponse<ComizyHomeData> = serde_json::from_slice(&bytes)?;
                let data = api_resp.data.unwrap_or_default();

                let encoded = serde_json::to_vec(&data)?;
                self.cache_provider
                    .cache(InsertEntry {
                        id: cache_key,
                        data: &encoded,
                        duration: Self::HOME_PAGE_CACHE_DURATION,
                    })
                    .ok();
                data
            },
        };

        Ok(home_data)
    }

    /// Fetches chapter pages (image URLs)
    async fn fetch_chapter_pages(&self, chapter_id: &str, manga_id: &str) -> Result<Vec<ChapterPageUrl>, Box<dyn Error>> {
        let (manga_id_part, actual_chapter_id) = chapter_id.split_once('|').unwrap_or((manga_id, chapter_id));
        let resolved_manga_id = if !manga_id_part.is_empty() {
            self.resolve_manga_id(manga_id_part).await?
        } else if !manga_id.is_empty() {
            self.resolve_manga_id(manga_id).await?
        } else {
            return Err("Missing manga_id for Comizy chapter pages request".into());
        };

        let url = format!("{COMIZY_API_URL}/titles/{resolved_manga_id}/chapters/{actual_chapter_id}/images");
        let cache = self.cache_provider.get(&url)?;

        let images: Vec<String> = match cache {
            Some(cached) => serde_json::from_slice(&cached.data)?,
            None => {
                let response = self.client.get(&url).send().await?;
                if response.status() != StatusCode::OK {
                    return Err(format!("Could not fetch Comizy chapter images: {url}").into());
                }

                let bytes = response.bytes().await?;
                let api_resp: ComizyApiResponse<ComizyImagesData> = serde_json::from_slice(&bytes)?;
                let images = api_resp.data.map(|d| d.images).unwrap_or_default();

                let encoded = serde_json::to_vec(&images)?;
                self.cache_provider
                    .cache(InsertEntry {
                        id: &url,
                        data: &encoded,
                        duration: Self::CHAPTER_PAGE_CACHE_DURATION,
                    })
                    .ok();
                images
            },
        };

        parse_images_to_page_urls(&images)
    }

    /// Builds a `ChapterToRead`
    async fn get_chapter_to_read(&self, chapter_id: &str, manga_id: &str) -> Result<ChapterToRead, Box<dyn Error>> {
        let pages = self.fetch_chapter_pages(chapter_id, manga_id).await?;
        let pages_url = pages.into_iter().map(|p| p.url).collect();

        let (manga_id_part, actual_chapter_id) = chapter_id.split_once('|').unwrap_or((manga_id, chapter_id));
        let target_manga_id = if !manga_id_part.is_empty() { manga_id_part } else { manga_id };

        let raw_chapters = if !target_manga_id.is_empty() {
            self.fetch_raw_chapters(target_manga_id).await.unwrap_or_default()
        } else {
            Vec::new()
        };

        let chapter_item = raw_chapters.iter().find(|ch| ch.id == actual_chapter_id);

        let (title, number) = match chapter_item {
            Some(item) => {
                let (num, _) = parse_chapter_number(&item.name, item.slug.as_deref(), item.number.as_ref());
                (item.name.clone(), num)
            },
            None => {
                let (num, _) = parse_chapter_number(actual_chapter_id, None, None);
                (format!("Chapter {actual_chapter_id}"), num)
            },
        };

        let full_id =
            if chapter_id.contains('|') { chapter_id.to_string() } else { format!("{target_manga_id}|{actual_chapter_id}") };

        Ok(ChapterToRead {
            id: full_id,
            title,
            number,
            volume_number: None,
            num_page_bookmarked: None,
            language: Languages::English,
            pages_url,
        })
    }

    /// Builds a `ListOfChapters` from manga details
    async fn get_list_of_chapters(&self, manga_id: &str) -> Result<ListOfChapters, Box<dyn Error>> {
        let resolved_id = self.resolve_manga_id(manga_id).await?;
        let chapters = self.fetch_raw_chapters(&resolved_id).await?;
        let reader_chapters: Vec<ChapterReader> = chapters.iter().map(|c| c.to_chapter_reader(&resolved_id)).collect();
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

impl ProviderIdentity for ComizyProvider {
    fn name(&self) -> MangaProviders {
        MangaProviders::Comizy
    }
}

impl GetRawImage for ComizyProvider {
    async fn get_raw_image(&self, url: &str) -> Result<Bytes, Box<dyn Error>> {
        let cache = self.cache_provider.get(url)?;

        match cache {
            Some(cached) => Ok(cached.data.into()),
            None => {
                let response = self.client.get(url).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!("Could not get image from Comizy CDN: {url}").into());
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

impl DecodeBytesToImage for ComizyProvider {}
impl SearchMangaPanel for ComizyProvider {}

impl HomePageMangaProvider for ComizyProvider {
    async fn get_popular_mangas(&self) -> Result<Vec<PopularManga>, Box<dyn Error>> {
        let home_data = self.fetch_home_data().await?;
        let popular = home_data.popular.into_iter().map(|item| item.to_popular_manga()).collect();
        Ok(popular)
    }

    async fn get_recently_added_mangas(&self) -> Result<Vec<RecentlyAddedManga>, Box<dyn Error>> {
        let home_data = self.fetch_home_data().await?;
        let recent = home_data
            .latest
            .map(|l| l.items)
            .unwrap_or_default()
            .into_iter()
            .map(|item| item.to_recently_added_manga())
            .collect();
        Ok(recent)
    }
}

impl SearchMangaById for ComizyProvider {
    async fn get_manga_by_id(&self, manga_id: &str) -> Result<Manga, Box<dyn Error>> {
        let details = self.fetch_manga_details(manga_id).await?;
        Ok(details.to_manga())
    }
}

impl GetChapterPages for ComizyProvider {
    async fn get_chapter_pages_url_with_extension(
        &self,
        chapter_id: &str,
        manga_id: &str,
        _image_quality: ImageQuality,
    ) -> Result<Vec<ChapterPageUrl>, Box<dyn Error>> {
        self.fetch_chapter_pages(chapter_id, manga_id).await
    }
}

impl SearchChapterById for ComizyProvider {
    async fn search_chapter(&self, chapter_id: &str, manga_id: &str) -> Result<ChapterToRead, Box<dyn Error>> {
        self.get_chapter_to_read(chapter_id, manga_id).await
    }
}

impl FetchChapterBookmarked for ComizyProvider {
    async fn fetch_chapter_bookmarked(
        &self,
        chapter: ChapterBookmarked,
    ) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
        let (mut chapter_to_read, list_of_chapters) = self.read_chapter(&chapter.id, &chapter.manga_id).await?;
        chapter_to_read.num_page_bookmarked = chapter.number_page_bookmarked;
        Ok((chapter_to_read, list_of_chapters))
    }
}

impl GoToReadChapter for ComizyProvider {
    async fn read_chapter(&self, chapter_id: &str, manga_id: &str) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
        let chapter_to_read = self.get_chapter_to_read(chapter_id, manga_id).await?;
        let (manga_id_part, _) = chapter_id.split_once('|').unwrap_or((manga_id, chapter_id));
        let resolved_manga_id = if !manga_id_part.is_empty() { manga_id_part } else { manga_id };
        let list_of_chapters = self.get_list_of_chapters(resolved_manga_id).await?;

        Ok((chapter_to_read, list_of_chapters))
    }
}

impl MangaPageProvider for ComizyProvider {
    async fn get_chapters(
        &self,
        manga_id: &str,
        filters: ChapterFilters,
        pagination: Pagination,
    ) -> Result<GetChaptersResponse, Box<dyn Error>> {
        let resolved_id = self.resolve_manga_id(manga_id).await?;
        let mut raw = self.fetch_raw_chapters(&resolved_id).await?;

        match filters.order {
            ChapterOrderBy::Ascending => {
                raw.sort_by(|a, b| {
                    let (na, _) = parse_chapter_number(&a.name, a.slug.as_deref(), a.number.as_ref());
                    let (nb, _) = parse_chapter_number(&b.name, b.slug.as_deref(), b.number.as_ref());
                    na.total_cmp(&nb)
                });
            },
            ChapterOrderBy::Descending => {
                raw.sort_by(|a, b| {
                    let (na, _) = parse_chapter_number(&a.name, a.slug.as_deref(), a.number.as_ref());
                    let (nb, _) = parse_chapter_number(&b.name, b.slug.as_deref(), b.number.as_ref());
                    nb.total_cmp(&na)
                });
            },
        }

        let total_chapters = raw.len() as u32;
        let start = (pagination.current_page.saturating_sub(1) * pagination.items_per_page) as usize;
        let end = (start + pagination.items_per_page as usize).min(raw.len());

        let chapters =
            if start < raw.len() { raw[start..end].iter().map(|ch| ch.to_chapter(&resolved_id)).collect() } else { vec![] };

        Ok(GetChaptersResponse {
            chapters,
            total_chapters,
        })
    }

    async fn get_all_chapters(&self, manga_id: &str, _language: Languages) -> Result<Vec<Chapter>, Box<dyn Error>> {
        let resolved_id = self.resolve_manga_id(manga_id).await?;
        let raw = self.fetch_raw_chapters(&resolved_id).await?;
        Ok(raw.iter().map(|ch| ch.to_chapter(&resolved_id)).collect())
    }
}

impl SearchPageProvider for ComizyProvider {
    type FiltersHandler = ComizyFiltersProvider;
    type InnerState = ComizyFilterState;
    type Widget = ComizyFilterWidget;

    async fn search_mangas(
        &self,
        search_term: Option<SearchTerm>,
        _filters: Self::InnerState,
        pagination: Pagination,
    ) -> Result<GetMangasResponse, Box<dyn Error>> {
        let page = pagination.current_page;
        let limit = pagination.items_per_page;

        let query = search_term.map(|t| t.to_string()).unwrap_or_default();
        let trimmed = query.trim();

        let url = if !trimmed.is_empty() {
            let encoded = urlencoding_encode(trimmed);
            format!("{COMIZY_API_URL}/titles/search?q={encoded}&page={page}&limit={limit}")
        } else {
            format!("{COMIZY_API_URL}/titles/search?page={page}&limit={limit}")
        };

        let cache_key = format!("comizy_search_{trimmed}_{page}_{limit}");
        let cache = self.cache_provider.get(&cache_key)?;

        let search_data: ComizySearchData = match cache {
            Some(cached) => serde_json::from_slice(&cached.data)?,
            None => {
                let response = self.client.get(&url).send().await?;
                if response.status() != StatusCode::OK {
                    return Err(format!("Comizy search failed with status {}", response.status()).into());
                }

                let bytes = response.bytes().await?;
                let api_resp: ComizyApiResponse<ComizySearchData> = serde_json::from_slice(&bytes)?;
                let data = api_resp.data.unwrap_or_default();

                let encoded = serde_json::to_vec(&data)?;
                self.cache_provider
                    .cache(InsertEntry {
                        id: &cache_key,
                        data: &encoded,
                        duration: Self::SEARCH_PAGE_CACHE_DURATION,
                    })
                    .ok();
                data
            },
        };

        let total_mangas = search_data
            .pagination
            .as_ref()
            .and_then(|p| p.total)
            .unwrap_or(search_data.items.len() as u32);
        let next_page = search_data.pagination.as_ref().and_then(|p| p.has_next).unwrap_or(false);
        let mangas: Vec<SearchManga> = search_data.items.into_iter().map(|item| item.to_search_manga()).collect();

        Ok(GetMangasResponse {
            mangas,
            total_mangas,
            next_page,
        })
    }
}

impl FeedPageProvider for ComizyProvider {
    async fn get_latest_chapters(&self, manga_id: &str) -> Result<Vec<LatestChapter>, Box<dyn Error>> {
        let resolved_id = self.resolve_manga_id(manga_id).await?;
        let mut raw = self.fetch_raw_chapters(&resolved_id).await?;
        raw.sort_by(|a, b| {
            let (na, _) = parse_chapter_number(&a.name, a.slug.as_deref(), a.number.as_ref());
            let (nb, _) = parse_chapter_number(&b.name, b.slug.as_deref(), b.number.as_ref());
            nb.total_cmp(&na)
        });

        let latest: Vec<LatestChapter> = raw
            .into_iter()
            .take(5)
            .map(|ch| {
                let (_, number_str) = parse_chapter_number(&ch.name, ch.slug.as_deref(), ch.number.as_ref());
                let publication_date = parse_publication_date(ch.updated_at.as_deref().or(ch.created_at.as_deref()));
                LatestChapter {
                    id: format!("{resolved_id}|{}", ch.id),
                    manga_id: resolved_id.clone(),
                    title: ch.name,
                    language: Languages::English,
                    chapter_number: number_str,
                    volume_number: None,
                    publication_date,
                }
            })
            .collect();
        Ok(latest)
    }
}

impl ReaderPageProvider for ComizyProvider {}
impl MangaProvider for ComizyProvider {}

fn urlencoding_encode(s: &str) -> String {
    let mut encoded = String::new();
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            },
            b' ' => encoded.push('+'),
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            },
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::cache::in_memory::InMemoryCache;

    #[test]
    fn test_comizy_chapter_number_parsing() {
        let (num1, str1) = parse_chapter_number("Chapter 1194", Some("chapter-1194"), None);
        assert_eq!(num1, 1194.0);
        assert_eq!(str1, "1194");

        let (num2, str2) = parse_chapter_number("Chapter 1.2 : Romance Dawn", Some("chapter-1-2-romance-dawn"), None);
        assert_eq!(num2, 1.2);
        assert_eq!(str2, "1.2");

        let (num3, str3) = parse_chapter_number("Chapter 0", Some("chapter-0"), None);
        assert_eq!(num3, 0.0);
        assert_eq!(str3, "0");
    }

    #[test]
    fn test_comizy_chapter_navigation_consecutive() {
        let items = vec![
            ComizyChapterItem {
                id: "c1".to_string(),
                name: "Chapter 1".to_string(),
                slug: Some("chapter-1".to_string()),
                number: Some(serde_json::json!(1)),
                updated_at: None,
                created_at: None,
                url: None,
            },
            ComizyChapterItem {
                id: "c2".to_string(),
                name: "Chapter 2".to_string(),
                slug: Some("chapter-2".to_string()),
                number: Some(serde_json::json!(2)),
                updated_at: None,
                created_at: None,
                url: None,
            },
            ComizyChapterItem {
                id: "c1192".to_string(),
                name: "Chapter 1192".to_string(),
                slug: Some("chapter-1192".to_string()),
                number: Some(serde_json::json!(1307)),
                updated_at: None,
                created_at: None,
                url: None,
            },
            ComizyChapterItem {
                id: "c1193".to_string(),
                name: "Chapter 1193".to_string(),
                slug: Some("chapter-1193".to_string()),
                number: Some(serde_json::json!(1308)),
                updated_at: None,
                created_at: None,
                url: None,
            },
        ];

        let reader_chapters: Vec<ChapterReader> = items.iter().map(|c| c.to_chapter_reader("pO3vrpjr")).collect();
        let sorted = SortedChapters::new(reader_chapters);
        let list = ListOfChapters {
            volumes: SortedVolumes::new(vec![Volumes {
                volume: "none".to_string(),
                chapters: sorted,
            }]),
        };

        // When reading chapter 1192 (number = 1192.0), next must be chapter 1193
        let next = list.get_next_chapter(None, 1192.0).expect("should find next chapter");
        assert_eq!(next.id, "pO3vrpjr|c1193");
        assert_eq!(next.number, "1193");

        // When reading chapter 1193 (number = 1193.0), prev must be chapter 1192
        let prev = list.get_previous_chapter(None, 1193.0).expect("should find previous chapter");
        assert_eq!(prev.id, "pO3vrpjr|c1192");
        assert_eq!(prev.number, "1192");
    }

    #[tokio::test]
    #[ignore]
    async fn test_comizy_live_search() {
        let cache = InMemoryCache::init(10);
        let provider = ComizyProvider::new(cache);
        let result = provider
            .search_mangas(SearchTerm::trimmed("one piece"), ComizyFilterState::default(), Pagination::from_first_page(20))
            .await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(!res.mangas.is_empty());
        println!("Comizy search found: {}", res.mangas.len());
    }

    #[tokio::test]
    #[ignore]
    async fn test_comizy_live_chapters() {
        let cache = InMemoryCache::init(10);
        let provider = ComizyProvider::new(cache);
        let chapters = provider.get_all_chapters("pO3vrpjr", Languages::English).await;
        assert!(chapters.is_ok());
        let list = chapters.unwrap();
        assert!(!list.is_empty());
        println!("Comizy One Piece chapters: {}", list.len());

        let pages = provider
            .get_chapter_pages_url_with_extension(&list[0].id, "pO3vrpjr", ImageQuality::default())
            .await;
        assert!(pages.is_ok());
        let page_urls = pages.unwrap();
        assert!(!page_urls.is_empty());
        println!("Comizy chapter pages: {}", page_urls.len());

        let img = provider.get_raw_image(page_urls[0].url.as_str()).await;
        assert!(img.is_ok());
        assert!(!img.unwrap().is_empty());
    }
}
