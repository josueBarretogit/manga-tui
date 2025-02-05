use std::borrow::BorrowMut;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use http::StatusCode;
use reqwest::{Client, Url};
use response::{GetPopularMangasResponse, NewAddedMangas, NewMangaAddedToolTip, ToolTipItem};

use super::{DecodeBytesToImage, GetRawImage, HomePageMangaProvider, PopularManga, RecentlyAddedManga, SearchMangaById};
use crate::backend::html_parser::HtmlElement;

pub static MANGANATO_BASE_URL: &str = "https://manganato.com";

pub mod response;

trait FromHtml: Sized {
    fn from_html(html: HtmlElement) -> Result<Self, Box<dyn Error>>;
}

#[derive(Clone, Debug)]
pub struct ManganatoProvider {
    client: reqwest::Client,
    base_url: Url,
    home_page_doc: Option<HtmlElement>,
}

impl ManganatoProvider {
    pub fn new(base_url: Url) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
            .build()
            .unwrap();
        Self {
            client,
            base_url,
            home_page_doc: None,
        }
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
        match &self.home_page_doc {
            Some(doc) => {
                let response = GetPopularMangasResponse::from_html(doc.clone())?;

                Ok(response.mangas.into_iter().map(PopularManga::from).collect())
            },
            None => {
                let response = self.client.get(self.base_url.clone()).send().await?;

                if response.status() != StatusCode::OK {
                    return Err("could not".into());
                }
                let doc = response.text().await?;

                let response = GetPopularMangasResponse::from_html(HtmlElement::new(doc))?;

                Ok(response.mangas.into_iter().map(PopularManga::from).collect())
            },
        }
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

        Ok(new_mangas
            .mangas
            .into_iter()
            .map(move |manga| {
                let from_tool_tip = tool_tip_response.clone().into_iter().find(|data| data.id == manga.id).unwrap_or_default();
                RecentlyAddedManga {
                    id: manga.manga_page_url,
                    title: from_tool_tip.name,
                    description: from_tool_tip.description,
                    cover_img_url: Some(from_tool_tip.image),
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {}
}
