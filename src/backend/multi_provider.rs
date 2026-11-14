use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

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
    MangaPageProvider, MangaPanel, MangaProviders, Pagination, ProviderError, SearchChapterById, SearchManga, SearchMangaById,
    SearchMangaPanel, SearchPageProvider,
};
use crate::config::{ImageQuality, MangaTuiConfig};

const CIRCUIT_BREAKER_THRESHOLD: u32 = 3;
const CIRCUIT_BREAKER_COOLDOWN: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProviderHealthStatus {
    #[default]
    Healthy,
    Degraded,
    CircuitOpen,
}

impl ProviderHealthStatus {
    pub fn indicator(&self) -> &'static str {
        match self {
            Self::Healthy => "🟢",
            Self::Degraded => "🟡",
            Self::CircuitOpen => "🔴",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProviderHealthRecord {
    pub consecutive_failures: u32,
    pub total_successes: u64,
    pub total_failures: u64,
    pub circuit_opened_at: Option<Instant>,
    pub last_error: Option<ProviderError>,
}

#[derive(Debug, Default)]
pub struct ProviderHealthTracker {
    records: Mutex<HashMap<MangaProviders, ProviderHealthRecord>>,
    cooldown: Duration,
}

impl ProviderHealthTracker {
    pub fn new() -> Self {
        Self::with_cooldown(CIRCUIT_BREAKER_COOLDOWN)
    }

    pub fn with_cooldown(cooldown: Duration) -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
            cooldown,
        }
    }

    pub fn record_success(&self, provider: MangaProviders) {
        if let Ok(mut map) = self.records.lock() {
            let rec = map.entry(provider).or_default();
            rec.consecutive_failures = 0;
            rec.total_successes += 1;
            rec.circuit_opened_at = None;
            rec.last_error = None;
        }
    }

    pub fn record_failure(&self, provider: MangaProviders, error: ProviderError) {
        if !error.should_trip_circuit() {
            return;
        }
        if let Ok(mut map) = self.records.lock() {
            let rec = map.entry(provider).or_default();
            rec.total_failures += 1;
            rec.consecutive_failures += 1;
            // Immediate circuit trip on Cloudflare block or rate-limit
            if matches!(error, ProviderError::CloudflareBlocked { .. } | ProviderError::RateLimited { .. }) {
                rec.consecutive_failures = rec.consecutive_failures.max(CIRCUIT_BREAKER_THRESHOLD);
            }
            if rec.consecutive_failures >= CIRCUIT_BREAKER_THRESHOLD {
                rec.circuit_opened_at = Some(Instant::now());
            }
            rec.last_error = Some(error);
        }
    }

    pub fn status(&self, provider: MangaProviders) -> ProviderHealthStatus {
        let Ok(map) = self.records.lock() else {
            return ProviderHealthStatus::Healthy;
        };
        let Some(rec) = map.get(&provider) else {
            return ProviderHealthStatus::Healthy;
        };
        if rec.consecutive_failures == 0 {
            ProviderHealthStatus::Healthy
        } else if rec.consecutive_failures >= CIRCUIT_BREAKER_THRESHOLD {
            if let Some(opened_at) = rec.circuit_opened_at
                && opened_at.elapsed() < self.cooldown
            {
                return ProviderHealthStatus::CircuitOpen;
            }
            // Cooldown elapsed -> half-open (degraded until next probe succeeds)
            ProviderHealthStatus::Degraded
        } else {
            ProviderHealthStatus::Degraded
        }
    }

    pub fn is_available(&self, provider: MangaProviders) -> bool {
        self.status(provider) != ProviderHealthStatus::CircuitOpen
    }

    pub fn last_error(&self, provider: MangaProviders) -> Option<ProviderError> {
        self.records.lock().ok()?.get(&provider).and_then(|r| r.last_error.clone())
    }

    pub fn reset(&self, provider: MangaProviders) {
        if let Ok(mut map) = self.records.lock() {
            map.remove(&provider);
        }
    }
}

