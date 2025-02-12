use std::error::Error;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use filter_state::{ManganatoFilterState, ManganatoFiltersProvider};
use filter_widget::ManganatoFilterWidget;
use http::header::{ACCEPT, ACCEPT_ENCODING, REFERER};
use http::{HeaderMap, HeaderValue, StatusCode};
use manga_tui::SearchTerm;
use reqwest::cookie::Jar;
use reqwest::{Client, Url};
use response::{
    extract_id_from_url, ChapterPageResponse, ChapterUrls, GetPopularMangasResponse, MangaPageData, ManganatoChaptersResponse,
    NewAddedMangas, SearchMangaResponse, ToolTipItem,
};

use super::{
    Author, Chapter, ChapterOrderBy, ChapterPageUrl, DecodeBytesToImage, FeedPageProvider, FetchChapterBookmarked, Genres,
    GetChapterPages, GetMangasResponse, GetRawImage, GoToReadChapter, HomePageMangaProvider, Languages, ListOfChapters,
    MangaPageProvider, MangaProvider, PopularManga, ProviderIdentity, ReaderPageProvider, RecentlyAddedManga, SearchChapterById,
    SearchMangaById, SearchMangaPanel, SearchPageProvider,
};
use crate::backend::html_parser::{HtmlElement, ParseHtml};
use crate::backend::manga_provider::ChapterToRead;

pub static MANGANATO_BASE_URL: &str = "https://manganato.com";

pub mod filter_state;
pub mod filter_widget;
pub mod response;

#[derive(Clone, Debug)]
pub struct ManganatoProvider {
    client: reqwest::Client,
    base_url: Url,
    chapter_pages_header: HeaderMap,
}

impl ManganatoProvider {
    pub const MANGANATO_MANGA_LANGUAGE: &[Languages] = &[Languages::English];

    pub fn new(base_url: Url) -> Self {
        let mut default_headers = HeaderMap::new();

        default_headers.insert(REFERER, HeaderValue::from_static("https://google.com"));
        default_headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate"));
        default_headers.insert(
            ACCEPT,
            HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8,application/json"),
        );

        default_headers.insert("priority", HeaderValue::from_static("u=0, i"));
        default_headers.insert("sec-fetch-site", HeaderValue::from_static("cross-site"));
        default_headers.insert("sec-fetch-mode", HeaderValue::from_static("navigate"));
        default_headers.insert("sec-fetch-user", HeaderValue::from_static("?1"));
        default_headers.insert("sec-fetch-dest", HeaderValue::from_static("document"));

        let mut chapter_pages_header = HeaderMap::new();

        chapter_pages_header.insert(REFERER, HeaderValue::from_static("https://chapmanganato.to"));

        chapter_pages_header.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate"));
        chapter_pages_header
            .insert(ACCEPT, HeaderValue::from_static("image/avif,image/webp,image/png,image/svg+xml,image/*;q=0.8,*/*;q=0.5"));

        let client = Client::builder()
            .cookie_provider(Arc::new(Jar::default()))
            .default_headers(default_headers)
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:133.0) Gecko/20100101 Firefox/133.0")
            .build()
            .unwrap();

        Self {
            client,
            base_url,
            chapter_pages_header,
        }
    }

    fn format_search_term(search_term: SearchTerm) -> String {
        let mut search: String = search_term.get().split(" ").map(|word| format!("{word}_")).collect();

        search.pop();

        search
    }

    /// From one endpoint we can get both the chapter to read and the list of chapters so thats why
    /// this exist method exists
    /// `chapter_id` is expected to be a full url like: `https://chapmanganato.to/manga-bp1004524/chapter-20`
    /// this is because on manganato a chapter doesnt have a id per se
    /// and the `manga_id` is actually not required
    async fn get_chapter_page(&self, manga_id: &str, chapter_id: &str) -> Result<(ChapterToRead, ListOfChapters), Box<dyn Error>> {
        let response = self.client.get(chapter_id).send().await?;

        if response.status() != StatusCode::OK {
            return Err(format!(
                "Could not search chapter of manga on manganato with id : {manga_id}, status code : {}, {chapter_id}",
                response.status()
            )
            .into());
        }

        let doc = response.text().await?;

        let response = ChapterPageResponse::parse_html(HtmlElement::new(doc))?;

        let chapter_to_read: ChapterToRead = ChapterToRead {
            id: chapter_id.to_string(),
            title: response.title.unwrap_or("no title".to_string()),
            number: response.number.parse().unwrap(),
            volume_number: response.volume_number,
            num_page_bookmarked: None,
            language: Languages::English,
            pages_url: response.pages_url.urls.into_iter().map(|url| Url::parse(&url)).flatten().collect(),
        };

        let list_of_chapters = ListOfChapters::from(response.chapters_list);

        Ok((chapter_to_read, list_of_chapters))
    }
}

impl ProviderIdentity for ManganatoProvider {
    fn name(&self) -> super::MangaProviders {
        super::MangaProviders::Manganato
    }
}

