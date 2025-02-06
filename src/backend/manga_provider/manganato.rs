use std::error::Error;
use std::time::Duration;

use filter_state::{ManganatoFilterState, ManganatoFiltersProvider};
use filter_widget::ManganatoFilterWidget;
use http::header::{ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, CONTENT_TYPE, REFERER};
use http::{HeaderMap, HeaderValue, StatusCode};
use manga_tui::SearchTerm;
use reqwest::{Client, Url};
use response::{GetPopularMangasResponse, NewAddedMangas, SearchMangaResponse, ToolTipItem};

use super::{
    DecodeBytesToImage, GetMangasResponse, GetRawImage, HomePageMangaProvider, PopularManga, ProviderIdentity, RecentlyAddedManga,
    SearchMangaById, SearchPageProvider,
};
use crate::backend::html_parser::HtmlElement;

pub static MANGANATO_BASE_URL: &str = "https://manganato.com";

pub mod filter_state;
pub mod filter_widget;
pub mod response;

trait FromHtml: Sized {
    fn from_html(html: HtmlElement) -> Result<Self, Box<dyn Error>>;
}

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
        todo!()
    }
}

impl HomePageMangaProvider for ManganatoProvider {
    async fn get_popular_mangas(&self) -> Result<Vec<super::PopularManga>, Box<dyn Error>> {
        let response = self.client.get(self.base_url.clone()).send().await?;

        if response.status() != StatusCode::OK {
            return Err("could not".into());
        }

        let doc = response.text().await?;

        let response = GetPopularMangasResponse::from_html(HtmlElement::new(doc))?;

        Ok(response.mangas.into_iter().map(PopularManga::from).collect())
    }

    async fn get_recently_added_mangas(&self) -> Result<Vec<super::RecentlyAddedManga>, Box<dyn Error>> {
        let response = self.client.get(self.base_url.clone()).send().await?;

        if response.status() != StatusCode::OK {
            return Err("could not find recently added mangas on manganato".into());
        }

        let doc = response.text().await?;

        let new_mangas = NewAddedMangas::from_html(HtmlElement::new(doc))?;

        let tool_tip_response = self.client.get(format!("{}/home_tooltips_json", self.base_url)).send().await?;

        if tool_tip_response.status() != StatusCode::OK {
            return Err("could not find recently added mangas on manganato".into());
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

        let result = SearchMangaResponse::from_html(HtmlElement::new(doc))?;

        Ok(GetMangasResponse::from(result))
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
