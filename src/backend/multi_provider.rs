use std::error::Error;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use bytes::Bytes;
use image::DynamicImage;
use manga_tui::SearchTerm;
use reqwest::Url;

use crate::backend::cache::Cacher;
use crate::backend::cache::in_memory::InMemoryCache;
use crate::backend::database::ChapterBookmarked;
use crate::backend::manga_provider::comizy::ComizyProvider;
use crate::backend::manga_provider::comizy::filter_state::ComizyFilterState;
use crate::backend::manga_provider::inmanga::InmangaProvider;
use crate::backend::manga_provider::inmanga::filter_state::InmangaFilterState;
use crate::backend::manga_provider::leercapitulo::LeercapituloProvider;
use crate::backend::manga_provider::leercapitulo::filter_state::LeercapituloFilterState;
use crate::backend::manga_provider::mangadex::filters::api_parameter::{ContentRating, Filters, SortBy};
use crate::backend::manga_provider::mangadex::{API_URL_BASE, COVER_IMG_URL_BASE, MangadexClient};
use crate::backend::manga_provider::mangaoni::MangaoniProvider;
use crate::backend::manga_provider::mangaoni::filter_state::MangaoniFilterState;
use crate::backend::manga_provider::mangapill::MangaPillProvider;
use crate::backend::manga_provider::mangapill::filter_state::MangaPillFilterState;
use crate::backend::manga_provider::weebcentral::filter_state::WeebcentralFilterState;
use crate::backend::manga_provider::weebcentral::{WEEBCENTRAL_BASE_URL, WeebcentralProvider};
use crate::backend::manga_provider::{
    Chapter, ChapterFilters, ChapterPage, ChapterToRead, DecodeBytesToImage, FeedPageProvider, FetchChapterBookmarked,
    GetChapterPages, GetChaptersResponse, GetRawImage, GoToReadChapter, Languages, LatestChapter, ListOfChapters, Manga,
    MangaPageProvider, MangaPanel, MangaProviders, Pagination, SearchChapterById, SearchManga, SearchMangaById, SearchMangaPanel,
    SearchPageProvider,
};
use crate::config::{ImageQuality, MangaTuiConfig};

pub struct MultiProviderPool {
    pub cache: Arc<dyn Cacher>,
    pub mangadex: MangadexClient,
    pub inmanga: InmangaProvider,
    pub leercapitulo: LeercapituloProvider,
    pub mangaoni: MangaoniProvider,
    pub weebcentral: WeebcentralProvider,
    pub mangapill: MangaPillProvider,
    pub comizy: ComizyProvider,
}

impl MultiProviderPool {
    pub fn new() -> Self {
        let cache: Arc<dyn Cacher> = InMemoryCache::init(16);
        let config = MangaTuiConfig::get();
        Self {
            mangadex: MangadexClient::new(API_URL_BASE.parse().unwrap(), COVER_IMG_URL_BASE.parse().unwrap(), Arc::clone(&cache))
                .with_image_quality(config.image_quality),
            inmanga: InmangaProvider::new(Arc::clone(&cache)),
            leercapitulo: LeercapituloProvider::new(Arc::clone(&cache)),
            mangaoni: MangaoniProvider::new(Arc::clone(&cache)),
            weebcentral: WeebcentralProvider::new(WEEBCENTRAL_BASE_URL.parse().unwrap(), Arc::clone(&cache)),
            mangapill: MangaPillProvider::init(Arc::clone(&cache)),
            comizy: ComizyProvider::new(Arc::clone(&cache)),
            cache,
        }
    }
}

pub static POOL: LazyLock<MultiProviderPool> = LazyLock::new(MultiProviderPool::new);