impl GetRawImage for ManganatoProvider {
    async fn get_raw_image(&self, url: &str) -> Result<bytes::Bytes, Box<dyn Error>> {
        let response = self
            .client
            .get(url)
            .timeout(Duration::from_secs(3))
            .headers(self.chapter_pages_header.clone())
            .send()
            .await?;

        if response.status() != StatusCode::OK {
            return Err(format!("Could not get image on manganato with url : {url}").into());
        }

        Ok(response.bytes().await?)
    }
}

impl DecodeBytesToImage for ManganatoProvider {}

impl SearchMangaPanel for ManganatoProvider {}

impl SearchMangaById for ManganatoProvider {
    /// `manga_id` is expected to be the url which points to the manga page
    async fn get_manga_by_id(&self, manga_id: &str) -> Result<super::Manga, Box<dyn Error>> {
        let response = self.client.get(manga_id).send().await?;

        if response.status() != StatusCode::OK {
            return Err("manga page could not be found".into());
        }

        let doc = response.text().await?;

        let manga = MangaPageData::parse_html(HtmlElement::new(doc))?;

        let authors = if manga.authors.is_empty() {
            None
        } else {
            Some(Author {
                name: manga.authors,
                ..Default::default()
            })
        };

        Ok(super::Manga {
            id: manga_id.to_string(),
            id_safe_for_download: extract_id_from_url(manga_id),
            title: manga.title,
            genres: manga.genres.into_iter().map(Genres::from).collect(),
            description: manga.description,
            status: manga.status.into(),
            cover_img_url: manga.cover_url.clone(),
            languages: Self::MANGANATO_MANGA_LANGUAGE.into(),
            rating: manga.rating,
            // There is no way of knowing the artist/artists of the manga on manganato
            artist: None,
            author: authors,
        })
    }
}

impl SearchChapterById for ManganatoProvider {
    async fn search_chapter(&self, chapter_id: &str, manga_id: &str) -> Result<super::ChapterToRead, Box<dyn Error>> {
        let chapter = self.get_chapter_page(manga_id, chapter_id).await?;
        Ok(chapter.0)
    }
}

impl HomePageMangaProvider for ManganatoProvider {
    async fn get_popular_mangas(&self) -> Result<Vec<super::PopularManga>, Box<dyn Error>> {
        let response = self.client.get(self.base_url.clone()).send().await?;

        if response.status() != StatusCode::OK {
            return Err("could not".into());
        }

        let doc = response.text().await?;

        let response = GetPopularMangasResponse::parse_html(HtmlElement::new(doc))?;

        Ok(response.mangas.into_iter().map(PopularManga::from).collect())
    }

    async fn get_recently_added_mangas(&self) -> Result<Vec<super::RecentlyAddedManga>, Box<dyn Error>> {
        let response = self.client.get(self.base_url.clone()).send().await?;

        if response.status() != StatusCode::OK {
            return Err(format!("could not find recently added mangas on manganato, status code : {}", response.status()).into());
        }

        let doc = response.text().await?;

        let new_mangas = NewAddedMangas::parse_html(HtmlElement::new(doc))?;

        let tool_tip_response = self.client.get(format!("{}/homepage_tooltips_json", self.base_url)).send().await?;

        if tool_tip_response.status() != StatusCode::OK {
            return Err(
                format!("could not find recently added mangas on manganato, status code : {}", tool_tip_response.status()).into()
            );
        }

        let tool_tip_response: Vec<ToolTipItem> = tool_tip_response.json().await?;
        let mut response: Vec<RecentlyAddedManga> = vec![];

        for new_manga in new_mangas.mangas.into_iter() {
            let from_tool_tip = tool_tip_response
                .iter()
                .find(|data| data.id == new_manga.id_tooltip)
                .cloned()
                .unwrap_or_default();

            let manga = RecentlyAddedManga {
                id: new_manga.id,
                title: from_tool_tip.name,
                description: from_tool_tip.description,
                cover_img_url: Some(from_tool_tip.image),
            };

            response.push(manga);
        }

        Ok(response)
    }
}

impl SearchPageProvider for ManganatoProvider {
    type FiltersHandler = ManganatoFiltersProvider;
    type InnerState = ManganatoFilterState;
    type Widget = ManganatoFilterWidget;

    async fn search_mangas(
        &self,
        search_term: Option<SearchTerm>,
        _filters: Self::InnerState,
        pagination: super::Pagination,
    ) -> Result<super::GetMangasResponse, Box<dyn Error>> {
        let search = match search_term {
            Some(search) => ("keyw", Self::format_search_term(search)),
            None => ("", "".to_string()),
        };

        let endpoint = format!("{}/advanced_search", self.base_url);

        let response = self
            .client
            .get(endpoint)
            .query(&[("page", pagination.current_page.to_string()), ("s", "all".to_string()), search])
            .send()
            .await?;

        if response.status() != StatusCode::OK {
            return Err("could not search mangas on manganato".into());
        }

        let doc = response.text().await?;

        let result = SearchMangaResponse::parse_html(HtmlElement::new(doc))?;

        Ok(GetMangasResponse::from(result))
    }
}

