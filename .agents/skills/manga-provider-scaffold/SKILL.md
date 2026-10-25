---
name: manga-provider-scaffold
description: >-
  Use this skill when scaffolding, implementing, or registering a new manga provider
  in manga-tui. Guides through file structure, required traits, HTML/API scraping,
  multi-provider registration, and live test creation.
---

# Manga Provider Scaffold Skill

This skill guides you through implementing and registering a new manga provider in `manga-tui`.

## Workflow Overview

1. **Information Gathering & Target Analysis**
2. **File Structure Generation**
3. **HTTP Client & Scraper Implementation (`response.rs` & `<name>.rs`)**
4. **Trait Implementations**
5. **Codebase Registration (7 Touchpoints)**
6. **Live & Unit Testing**

---

## 1. Information Gathering & Target Analysis

Before writing code, analyze the target site:
- **Base URL & Language**: e.g., English, Spanish (`Languages::English`, `Languages::Spanish`).
- **Data Source**:
  - Direct JSON API (like MangaDex).
  - Server-rendered HTML (like WeebCentral, LeerCapitulo, MangaPill) using `scraper::Html` or custom parsers.
  - Form/POST searches or CSRF tokens (like MangaOni).
- **Anti-Bot / Cloudflare Protections**:
  - Requires `User-Agent` (desktop browser).
  - Requires specific `Referer` (e.g. for loading images/pages).
  - CookieJar or session headers.

---

## 2. Directory & File Structure

Create the following files under `src/backend/manga_provider/`:

```text
src/backend/manga_provider/
├── <name>.rs                  # Provider struct, client, trait impls, tests
└── <name>/
    ├── response.rs            # Scraper parsing / API response models
    ├── filter_state.rs        # Filter state & FiltersHandler impl
    └── filter_widget.rs       # Filter UI widget & FiltersWidget impl
```

---

## 3. Core Implementation Checklist

### A. Data Parsing (`<name>/response.rs`)
Implement the structs that convert raw HTML/JSON into `manga-tui` models:
- Search results $\rightarrow$ `SearchManga` / `PopularManga` / `RecentlyAddedManga`
- Manga details $\rightarrow$ `Manga` (title, authors, status, genres, cover URL)
- Chapter list $\rightarrow$ `ChapterReader` or `MangaChapter`
- Reader page links $\rightarrow$ `Vec<ChapterPageUrl>`

### B. Provider Struct (`<name>.rs`)
Define the provider struct and constructors:
```rust
#[derive(Clone, Debug)]
pub struct <Name>Provider {
    client: reqwest::Client,
    cache_provider: Arc<dyn Cacher>,
}

impl <Name>Provider {
    const CHAPTER_PAGE_CACHE_DURATION: CacheDuration = CacheDuration::Long;
    const HOME_PAGE_CACHE_DURATION: CacheDuration = CacheDuration::Short;
    const MANGA_PAGE_CACHE_DURATION: CacheDuration = CacheDuration::LongLong;
    const SEARCH_PAGE_CACHE_DURATION: CacheDuration = CacheDuration::VeryShort;

    pub fn new(cache_provider: Arc<dyn Cacher>) -> Self {
        // Build reqwest::Client with proper headers (User-Agent, Referer, Accept, Cookie store)
    }
}
```

### C. Required Traits
Implement all required traits for full integration:
- `ProviderIdentity`: Returns `MangaProviders::<Name>`
- `GetRawImage`: Fetches image bytes (often requires `Referer` header)
- `DecodeBytesToImage`: Marker trait
- `SearchMangaPanel`: Marker trait
- `HomePageMangaProvider`: Popular & recently added mangas for homepage
- `SearchMangaById`: Manga details page
- `GetChapterPages`: URLs for all images in a chapter
- `SearchChapterById`: Look up single chapter
- `FetchChapterBookmarked`: Sync bookmark status
- `GoToReadChapter`: Resolve chapter reader context
- `MangaPageProvider`: Chapter listings & pagination
- `SearchPageProvider`: Search query & filters execution
- `FeedPageProvider`: Provider feed / latest updates
- `ReaderPageProvider`: Marker trait
- `MangaProvider`: Super-trait unifying all above

See [references/trait_cheatsheet.md](./references/trait_cheatsheet.md) for boilerplate implementations.

---

## 4. Codebase Registration (Must Update All 7 Places)

See [references/registration_checklist.md](./references/registration_checklist.md) for the exact code snippets needed across:
1. `src/backend/manga_provider.rs`: `pub mod <name>;` and add to `pub enum MangaProviders`.
2. `src/backend/multi_provider.rs`: Add field to `MultiProvider`, initialize in `new`, and match in all dispatch methods (`search_all`, `get_manga_by_id`, `get_chapter_pages`, etc.).
3. `src/main.rs`: Match arm in startup router.
4. `src/view/app.rs`: Set pagination size in `search_page_size()`.
5. `src/view/widgets/search.rs`: Badge label & color in search results.
6. `src/view/pages/manga.rs`: Provider badge label & color in details view.
7. `Cargo.toml` / imports (if any new dependencies like specific regex or crypto are needed).

---

## 5. Live & Unit Testing

Add tests to the bottom of `src/backend/manga_provider/<name>.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::cache::mock::EmptyCache;

    #[tokio::test]
    #[ignore]
    async fn test_<name>_live_search() {
        let provider = <Name>Provider::new(EmptyCache::new_arc());
        let result = provider
            .search_mangas(SearchTerm::trimmed("one piece"), <Name>FilterState::default(), Pagination::from_first_page(20))
            .await;
        assert!(result.is_ok(), "search failed: {:?}", result.err());
        let res = result.unwrap();
        assert!(!res.mangas.is_empty(), "expected search results");
    }

    #[tokio::test]
    #[ignore]
    async fn test_<name>_live_chapters_and_pages() {
        let provider = <Name>Provider::new(EmptyCache::new_arc());
        // Fetch known manga chapters and first chapter pages
    }
}
```

Run test verification:
```powershell
cargo test backend::manga_provider::<name> -- --ignored --nocapture
```

---

## 6. Binary Installation (Mandatory)

Always install the updated binary into the user's local Cargo environment so the new provider is immediately usable with `manga-tui`:
```powershell
cargo install --path .
```