/// Search concurrently across all available manga providers
pub async fn search_all_providers(query: &str) -> Vec<SearchManga> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    let pool = &*POOL;
    let timeout_duration = Duration::from_secs(8);

    let (inmanga_res, mangaoni_res, leercapitulo_res, mangadex_res, weebcentral_res, mangapill_res, comizy_res) = tokio::join!(
        // InManga
        async {
            let term = SearchTerm::trimmed_lowercased(query);
            tokio::time::timeout(
                timeout_duration,
                pool.inmanga.search_mangas(term, InmangaFilterState::default(), Pagination::default()),
            )
            .await
            .ok()
            .and_then(|r| r.ok())
        },
        // MangaOni
        async {
            let term = SearchTerm::trimmed_lowercased(query);
            tokio::time::timeout(
                timeout_duration,
                pool.mangaoni.search_mangas(term, MangaoniFilterState::default(), Pagination::default()),
            )
            .await
            .ok()
            .and_then(|r| r.ok())
        },
        // LeerCapítulo
        async {
            let term = SearchTerm::trimmed_lowercased(query);
            tokio::time::timeout(
                timeout_duration,
                pool.leercapitulo
                    .search_mangas(term, LeercapituloFilterState::default(), Pagination::default()),
            )
            .await
            .ok()
            .and_then(|r| r.ok())
        },
        // MangaDex
        async {
            let term = SearchTerm::trimmed_lowercased(query);
            let mut filters = Filters::default();
            filters.set_sort_by(SortBy::BestMatch);
            filters.set_content_rating(vec![ContentRating::Safe, ContentRating::Suggestive]);
            tokio::time::timeout(timeout_duration, pool.mangadex.search_mangas(term, filters, Pagination::default()))
                .await
                .ok()
                .and_then(|r| r.ok())
        },
        // WeebCentral
        async {
            let term = SearchTerm::trimmed_lowercased(query);
            tokio::time::timeout(
                timeout_duration,
                pool.weebcentral
                    .search_mangas(term, WeebcentralFilterState::default(), Pagination::default()),
            )
            .await
            .ok()
            .and_then(|r| r.ok())
        },
        // MangaPill
        async {
            let term = SearchTerm::trimmed_lowercased(query);
            tokio::time::timeout(
                timeout_duration,
                pool.mangapill.search_mangas(term, MangaPillFilterState::default(), Pagination::default()),
            )
            .await
            .ok()
            .and_then(|r| r.ok())
        },
        // Comizy
        async {
            let term = SearchTerm::trimmed_lowercased(query);
            tokio::time::timeout(
                timeout_duration,
                pool.comizy.search_mangas(term, ComizyFilterState::default(), Pagination::default()),
            )
            .await
            .ok()
            .and_then(|r| r.ok())
        },
    );

    let mut combined = Vec::new();

    // Spanish providers first for Spanish user priority, then international
    if let Some(res) = inmanga_res {
        for mut manga in res.mangas {
            manga.provider = Some(MangaProviders::Inmanga);
            combined.push(manga);
        }
    }
    if let Some(res) = mangaoni_res {
        for mut manga in res.mangas {
            manga.provider = Some(MangaProviders::Mangaoni);
            combined.push(manga);
        }
    }
    if let Some(res) = leercapitulo_res {
        for mut manga in res.mangas {
            manga.provider = Some(MangaProviders::Leercapitulo);
            combined.push(manga);
        }
    }
    if let Some(res) = mangadex_res {
        for mut manga in res.mangas {
            manga.provider = Some(MangaProviders::Mangadex);
            combined.push(manga);
        }
    }
    if let Some(res) = weebcentral_res {
        for mut manga in res.mangas {
            manga.provider = Some(MangaProviders::Weebcentral);
            combined.push(manga);
        }
    }
    if let Some(res) = mangapill_res {
        for mut manga in res.mangas {
            manga.provider = Some(MangaProviders::Mangapill);
            combined.push(manga);
        }
    }
    if let Some(res) = comizy_res {
        for mut manga in res.mangas {
            manga.provider = Some(MangaProviders::Comizy);
            combined.push(manga);
        }
    }

    combined
}

pub async fn get_manga_by_id_for_provider(provider: MangaProviders, manga_id: &str) -> Result<Manga, Box<dyn Error + Send + Sync>> {
    let pool = &*POOL;
    let mut manga = match provider {
        MangaProviders::Mangadex => pool.mangadex.get_manga_by_id(manga_id).await.map_err(|e| e.to_string())?,
        MangaProviders::Inmanga => pool.inmanga.get_manga_by_id(manga_id).await.map_err(|e| e.to_string())?,
        MangaProviders::Leercapitulo => pool.leercapitulo.get_manga_by_id(manga_id).await.map_err(|e| e.to_string())?,
        MangaProviders::Mangaoni => pool.mangaoni.get_manga_by_id(manga_id).await.map_err(|e| e.to_string())?,
        MangaProviders::Weebcentral => pool.weebcentral.get_manga_by_id(manga_id).await.map_err(|e| e.to_string())?,
        MangaProviders::Mangapill => pool.mangapill.get_manga_by_id(manga_id).await.map_err(|e| e.to_string())?,
        MangaProviders::Comizy => pool.comizy.get_manga_by_id(manga_id).await.map_err(|e| e.to_string())?,
    };
    manga.provider = Some(provider);
    Ok(manga)
}

