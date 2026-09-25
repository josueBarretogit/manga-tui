use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use filter_state::{LeercapituloFilterState, LeercapituloFiltersProvider};
use filter_widget::LeercapituloFilterWidget;
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

/// LeerCapitulo: `https://www.leercapitulo.co`
/// Major Spanish manga reading platform.
/// - Caters to Spanish translations (manga, manhwa, manhua)
/// - Clean search JSON endpoint `/search/?q={query}`
/// - High-speed CDN for chapter image pages
#[derive(Clone, Debug)]
pub struct LeercapituloProvider {
    client: Client,
    cache_provider: Arc<dyn Cacher>,
}

impl LeercapituloProvider {
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

    /// Fetches manga details page HTML and parses it
    async fn fetch_manga_details(&self, manga_id: &str) -> Result<LeercapituloMangaDetails, Box<dyn Error>> {
        let clean_id = manga_id.trim_matches('/');
        let url = format!("{LEERCAPITULO_BASE_URL}/manga/{clean_id}/");
        let cache = self.cache_provider.get(&url)?;

        let html = match cache {
            Some(cached) => String::from_utf8(cached.data)?,
            None => {
                let response = self.client.get(&url).header(REFERER, LEERCAPITULO_BASE_URL).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!("Could not fetch LeerCapitulo manga page: {url}").into());
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

        Ok(parse_manga_details(&html, clean_id))
    }

    /// Fetches chapter reader page and extracts images
    async fn fetch_chapter_pages(&self, chapter_path: &str) -> Result<Vec<ChapterPageUrl>, Box<dyn Error>> {
        let url = if chapter_path.starts_with("http") {
            chapter_path.to_string()
        } else if chapter_path.starts_with('/') {
            format!("{LEERCAPITULO_BASE_URL}{chapter_path}")
        } else {
            format!("{LEERCAPITULO_BASE_URL}/{chapter_path}")
        };

        let cache = self.cache_provider.get(&url)?;

        let html = match cache {
            Some(cached) => String::from_utf8(cached.data)?,
            None => {
                let response = self.client.get(&url).header(REFERER, LEERCAPITULO_BASE_URL).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!("Could not fetch LeerCapitulo chapter reader: {url}").into());
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

        parse_reader_pages(&html)
    }

    /// Builds a `ChapterToRead`
    async fn get_chapter_to_read(&self, chapter_path: &str) -> Result<ChapterToRead, Box<dyn Error>> {
        let pages = self.fetch_chapter_pages(chapter_path).await?;
        let pages_url = pages.into_iter().map(|p| p.url).collect();

        let num = chapter_path
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);

        Ok(ChapterToRead {
            id: chapter_path.to_string(),
            title: format!("Capítulo {num}"),
            number: num,
            volume_number: None,
            num_page_bookmarked: None,
            language: Languages::Spanish,
            pages_url,
        })
    }

    async fn fetch_raw_chapters(&self, manga_id: &str) -> Result<Vec<LeercapituloChapterItem>, Box<dyn Error>> {
        let details = self.fetch_manga_details(manga_id).await?;
        Ok(details.chapters)
    }

    /// Builds a `ListOfChapters` from manga details
    async fn get_list_of_chapters(&self, manga_id: &str) -> Result<ListOfChapters, Box<dyn Error>> {
        let chapters = self.fetch_raw_chapters(manga_id).await?;
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

impl ProviderIdentity for LeercapituloProvider {
    fn name(&self) -> MangaProviders {
        MangaProviders::Leercapitulo
    }
}

impl GetRawImage for LeercapituloProvider {
    async fn get_raw_image(&self, url: &str) -> Result<Bytes, Box<dyn Error>> {
        let cache = self.cache_provider.get(url)?;

        match cache {
            Some(cached) => Ok(cached.data.into()),
            None => {
                let response = self.client.get(url).header(REFERER, LEERCAPITULO_BASE_URL).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!("Could not get image from LeerCapitulo: {url}").into());
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

impl DecodeBytesToImage for LeercapituloProvider {}

impl SearchMangaPanel for LeercapituloProvider {}

impl HomePageMangaProvider for LeercapituloProvider {
    async fn get_popular_mangas(&self) -> Result<Vec<PopularManga>, Box<dyn Error>> {
        let cache_key = "leercapitulo_home_popular";
        let cache = self.cache_provider.get(cache_key)?;

        let html = match cache {
            Some(cached) => String::from_utf8(cached.data)?,
            None => {
                let response = self.client.get(LEERCAPITULO_BASE_URL).send().await?;
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
        let cache_key = "leercapitulo_home_recent";
        let cache = self.cache_provider.get(cache_key)?;

        let html = match cache {
            Some(cached) => String::from_utf8(cached.data)?,
            None => {
                let response = self.client.get(LEERCAPITULO_BASE_URL).send().await?;
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

impl SearchMangaById for LeercapituloProvider {
    async fn get_manga_by_id(&self, manga_id: &str) -> Result<Manga, Box<dyn Error>> {
        let details = self.fetch_manga_details(manga_id).await?;
        let id_safe = manga_id.split('/').nth(1).unwrap_or(manga_id).to_string();

        Ok(Manga {
            id: manga_id.to_string(),
            id_safe_for_download: id_safe,
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

impl GetChapterPages for LeercapituloProvider {
    async fn get_chapter_pages_url_with_extension(
        &self,
        chapter_id: &str,
        _manga_id: &str,
        _image_quality: ImageQuality,
    ) -> Result<Vec<ChapterPageUrl>, Box<dyn Error>> {
        self.fetch_chapter_pages(chapter_id).await
    }
}

impl SearchChapterById for LeercapituloProvider {
    async fn search_chapter(&self, chapter_id: &str, _manga_id: &str) -> Result<ChapterToRead, Box<dyn Error>> {
        self.get_chapter_to_read(chapter_id).await
    }
}

impl FetchChapterBookmarked for LeercapituloProvider {
    async fn fetch_chapter_bookmarked(
        &self,
        chapter: ChapterBookmarked,
    ) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
        self.read_chapter(&chapter.id, &chapter.manga_id).await
    }
}

impl GoToReadChapter for LeercapituloProvider {
    async fn read_chapter(&self, chapter_id: &str, manga_id: &str) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
        let chapter_to_read = self.get_chapter_to_read(chapter_id).await?;
        let list_of_chapters = self.get_list_of_chapters(manga_id).await?;

        Ok((chapter_to_read, list_of_chapters))
    }
}

impl MangaPageProvider for LeercapituloProvider {
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

impl SearchPageProvider for LeercapituloProvider {
    type FiltersHandler = LeercapituloFiltersProvider;
    type InnerState = LeercapituloFilterState;
    type Widget = LeercapituloFilterWidget;

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

        let cache_key = format!("leercapitulo_search_{trimmed}");
        let cache = self.cache_provider.get(&cache_key)?;

        let items: Vec<LeercapituloSearchItem> = match cache {
            Some(cached) => serde_json::from_slice(&cached.data)?,
            None => {
                let encoded_query = urlencoding_encode(trimmed);
                let url = format!("{LEERCAPITULO_BASE_URL}/search/?q={encoded_query}");

                let response = self.client.get(&url).header(REFERER, LEERCAPITULO_BASE_URL).send().await?;

                if response.status() != StatusCode::OK {
                    return Err(format!("LeerCapitulo search failed with status {}", response.status()).into());
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

        let mangas: Vec<SearchManga> = items
            .into_iter()
            .map(|item| {
                let manga_id = item.uri.trim_matches('/').strip_prefix("manga/").unwrap_or(&item.uri).to_string();

                let cover_img_url = if let Some(cover) = item.cover_uri {
                    if cover.starts_with("http") { cover } else { format!("{LEERCAPITULO_BASE_URL}{cover}") }
                } else {
                    String::new()
                };

                let status = match item.status.as_deref() {
                    Some("Completed") | Some("Finalizado") => MangaStatus::Completed,
                    Some("Hiatus") | Some("Pausa") => MangaStatus::Hiatus,
                    _ => MangaStatus::Ongoing,
                };

                SearchManga {
                    id: manga_id,
                    title: item.name,
                    genres: Vec::new(),
                    description: None,
                    status: Some(status),
                    cover_img_url,
                    languages: vec![Languages::Spanish],
                    artist: None,
                    author: None,
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

impl FeedPageProvider for LeercapituloProvider {
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

impl ReaderPageProvider for LeercapituloProvider {}
impl MangaProvider for LeercapituloProvider {}

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

    #[tokio::test]
    #[ignore]
    async fn test_leercapitulo_live_search() {
        let cache = InMemoryCache::init(10);
        let provider = LeercapituloProvider::new(cache);
        let result = provider
            .search_mangas(SearchTerm::trimmed("one piece"), LeercapituloFilterState::default(), Pagination::from_first_page(30))
            .await;
        assert!(result.is_ok());
        let res = result.unwrap();
        assert!(!res.mangas.is_empty());
        println!("LeerCapitulo search found: {}", res.mangas.len());
    }

    #[tokio::test]
    #[ignore]
    async fn test_leercapitulo_live_chapters() {
        let cache = InMemoryCache::init(10);
        let provider = LeercapituloProvider::new(cache);
        let chapters = provider.get_all_chapters("psvkfbmjgo/one-piece", Languages::Spanish).await;
        assert!(chapters.is_ok());
        let list = chapters.unwrap();
        assert!(!list.is_empty());
        println!("LeerCapitulo One Piece chapters: {}", list.len());

        let pages = provider
            .get_chapter_pages_url_with_extension(&list[0].id, "psvkfbmjgo/one-piece", ImageQuality::default())
            .await;
        assert!(pages.is_ok());
        let page_urls = pages.unwrap();
        assert!(!page_urls.is_empty());
        println!("LeerCapitulo chapter pages: {}", page_urls.len());

        let img = provider.get_raw_image(page_urls[0].url.as_str()).await;
        assert!(img.is_ok());
        assert!(!img.unwrap().is_empty());
    }
}
