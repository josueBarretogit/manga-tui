# Manga Provider Trait Implementation Cheatsheet

Below is the standard boilerplate to implement all required traits for a provider `<Name>Provider`.

```rust
use std::error::Error;
use std::future::Future;
use std::sync::Arc;
use bytes::Bytes;
use manga_tui::SearchTerm;
use super::*;

// 1. Identity
impl ProviderIdentity for <Name>Provider {
    fn provider(&self) -> MangaProviders {
        MangaProviders::<Name>
    }
}

// 2. Raw Image Fetching (Crucial: referer header often required)
impl GetRawImage for <Name>Provider {
    fn get_raw_image(&self, url: &str) -> impl Future<Output = Result<Bytes, Box<dyn Error>>> + Send {
        async move {
            let res = self.client.get(url)
                .header(http::header::REFERER, "<BASE_URL>")
                .send()
                .await?
                .error_for_status()?
                .bytes()
                .await?;
            Ok(res)
        }
    }
}

// 3. Markers
impl DecodeBytesToImage for <Name>Provider {}
impl SearchMangaPanel for <Name>Provider {}
impl ReaderPageProvider for <Name>Provider {}
impl MangaProvider for <Name>Provider {}

// 4. Home Page
impl HomePageMangaProvider for <Name>Provider {
    fn get_popular_mangas(&self) -> impl Future<Output = Result<Vec<PopularManga>, Box<dyn Error>>> + Send {
        async move {
            // Fetch and parse popular mangas
            Ok(vec![])
        }
    }

    fn get_latest_mangas(&self) -> impl Future<Output = Result<Vec<RecentlyAddedManga>, Box<dyn Error>>> + Send {
        async move {
            // Fetch and parse latest mangas
            Ok(vec![])
        }
    }
}

// 5. Manga Details
impl SearchMangaById for <Name>Provider {
    fn get_manga_by_id(&self, id: &str) -> impl Future<Output = Result<Manga, Box<dyn Error>>> + Send {
        async move {
            // Fetch manga information by id/slug
            Ok(Manga::default())
        }
    }
}

// 6. Chapter Pages (Reader)
impl GetChapterPages for <Name>Provider {
    fn get_chapter_pages_url_with_extension(
        &self,
        chapter_id: &str,
        manga_id: &str,
        _quality: ImageQuality,
    ) -> impl Future<Output = Result<Vec<ChapterPageUrl>, Box<dyn Error>>> + Send {
        async move {
            // Fetch image URLs for chapter
            Ok(vec![])
        }
    }
}

// 7. Search Chapter By ID
impl SearchChapterById for <Name>Provider {
    fn get_chapter_by_id(&self, chapter_id: &str) -> impl Future<Output = Result<Chapter, Box<dyn Error>>> + Send {
        async move {
            Ok(Chapter::default())
        }
    }
}

// 8. Bookmarks & Reader Navigation
impl FetchChapterBookmarked for <Name>Provider {
    fn fetch_chapter_bookmarked(&self, manga_id: &str, chapter_id: &str) -> impl Future<Output = Result<Option<ChapterBookmarked>, Box<dyn Error>>> + Send {
        async move { Ok(None) }
    }
}

impl GoToReadChapter for <Name>Provider {
    fn go_to_read_chapter(&self, chapter_id: &str, manga_id: &str) -> impl Future<Output = Result<ChapterToRead, Box<dyn Error>>> + Send {
        async move {
            // Return ChapterToRead context
            Ok(ChapterToRead::default())
        }
    }
}

// 9. Manga Details & Chapter List
impl MangaPageProvider for <Name>Provider {
    type FilterState = <Name>FilterState;
    type FiltersProvider = <Name>FiltersProvider;

    fn get_all_chapters(&self, manga_id: &str, _lang: Languages) -> impl Future<Output = Result<Vec<ChapterReader>, Box<dyn Error>>> + Send {
        async move {
            // Return all chapters for reader navigation
            Ok(vec![])
        }
    }

    fn get_chapters(&self, manga_id: &str, page: usize) -> impl Future<Output = Result<GetChaptersResponse, Box<dyn Error>>> + Send {
        async move {
            // Paginated chapter list for manga page
            Ok(GetChaptersResponse::default())
        }
    }
}

// 10. Search Page
impl SearchPageProvider for <Name>Provider {
    type FilterState = <Name>FilterState;
    type FiltersProvider = <Name>FiltersProvider;

    fn search_mangas(
        &self,
        search_term: SearchTerm,
        filter_state: Self::FilterState,
        pagination: Pagination,
    ) -> impl Future<Output = Result<GetMangasResponse, Box<dyn Error>>> + Send {
        async move {
            // Execute search with pagination
            Ok(GetMangasResponse::default())
        }
    }
}

// 11. Feed Updates
impl FeedPageProvider for <Name>Provider {
    fn get_feed(&self, page: usize) -> impl Future<Output = Result<GetMangasResponse, Box<dyn Error>>> + Send {
        async move {
            Ok(GetMangasResponse::default())
        }
    }
}
```