pub async fn get_chapters_for_provider(
    provider: MangaProviders,
    manga_id: &str,
    chapter_filters: ChapterFilters,
    pagination: Pagination,
) -> Result<GetChaptersResponse, Box<dyn Error>> {
    let pool = &*POOL;
    match provider {
        MangaProviders::Mangadex => pool.mangadex.get_chapters(manga_id, chapter_filters, pagination).await,
        MangaProviders::Inmanga => pool.inmanga.get_chapters(manga_id, chapter_filters, pagination).await,
        MangaProviders::Leercapitulo => pool.leercapitulo.get_chapters(manga_id, chapter_filters, pagination).await,
        MangaProviders::Mangaoni => pool.mangaoni.get_chapters(manga_id, chapter_filters, pagination).await,
        MangaProviders::Weebcentral => pool.weebcentral.get_chapters(manga_id, chapter_filters, pagination).await,
        MangaProviders::Mangapill => pool.mangapill.get_chapters(manga_id, chapter_filters, pagination).await,
        MangaProviders::Comizy => pool.comizy.get_chapters(manga_id, chapter_filters, pagination).await,
    }
}

pub async fn get_all_chapters_for_provider(
    provider: MangaProviders,
    manga_id: &str,
    lang: Languages,
) -> Result<Vec<Chapter>, Box<dyn Error>> {
    let pool = &*POOL;
    match provider {
        MangaProviders::Mangadex => pool.mangadex.get_all_chapters(manga_id, lang).await,
        MangaProviders::Inmanga => pool.inmanga.get_all_chapters(manga_id, lang).await,
        MangaProviders::Leercapitulo => pool.leercapitulo.get_all_chapters(manga_id, lang).await,
        MangaProviders::Mangaoni => pool.mangaoni.get_all_chapters(manga_id, lang).await,
        MangaProviders::Weebcentral => pool.weebcentral.get_all_chapters(manga_id, lang).await,
        MangaProviders::Mangapill => pool.mangapill.get_all_chapters(manga_id, lang).await,
        MangaProviders::Comizy => pool.comizy.get_all_chapters(manga_id, lang).await,
    }
}

pub async fn get_latest_chapters_for_provider(
    provider: MangaProviders,
    manga_id: &str,
) -> Result<Vec<LatestChapter>, Box<dyn Error>> {
    let pool = &*POOL;
    match provider {
        MangaProviders::Mangadex => pool.mangadex.get_latest_chapters(manga_id).await,
        MangaProviders::Inmanga => pool.inmanga.get_latest_chapters(manga_id).await,
        MangaProviders::Leercapitulo => pool.leercapitulo.get_latest_chapters(manga_id).await,
        MangaProviders::Mangaoni => pool.mangaoni.get_latest_chapters(manga_id).await,
        MangaProviders::Weebcentral => pool.weebcentral.get_latest_chapters(manga_id).await,
        MangaProviders::Mangapill => pool.mangapill.get_latest_chapters(manga_id).await,
        MangaProviders::Comizy => pool.comizy.get_latest_chapters(manga_id).await,
    }
}

pub async fn read_chapter_for_provider(
    provider: MangaProviders,
    chapter_id: &str,
    manga_id: &str,
) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
    let pool = &*POOL;
    match provider {
        MangaProviders::Mangadex => pool.mangadex.read_chapter(chapter_id, manga_id).await,
        MangaProviders::Inmanga => pool.inmanga.read_chapter(chapter_id, manga_id).await,
        MangaProviders::Leercapitulo => pool.leercapitulo.read_chapter(chapter_id, manga_id).await,
        MangaProviders::Mangaoni => pool.mangaoni.read_chapter(chapter_id, manga_id).await,
        MangaProviders::Weebcentral => pool.weebcentral.read_chapter(chapter_id, manga_id).await,
        MangaProviders::Mangapill => pool.mangapill.read_chapter(chapter_id, manga_id).await,
        MangaProviders::Comizy => pool.comizy.read_chapter(chapter_id, manga_id).await,
    }
}