pub struct MultiProviderPool {
    pub cache: Arc<dyn Cacher>,
    pub health: ProviderHealthTracker,
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
            health: ProviderHealthTracker::new(),
            cache,
        }
    }
}

pub static POOL: LazyLock<MultiProviderPool> = LazyLock::new(MultiProviderPool::new);

pub fn get_provider_health_status(provider: MangaProviders) -> ProviderHealthStatus {
    POOL.health.status(provider)
}

pub fn is_provider_available(provider: MangaProviders) -> bool {
    POOL.health.is_available(provider)
}

pub fn get_provider_last_error(provider: MangaProviders) -> Option<ProviderError> {
    POOL.health.last_error(provider)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MultiSearchLanguage {
    #[default]
    All,
    Spanish,
    English,
}

pub fn calculate_relevance_score(title: &str, query_lower: &str) -> u8 {
    let t = title.to_lowercase();
    if t == query_lower {
        0
    } else if t.starts_with(query_lower) {
        1
    } else if t.split_whitespace().any(|word| word.starts_with(query_lower)) {
        2
    } else if t.contains(query_lower) {
        3
    } else {
        4
    }
}

fn handle_search_outcome<T>(
    pool: &MultiProviderPool,
    provider: MangaProviders,
    res: Result<Result<T, Box<dyn Error>>, tokio::time::error::Elapsed>,
) -> Option<T> {
    match res {
        Ok(Ok(val)) => {
            pool.health.record_success(provider);
            Some(val)
        },
        Ok(Err(e)) => {
            pool.health
                .record_failure(provider, ProviderError::from_error(Some(provider), e.as_ref()));
            None
        },
        Err(_) => {
            pool.health.record_failure(provider, ProviderError::NetworkTimeout {
                provider: Some(provider),
            });
            None
        },
    }
}

/// Search concurrently across manga providers matching the specified language filter.
/// Providers whose circuit breaker is open are automatically skipped during their cooldown window.
pub async fn search_all_providers_with_lang(query: &str, lang: MultiSearchLanguage) -> Vec<SearchManga> {
    let query_trimmed = query.trim();
    if query_trimmed.is_empty() {
        return Vec::new();
    }
    let pool = &*POOL;
    let timeout_duration = Duration::from_secs(8);

    let query_spanish = lang == MultiSearchLanguage::All || lang == MultiSearchLanguage::Spanish;
    let query_english = lang == MultiSearchLanguage::All || lang == MultiSearchLanguage::English;

    let (inmanga_res, mangaoni_res, leercapitulo_res, mangadex_res, weebcentral_res, mangapill_res, comizy_res) = tokio::join!(
        // InManga
        async {
            if !query_spanish || !pool.health.is_available(MangaProviders::Inmanga) {
                return None;
            }
            let term = SearchTerm::trimmed_lowercased(query);
            let res = tokio::time::timeout(
                timeout_duration,
                pool.inmanga.search_mangas(term, InmangaFilterState::default(), Pagination::default()),
            )
            .await;
            handle_search_outcome(pool, MangaProviders::Inmanga, res)
        },
        // MangaOni
        async {
            if !query_spanish || !pool.health.is_available(MangaProviders::Mangaoni) {
                return None;
            }
            let term = SearchTerm::trimmed_lowercased(query);
            let res = tokio::time::timeout(
                timeout_duration,
                pool.mangaoni.search_mangas(term, MangaoniFilterState::default(), Pagination::default()),
            )
            .await;
            handle_search_outcome(pool, MangaProviders::Mangaoni, res)
        },
        // LeerCapítulo
        async {
            if !query_spanish || !pool.health.is_available(MangaProviders::Leercapitulo) {
                return None;
            }
            let term = SearchTerm::trimmed_lowercased(query);
            let res = tokio::time::timeout(
                timeout_duration,
                pool.leercapitulo
                    .search_mangas(term, LeercapituloFilterState::default(), Pagination::default()),
            )
            .await;
            handle_search_outcome(pool, MangaProviders::Leercapitulo, res)
        },
        // MangaDex (included in All, Spanish, and English)
        async {
            if !pool.health.is_available(MangaProviders::Mangadex) {
                return None;
            }
            let term = SearchTerm::trimmed_lowercased(query);
            let mut filters = Filters::default();
            filters.set_sort_by(SortBy::BestMatch);
            filters.set_content_rating(vec![ContentRating::Safe, ContentRating::Suggestive]);
            let res =
                tokio::time::timeout(timeout_duration, pool.mangadex.search_mangas(term, filters, Pagination::default())).await;
            handle_search_outcome(pool, MangaProviders::Mangadex, res)
        },
        // WeebCentral
        async {
            if !query_english || !pool.health.is_available(MangaProviders::Weebcentral) {
                return None;
            }
            let term = SearchTerm::trimmed_lowercased(query);
            let res = tokio::time::timeout(
                timeout_duration,
                pool.weebcentral
                    .search_mangas(term, WeebcentralFilterState::default(), Pagination::default()),
            )
            .await;
            handle_search_outcome(pool, MangaProviders::Weebcentral, res)
        },
        // MangaPill
        async {
            if !query_english || !pool.health.is_available(MangaProviders::Mangapill) {
                return None;
            }
            let term = SearchTerm::trimmed_lowercased(query);
            let res = tokio::time::timeout(
                timeout_duration,
                pool.mangapill.search_mangas(term, MangaPillFilterState::default(), Pagination::default()),
            )
            .await;
            handle_search_outcome(pool, MangaProviders::Mangapill, res)
        },
        // Comizy
        async {
            if !query_english || !pool.health.is_available(MangaProviders::Comizy) {
                return None;
            }
            let term = SearchTerm::trimmed_lowercased(query);
            let res = tokio::time::timeout(
                timeout_duration,
                pool.comizy.search_mangas(term, ComizyFilterState::default(), Pagination::default()),
            )
            .await;
            handle_search_outcome(pool, MangaProviders::Comizy, res)
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

    // Sort by query relevance (exact match first -> prefix -> word match -> substring)
    let query_lower = query_trimmed.to_lowercase();
    combined.sort_by(|a, b| {
        let score_a = calculate_relevance_score(&a.title, &query_lower);
        let score_b = calculate_relevance_score(&b.title, &query_lower);
        score_a.cmp(&score_b)
    });

    combined
}

/// Search concurrently across all available manga providers
pub async fn search_all_providers(query: &str) -> Vec<SearchManga> {
    search_all_providers_with_lang(query, MultiSearchLanguage::All).await
}

/// Find alternative providers that carry a manga matching `manga_title`, excluding `exclude_provider`.
/// Prioritizes the best match per provider first so the user can quickly migrate between sources.
pub async fn find_alternative_providers(manga_title: &str, exclude_provider: Option<MangaProviders>) -> Vec<SearchManga> {
    let query_trimmed = manga_title.trim();
    if query_trimmed.is_empty() {
        return Vec::new();
    }
    let query_lower = query_trimmed.to_lowercase();
    let all_results = search_all_providers(query_trimmed).await;

    let mut primary_per_provider = Vec::new();
    let mut secondary_matches = Vec::new();
    let mut seen_providers = HashSet::new();

    for manga in all_results {
        if manga.provider == exclude_provider {
            continue;
        }
        let score = calculate_relevance_score(&manga.title, &query_lower);
        if score <= 3 {
            if let Some(prov) = manga.provider
                && seen_providers.insert(prov)
            {
                primary_per_provider.push(manga);
            } else if score <= 1 {
                secondary_matches.push(manga);
            }
        }
    }

    primary_per_provider.extend(secondary_matches);
    primary_per_provider
}

fn record_result<T>(provider: MangaProviders, res: Result<T, Box<dyn Error>>) -> Result<T, Box<dyn Error>> {
    let pool = &*POOL;
    match res {
        Ok(val) => {
            pool.health.record_success(provider);
            Ok(val)
        },
        Err(e) => {
            let typed_err = ProviderError::from_error(Some(provider), e.as_ref());
            pool.health.record_failure(provider, typed_err.clone());
            Err(Box::new(typed_err))
        },
    }
}

pub async fn get_manga_by_id_for_provider(provider: MangaProviders, manga_id: &str) -> Result<Manga, Box<dyn Error + Send + Sync>> {
    let pool = &*POOL;
    let res = match provider {
        MangaProviders::Mangadex => pool.mangadex.get_manga_by_id(manga_id).await,
        MangaProviders::Inmanga => pool.inmanga.get_manga_by_id(manga_id).await,
        MangaProviders::Leercapitulo => pool.leercapitulo.get_manga_by_id(manga_id).await,
        MangaProviders::Mangaoni => pool.mangaoni.get_manga_by_id(manga_id).await,
        MangaProviders::Weebcentral => pool.weebcentral.get_manga_by_id(manga_id).await,
        MangaProviders::Mangapill => pool.mangapill.get_manga_by_id(manga_id).await,
        MangaProviders::Comizy => pool.comizy.get_manga_by_id(manga_id).await,
    };
    match res {
        Ok(mut manga) => {
            pool.health.record_success(provider);
            manga.provider = Some(provider);
            Ok(manga)
        },
        Err(e) => {
            let typed_err = ProviderError::from_error(Some(provider), e.as_ref());
            pool.health.record_failure(provider, typed_err.clone());
            Err(Box::new(typed_err))
        },
    }
}

pub async fn get_chapters_for_provider(
    provider: MangaProviders,
    manga_id: &str,
    chapter_filters: ChapterFilters,
    pagination: Pagination,
) -> Result<GetChaptersResponse, Box<dyn Error>> {
    let pool = &*POOL;
    let res = match provider {
        MangaProviders::Mangadex => pool.mangadex.get_chapters(manga_id, chapter_filters, pagination).await,
        MangaProviders::Inmanga => pool.inmanga.get_chapters(manga_id, chapter_filters, pagination).await,
        MangaProviders::Leercapitulo => pool.leercapitulo.get_chapters(manga_id, chapter_filters, pagination).await,
        MangaProviders::Mangaoni => pool.mangaoni.get_chapters(manga_id, chapter_filters, pagination).await,
        MangaProviders::Weebcentral => pool.weebcentral.get_chapters(manga_id, chapter_filters, pagination).await,
        MangaProviders::Mangapill => pool.mangapill.get_chapters(manga_id, chapter_filters, pagination).await,
        MangaProviders::Comizy => pool.comizy.get_chapters(manga_id, chapter_filters, pagination).await,
    };
    record_result(provider, res)
}

pub async fn get_all_chapters_for_provider(
    provider: MangaProviders,
    manga_id: &str,
    lang: Languages,
) -> Result<Vec<Chapter>, Box<dyn Error>> {
    let pool = &*POOL;
    let res = match provider {
        MangaProviders::Mangadex => pool.mangadex.get_all_chapters(manga_id, lang).await,
        MangaProviders::Inmanga => pool.inmanga.get_all_chapters(manga_id, lang).await,
        MangaProviders::Leercapitulo => pool.leercapitulo.get_all_chapters(manga_id, lang).await,
        MangaProviders::Mangaoni => pool.mangaoni.get_all_chapters(manga_id, lang).await,
        MangaProviders::Weebcentral => pool.weebcentral.get_all_chapters(manga_id, lang).await,
        MangaProviders::Mangapill => pool.mangapill.get_all_chapters(manga_id, lang).await,
        MangaProviders::Comizy => pool.comizy.get_all_chapters(manga_id, lang).await,
    };
    record_result(provider, res)
}

pub async fn get_latest_chapters_for_provider(
    provider: MangaProviders,
    manga_id: &str,
) -> Result<Vec<LatestChapter>, Box<dyn Error>> {
    let pool = &*POOL;
    let res = match provider {
        MangaProviders::Mangadex => pool.mangadex.get_latest_chapters(manga_id).await,
        MangaProviders::Inmanga => pool.inmanga.get_latest_chapters(manga_id).await,
        MangaProviders::Leercapitulo => pool.leercapitulo.get_latest_chapters(manga_id).await,
        MangaProviders::Mangaoni => pool.mangaoni.get_latest_chapters(manga_id).await,
        MangaProviders::Weebcentral => pool.weebcentral.get_latest_chapters(manga_id).await,
        MangaProviders::Mangapill => pool.mangapill.get_latest_chapters(manga_id).await,
        MangaProviders::Comizy => pool.comizy.get_latest_chapters(manga_id).await,
    };
    record_result(provider, res)
}

pub async fn read_chapter_for_provider(
    provider: MangaProviders,
    chapter_id: &str,
    manga_id: &str,
) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
    let pool = &*POOL;
    let res = match provider {
        MangaProviders::Mangadex => pool.mangadex.read_chapter(chapter_id, manga_id).await,
        MangaProviders::Inmanga => pool.inmanga.read_chapter(chapter_id, manga_id).await,
        MangaProviders::Leercapitulo => pool.leercapitulo.read_chapter(chapter_id, manga_id).await,
        MangaProviders::Mangaoni => pool.mangaoni.read_chapter(chapter_id, manga_id).await,
        MangaProviders::Weebcentral => pool.weebcentral.read_chapter(chapter_id, manga_id).await,
        MangaProviders::Mangapill => pool.mangapill.read_chapter(chapter_id, manga_id).await,
        MangaProviders::Comizy => pool.comizy.read_chapter(chapter_id, manga_id).await,
    };
    record_result(provider, res)
}

pub async fn search_chapter_for_provider(
    provider: MangaProviders,
    chapter_id: &str,
    manga_id: &str,
) -> Result<ChapterToRead, Box<dyn Error>> {
    let pool = &*POOL;
    let res = match provider {
        MangaProviders::Mangadex => pool.mangadex.search_chapter(chapter_id, manga_id).await,
        MangaProviders::Inmanga => pool.inmanga.search_chapter(chapter_id, manga_id).await,
        MangaProviders::Leercapitulo => pool.leercapitulo.search_chapter(chapter_id, manga_id).await,
        MangaProviders::Mangaoni => pool.mangaoni.search_chapter(chapter_id, manga_id).await,
        MangaProviders::Weebcentral => pool.weebcentral.search_chapter(chapter_id, manga_id).await,
        MangaProviders::Mangapill => pool.mangapill.search_chapter(chapter_id, manga_id).await,
        MangaProviders::Comizy => pool.comizy.search_chapter(chapter_id, manga_id).await,
    };
    record_result(provider, res)
}

pub async fn get_chapter_pages_for_provider<F: Fn(f64, &str) + 'static + Send>(
    provider: MangaProviders,
    chapter_id: &str,
    manga_id: &str,
    quality: ImageQuality,
    progress: F,
) -> Result<Vec<ChapterPage>, Box<dyn Error>> {
    let pool = &*POOL;
    let res = match provider {
        MangaProviders::Mangadex => pool.mangadex.get_chapter_pages(chapter_id, manga_id, quality, progress).await,
        MangaProviders::Inmanga => pool.inmanga.get_chapter_pages(chapter_id, manga_id, quality, progress).await,
        MangaProviders::Leercapitulo => pool.leercapitulo.get_chapter_pages(chapter_id, manga_id, quality, progress).await,
        MangaProviders::Mangaoni => pool.mangaoni.get_chapter_pages(chapter_id, manga_id, quality, progress).await,
        MangaProviders::Weebcentral => pool.weebcentral.get_chapter_pages(chapter_id, manga_id, quality, progress).await,
        MangaProviders::Mangapill => pool.mangapill.get_chapter_pages(chapter_id, manga_id, quality, progress).await,
        MangaProviders::Comizy => pool.comizy.get_chapter_pages(chapter_id, manga_id, quality, progress).await,
    };
    record_result(provider, res)
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

    #[test]
    fn test_calculate_relevance_score() {
        assert_eq!(calculate_relevance_score("One Piece", "one piece"), 0);
        assert_eq!(calculate_relevance_score("One Piece: Ace's Story", "one piece"), 1);
        assert_eq!(calculate_relevance_score("That One Piece Fan", "piece"), 2);
        assert_eq!(calculate_relevance_score("Clone Piece", "one"), 3);
        assert_eq!(calculate_relevance_score("Naruto", "one piece"), 4);
    }

    #[test]
    fn test_provider_error_classification() {
        let rate_err = ProviderError::from_error_str(Some(MangaProviders::Mangadex), "HTTP 429 Too Many Requests");
        assert!(rate_err.is_rate_limited());
        assert!(rate_err.is_transient());
        assert!(rate_err.should_trip_circuit());

        let cf_err = ProviderError::from_error_str(Some(MangaProviders::Inmanga), "403 Forbidden: Cloudflare challenge");
        assert_eq!(cf_err, ProviderError::CloudflareBlocked {
            provider: Some(MangaProviders::Inmanga),
        });
        assert!(cf_err.should_trip_circuit());

        let not_found = ProviderError::from_http_status(Some(MangaProviders::Mangaoni), reqwest::StatusCode::NOT_FOUND);
        assert!(!not_found.should_trip_circuit());

        let timeout_err = ProviderError::from_error_str(Some(MangaProviders::Comizy), "operation timed out");
        assert_eq!(timeout_err, ProviderError::NetworkTimeout {
            provider: Some(MangaProviders::Comizy),
        });
    }

    #[test]
    fn test_circuit_breaker_health_tracker() {
        let tracker = ProviderHealthTracker::with_cooldown(Duration::from_millis(50));
        let p = MangaProviders::Inmanga;

        assert_eq!(tracker.status(p), ProviderHealthStatus::Healthy);
        assert!(tracker.is_available(p));

        // 1st transient failure -> Degraded
        tracker.record_failure(p, ProviderError::NetworkTimeout { provider: Some(p) });
        assert_eq!(tracker.status(p), ProviderHealthStatus::Degraded);
        assert!(tracker.is_available(p));

        // NotFound should not trip circuit
        tracker.record_failure(p, ProviderError::NotFound {
            resource: "ch1".to_string(),
        });
        assert_eq!(tracker.status(p), ProviderHealthStatus::Degraded);

        // 2nd & 3rd failures -> CircuitOpen
        tracker.record_failure(p, ProviderError::NetworkTimeout { provider: Some(p) });
        tracker.record_failure(p, ProviderError::NetworkTimeout { provider: Some(p) });
        assert_eq!(tracker.status(p), ProviderHealthStatus::CircuitOpen);
        assert!(!tracker.is_available(p));

        // Wait for cooldown -> transitions to half-open (Degraded) and available again
        std::thread::sleep(Duration::from_millis(60));
        assert_eq!(tracker.status(p), ProviderHealthStatus::Degraded);
        assert!(tracker.is_available(p));

        // Success resets to Healthy
        tracker.record_success(p);
        assert_eq!(tracker.status(p), ProviderHealthStatus::Healthy);
        assert!(tracker.last_error(p).is_none());

        // Immediate circuit trip on Cloudflare block
        tracker.record_failure(p, ProviderError::CloudflareBlocked { provider: Some(p) });
        assert_eq!(tracker.status(p), ProviderHealthStatus::CircuitOpen);
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