impl GoToReadChapter for ManganatoProvider {
    async fn read_chapter(
        &self,
        chapter_id: &str,
        manga_id: &str,
    ) -> Result<(super::ChapterToRead, super::ListOfChapters), Box<dyn Error>> {
        Ok(self.get_chapter_page(manga_id, chapter_id).await?)
    }
}

impl GetChapterPages for ManganatoProvider {
    async fn get_chapter_pages_url_with_extension(
        &self,
        chapter_id: &str,
        _manga_id: &str,
        _image_quality: crate::config::ImageQuality,
    ) -> Result<Vec<super::ChapterPageUrl>, Box<dyn Error>> {
        let response = self.client.get(chapter_id).send().await?;

        if response.status() != StatusCode::OK {
            return Err("could not".into());
        }

        let doc = response.text().await?;

        let pages = ChapterUrls::parse_html(HtmlElement::new(doc))?;

        let mut pages_url: Vec<ChapterPageUrl> = vec![];

        for page in pages.urls {
            let url = Url::parse(&page).unwrap_or("https://localhost".parse().unwrap());
            let extension = Path::new(&page).extension().unwrap().to_str().unwrap().to_string();

            pages_url.push(ChapterPageUrl { url, extension });
        }
        Ok(pages_url)
    }
}

impl FetchChapterBookmarked for ManganatoProvider {
    async fn fetch_chapter_bookmarked(
        &self,
        chapter: crate::backend::database::ChapterBookmarked,
    ) -> Result<(super::ChapterToRead, super::ListOfChapters), Box<dyn Error>> {
        Ok(self.get_chapter_page("", &chapter.id).await?)
    }
}

impl ReaderPageProvider for ManganatoProvider {}

impl MangaPageProvider for ManganatoProvider {
    async fn get_chapters(
        &self,
        manga_id: &str,
        filters: super::ChapterFilters,
        pagination: super::Pagination,
    ) -> Result<super::GetChaptersResponse, Box<dyn Error>> {
        let response = self.client.get(manga_id).send().await?;

        if response.status() != StatusCode::OK {
            return Err(format!("could not find manga page for : {manga_id}").into());
        }

        let doc = response.text().await?;

        let response = ManganatoChaptersResponse::parse_html(HtmlElement::new(doc))?;

        let total_chapters = response.total_chapters;

        let mut chapters: Vec<Chapter> = response
            .chapters
            .into_iter()
            .map(|chap| Chapter {
                id: chap.page_url.clone(),
                id_safe_for_download: format!("{}-{}", extract_id_from_url(manga_id), extract_id_from_url(chap.page_url)),
                title: chap.title.unwrap_or("no title".to_string()),
                volume_number: chap.volume,
                scanlator: Some("Manganato".to_string()),
                language: Languages::English,
                chapter_number: chap.number,
                manga_id: manga_id.to_string(),
                publication_date: chap.uploaded_at,
            })
            .collect();

        if filters.order == ChapterOrderBy::Ascending {
            chapters.reverse();
        }

        let from = pagination.from_index();
        let to = pagination.to_index(total_chapters as usize);

        let chapters = chapters.as_slice().get(from..to).unwrap_or(&[]);

        Ok(super::GetChaptersResponse {
            chapters: chapters.to_vec(),
            total_chapters,
        })
    }

    async fn get_all_chapters(&self, manga_id: &str, _language: Languages) -> Result<Vec<super::Chapter>, Box<dyn Error>> {
        let response = self.client.get(manga_id).send().await?;

        if response.status() != StatusCode::OK {
            return Err(format!("could not find manga page for : {manga_id}").into());
        }

        let doc = response.text().await?;

        let response = ManganatoChaptersResponse::parse_html(HtmlElement::new(doc))?;

        let chapters: Vec<Chapter> = response
            .chapters
            .into_iter()
            .map(|chap| Chapter {
                id: chap.page_url.clone(),
                id_safe_for_download: format!("{}-{}", extract_id_from_url(manga_id), extract_id_from_url(chap.page_url)),
                title: chap.title.unwrap_or("no title".to_string()),
                volume_number: chap.volume,
                scanlator: Some("Manganato".to_string()),
                language: Languages::English,
                chapter_number: chap.number,
                manga_id: manga_id.to_string(),
                publication_date: chap.uploaded_at,
            })
            .collect();

        Ok(chapters)
    }
}

impl FeedPageProvider for ManganatoProvider {
    async fn get_latest_chapters(&self, manga_id: &str) -> Result<Vec<super::LatestChapter>, Box<dyn Error>> {
        todo!()
    }
}

impl MangaProvider for ManganatoProvider {}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn parses_search_term_correctly() {
        let searchterm = SearchTerm::trimmed_lowercased("death note").unwrap();

        assert_eq!("death_note", ManganatoProvider::format_search_term(searchterm));

        let searchterm = SearchTerm::trimmed_lowercased("oshi no ko").unwrap();

        assert_eq!("oshi_no_ko", ManganatoProvider::format_search_term(searchterm));
    }
}