pub async fn search_chapter_for_provider(
    provider: MangaProviders,
    chapter_id: &str,
    manga_id: &str,
) -> Result<ChapterToRead, Box<dyn Error>> {
    let pool = &*POOL;
    match provider {
        MangaProviders::Mangadex => pool.mangadex.search_chapter(chapter_id, manga_id).await,
        MangaProviders::Inmanga => pool.inmanga.search_chapter(chapter_id, manga_id).await,
        MangaProviders::Leercapitulo => pool.leercapitulo.search_chapter(chapter_id, manga_id).await,
        MangaProviders::Mangaoni => pool.mangaoni.search_chapter(chapter_id, manga_id).await,
        MangaProviders::Weebcentral => pool.weebcentral.search_chapter(chapter_id, manga_id).await,
        MangaProviders::Mangapill => pool.mangapill.search_chapter(chapter_id, manga_id).await,
        MangaProviders::Comizy => pool.comizy.search_chapter(chapter_id, manga_id).await,
    }
}

pub async fn get_chapter_pages_for_provider<F: Fn(f64, &str) + 'static + Send>(
    provider: MangaProviders,
    chapter_id: &str,
    manga_id: &str,
    quality: ImageQuality,
    progress: F,
) -> Result<Vec<ChapterPage>, Box<dyn Error>> {
    let pool = &*POOL;
    match provider {
        MangaProviders::Mangadex => pool.mangadex.get_chapter_pages(chapter_id, manga_id, quality, progress).await,
        MangaProviders::Inmanga => pool.inmanga.get_chapter_pages(chapter_id, manga_id, quality, progress).await,
        MangaProviders::Leercapitulo => pool.leercapitulo.get_chapter_pages(chapter_id, manga_id, quality, progress).await,
        MangaProviders::Mangaoni => pool.mangaoni.get_chapter_pages(chapter_id, manga_id, quality, progress).await,
        MangaProviders::Weebcentral => pool.weebcentral.get_chapter_pages(chapter_id, manga_id, quality, progress).await,
        MangaProviders::Mangapill => pool.mangapill.get_chapter_pages(chapter_id, manga_id, quality, progress).await,
        MangaProviders::Comizy => pool.comizy.get_chapter_pages(chapter_id, manga_id, quality, progress).await,
    }
}

pub async fn search_manga_panel_for_provider(provider: MangaProviders, endpoint: Url) -> Result<MangaPanel, Box<dyn Error>> {
    let pool = &*POOL;
    match provider {
        MangaProviders::Mangadex => pool.mangadex.search_manga_panel(endpoint).await,
        MangaProviders::Inmanga => pool.inmanga.search_manga_panel(endpoint).await,
        MangaProviders::Leercapitulo => pool.leercapitulo.search_manga_panel(endpoint).await,
        MangaProviders::Mangaoni => pool.mangaoni.search_manga_panel(endpoint).await,
        MangaProviders::Weebcentral => pool.weebcentral.search_manga_panel(endpoint).await,
        MangaProviders::Mangapill => pool.mangapill.search_manga_panel(endpoint).await,
        MangaProviders::Comizy => pool.comizy.search_manga_panel(endpoint).await,
    }
}

pub async fn get_raw_image_for_provider(provider: MangaProviders, url: &str) -> Result<Bytes, Box<dyn Error>> {
    let pool = &*POOL;
    match provider {
        MangaProviders::Mangadex => pool.mangadex.get_raw_image(url).await,
        MangaProviders::Inmanga => pool.inmanga.get_raw_image(url).await,
        MangaProviders::Leercapitulo => pool.leercapitulo.get_raw_image(url).await,
        MangaProviders::Mangaoni => pool.mangaoni.get_raw_image(url).await,
        MangaProviders::Weebcentral => pool.weebcentral.get_raw_image(url).await,
        MangaProviders::Mangapill => pool.mangapill.get_raw_image(url).await,
        MangaProviders::Comizy => pool.comizy.get_raw_image(url).await,
    }
}

