use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};

use filter_state::{ManganatoFilterState, ManganatoFiltersProvider};
use filter_widget::ManganatoFilterWidget;
use http::header::{ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, REFERER};
use http::{HeaderMap, HeaderValue, StatusCode};
use manga_tui::SearchTerm;
use reqwest::{Client, Url};
use response::{GetPopularMangasResponse, MangaPageData, NewAddedMangas, SearchMangaResponse, ToolTipItem};
use tokio::sync::Mutex;

use super::{
    DecodeBytesToImage, FetchChapterBookmarked, Genres, GetChapterPages, GetMangasResponse, GetRawImage, GoToReadChapter,
    HomePageMangaProvider, Languages, MangaPageProvider, PopularManga, ProviderIdentity, RecentlyAddedManga, SearchMangaById,
    SearchPageProvider,
};
use crate::backend::html_parser::{HtmlElement, ParseHtml};

pub static MANGANATO_BASE_URL: &str = "https://manganato.com";

pub mod filter_state;
pub mod filter_widget;
pub mod response;

#[derive(Clone, Debug)]
pub struct ManganatoProvider {
    client: reqwest::Client,
    base_url: Url,
}

impl ManganatoProvider {
    pub fn new(base_url: Url) -> Self {
        let mut default_headers = HeaderMap::new();

        default_headers.insert(
            ACCEPT,
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8,application/json",
            ),
        );

        default_headers.insert(REFERER, HeaderValue::from_static("http://www.google.com"));
        default_headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("gzip, deflate, br"));
        default_headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.5"));

        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
            .build()
            .unwrap();
        Self { client, base_url }
    }

    fn format_search_term(search_term: SearchTerm) -> String {
        let mut search: String = search_term.get().split(" ").map(|word| format!("{word}_")).collect();

        search.pop();

        search
    }
}

impl ProviderIdentity for ManganatoProvider {
    fn name(&self) -> super::MangaProviders {
        super::MangaProviders::Manganato
    }
}

impl GetRawImage for ManganatoProvider {
    async fn get_raw_image(&self, url: &str) -> Result<bytes::Bytes, Box<dyn Error>> {
        let response = self.client.get(url).timeout(Duration::from_secs(3)).send().await?;

        if response.status() != StatusCode::OK {
            return Err(format!("Could not get image with url : {url} on manganato, status code {}", response.status()).into());
        }

        Ok(response.bytes().await?)
    }
}

impl DecodeBytesToImage for ManganatoProvider {}

impl SearchMangaById for ManganatoProvider {
    async fn get_manga_by_id(&self, manga_id: &str) -> Result<super::Manga, Box<dyn Error>> {
        let response = self.client.get(manga_id).send().await?;

        if response.status() != StatusCode::OK {
            return Err(format!("Could not get manga with url {manga_id} on manganato, status code {}", response.status()).into());
        }

        let doc = response.text().await?;

        let manga = MangaPageData::parse_html(HtmlElement::new(doc))?;

        Ok(super::Manga {
            id: manga_id.to_string(),
            title: manga.title,
            genres: manga.genres.into_iter().map(Genres::from).collect(),
            description: manga.description,
            status: manga.status.into(),
            cover_img_url: Some(manga.cover_url.clone()),
            languages: vec![Languages::English],
            rating: manga.rating,
            artist: None,
            author: None,
        })
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
            let from_tool_tip = tool_tip_response.iter().find(|data| data.id == new_manga.id).cloned().unwrap_or_default();
            let manga = RecentlyAddedManga {
                id: new_manga.manga_page_url,
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
        todo!()
    }
}

impl GetChapterPages for ManganatoProvider {
    async fn get_chapter_pages<F: Fn(f64, &str) + 'static + Send>(
        &self,
        chapter_id: &str,
        manga_id: &str,
        image_quality: crate::config::ImageQuality,
        on_progress: F,
    ) -> Result<Vec<super::ChapterPage>, Box<dyn Error>> {
        todo!()
    }

    async fn get_chapter_pages_url(
        &self,
        chapter_id: &str,
        manga_id: &str,
        image_quality: crate::config::ImageQuality,
    ) -> Result<Vec<Url>, Box<dyn Error>> {
        todo!()
    }

    async fn get_chapter_pages_url_with_extension(
        &self,
        chapter_id: &str,
        manga_id: &str,
        image_quality: crate::config::ImageQuality,
    ) -> Result<Vec<super::ChapterPageUrl>, Box<dyn Error>> {
        todo!()
    }
}

impl FetchChapterBookmarked for ManganatoProvider {
    async fn fetch_chapter_bookmarked(
        &self,
        chapter: crate::backend::database::ChapterBookmarked,
    ) -> Result<(super::ChapterToRead, super::ListOfChapters), Box<dyn Error>> {
        todo!()
    }
}

impl MangaPageProvider for ManganatoProvider {
    async fn get_chapters(
        &self,
        manga_id: &str,
        filters: super::ChapterFilters,
        pagination: super::Pagination,
    ) -> Result<super::GetChaptersResponse, Box<dyn Error>> {
        Ok(super::GetChaptersResponse {
            chapters: vec![],
            total_chapters: 10,
        })
    }

    async fn get_all_chapters(&self, manga_id: &str, language: Languages) -> Result<Vec<super::Chapter>, Box<dyn Error>> {
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_term_correctly() {
        let searchterm = SearchTerm::trimmed_lowercased("death note").unwrap();

        assert_eq!("death_note", ManganatoProvider::format_search_term(searchterm));

        let searchterm = SearchTerm::trimmed_lowercased("oshi no ko").unwrap();

        assert_eq!("oshi_no_ko", ManganatoProvider::format_search_term(searchterm));
    }
}