pub async fn get_image_for_provider(provider: MangaProviders, url: &str) -> Result<DynamicImage, Box<dyn Error + Send + Sync>> {
    let pool = &*POOL;
    match provider {
        MangaProviders::Mangadex => pool.mangadex.get_image(url).await.map_err(|e| e.to_string().into()),
        MangaProviders::Inmanga => pool.inmanga.get_image(url).await.map_err(|e| e.to_string().into()),
        MangaProviders::Leercapitulo => pool.leercapitulo.get_image(url).await.map_err(|e| e.to_string().into()),
        MangaProviders::Mangaoni => pool.mangaoni.get_image(url).await.map_err(|e| e.to_string().into()),
        MangaProviders::Weebcentral => pool.weebcentral.get_image(url).await.map_err(|e| e.to_string().into()),
        MangaProviders::Mangapill => pool.mangapill.get_image(url).await.map_err(|e| e.to_string().into()),
        MangaProviders::Comizy => pool.comizy.get_image(url).await.map_err(|e| e.to_string().into()),
    }
}

pub async fn fetch_chapter_bookmarked_for_provider(
    provider: MangaProviders,
    bookmark: ChapterBookmarked,
) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
    let pool = &*POOL;
    match provider {
        MangaProviders::Mangadex => pool.mangadex.fetch_chapter_bookmarked(bookmark).await,
        MangaProviders::Inmanga => pool.inmanga.fetch_chapter_bookmarked(bookmark).await,
        MangaProviders::Leercapitulo => pool.leercapitulo.fetch_chapter_bookmarked(bookmark).await,
        MangaProviders::Mangaoni => pool.mangaoni.fetch_chapter_bookmarked(bookmark).await,
        MangaProviders::Weebcentral => pool.weebcentral.fetch_chapter_bookmarked(bookmark).await,
        MangaProviders::Mangapill => pool.mangapill.fetch_chapter_bookmarked(bookmark).await,
        MangaProviders::Comizy => pool.comizy.fetch_chapter_bookmarked(bookmark).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::manga_provider::ProviderIdentity;

    #[tokio::test]
    async fn test_pool_initializes_and_empty_query_returns_empty() {
        let results = search_all_providers("   ").await;
        assert!(results.is_empty());
    }

    #[test]
    fn test_pool_singleton() {
        assert_eq!(POOL.mangadex.name(), MangaProviders::Mangadex);
        assert_eq!(POOL.inmanga.name(), MangaProviders::Inmanga);
        assert_eq!(POOL.mangaoni.name(), MangaProviders::Mangaoni);
        assert_eq!(POOL.leercapitulo.name(), MangaProviders::Leercapitulo);
        assert_eq!(POOL.weebcentral.name(), MangaProviders::Weebcentral);
        assert_eq!(POOL.mangapill.name(), MangaProviders::Mangapill);
        assert_eq!(POOL.comizy.name(), MangaProviders::Comizy);
    }

    #[tokio::test]
    #[ignore]
    async fn test_search_all_providers_one_piece() {
        let results = search_all_providers("one piece").await;
        println!("Total results: {}", results.len());
        for r in &results {
            println!("Provider: {:?} | Title: {}", r.provider, r.title);
        }
        let mangadex_count = results.iter().filter(|r| r.provider == Some(MangaProviders::Mangadex)).count();
        println!("Mangadex results count: {}", mangadex_count);
        assert!(mangadex_count > 0, "No mangadex results returned in search_all_providers!");
    }

    #[tokio::test]
    #[ignore]
    async fn test_search_all_providers_made_in_abyss() {
        let results = search_all_providers("made in abyss").await;
        println!("Total results: {}", results.len());
        for r in &results {
            println!("Provider: {:?} | Title: {}", r.provider, r.title);
        }
        let mangaoni_count = results.iter().filter(|r| r.provider == Some(MangaProviders::Mangaoni)).count();
        println!("MangaOni results count: {}", mangaoni_count);
        assert!(mangaoni_count > 0, "No mangaoni results returned in search_all_providers for made in abyss!");
    }
}
