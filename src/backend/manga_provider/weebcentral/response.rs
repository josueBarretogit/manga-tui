use std::collections::HashMap;
use std::error::Error;
use std::fmt::Display;
use std::num::ParseIntError;

use chrono::NaiveDate;
use regex::Regex;
use scraper::selectable::Selectable;
use scraper::{Selector, html};
use serde::{Deserialize, Serialize};

use crate::backend::html_parser::scraper::AsSelector;
use crate::backend::html_parser::{HtmlElement, ParseHtml};
use crate::backend::manga_provider::{
    ChapterReader, Genres, GetMangasResponse, ListOfChapters, MangaStatus, PopularManga, Rating, SearchManga, SortedChapters,
    SortedVolumes, Volumes,
};

#[derive(Debug)]
pub(super) struct PopularMangaParseError {
    reason: String,
}

impl Display for PopularMangaParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to parse popular manga from weebcentral, more details about the error: {}", self.reason)
    }
}

impl<T: Into<String>> From<T> for PopularMangaParseError {
    fn from(value: T) -> Self {
        let reason: String = value.into();
        Self { reason }
    }
}

impl Error for PopularMangaParseError {}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct PopularMangaItem {
    pub(super) page_url: String,
    pub(super) cover_url: String,
    pub(super) title: String,
    pub(super) latest_chapter: Option<String>,
}

impl From<PopularMangaItem> for PopularManga {
    fn from(manga: PopularMangaItem) -> Self {
        Self {
            id: manga.page_url,
            title: manga.title,
            genres: vec![],
            description: manga.latest_chapter.unwrap_or_default(),
            status: None,
            cover_img_url: manga.cover_url,
        }
    }
}

/// How to scrape the popoular mangas from weebcentral:
/// - The `section` which contains the mangas is the first one
#[derive(Debug)]
pub(super) struct PopularMangasWeebCentral {
    pub(super) mangas: Vec<PopularMangaItem>,
}

impl ParseHtml for PopularMangaItem {
    type ParseError = PopularMangaParseError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let div = html::Html::parse_fragment(html.as_str());

        let img = div.select(&"img".as_selector()).next().ok_or("No image found")?;

        let article = div
            .select(&"article".as_selector())
            .next()
            .ok_or("The tag article containing most info was not found")?;

        let cover_url = img.attr("src").ok_or("no source of image found")?.to_string();

        let page_url = article.select(&"a".as_selector()).next().ok_or("no tag containing page url found")?;
        let page_url = page_url.attr("href").ok_or("no href found")?.to_string();

        let title_selector = ".truncate".as_selector();

        let title = article.select(&title_selector).next().ok_or("no title was found")?.inner_html();

        let latest_chapter = article.select(&title_selector).last().map(|tag| tag.inner_html());

        Ok(Self {
            page_url,
            cover_url,
            title,
            latest_chapter,
        })
    }
}

impl ParseHtml for PopularMangasWeebCentral {
    type ParseError = PopularMangaParseError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let doc = html::Html::parse_document(html.as_str());
        let section_containing_mangas_selector = "main > section".as_selector();

        let mut mangas: Vec<Result<PopularMangaItem, <PopularMangaItem as ParseHtml>::ParseError>> = vec![];

        let section_containing_mangas = doc
            .select(&section_containing_mangas_selector)
            .next()
            .ok_or("The section containing mangas was not found")?;

        let mangas_selector = "abbr".as_selector();

        for div in section_containing_mangas.select(&mangas_selector) {
            mangas.push(PopularMangaItem::parse_html(HtmlElement::new(div.html())));
        }

        Ok(Self {
            mangas: mangas.into_iter().map(|may| may.unwrap()).collect(),
        })
    }
}

#[derive(Debug)]
pub(super) struct LatestMangaError {
    reason: String,
}

impl Display for LatestMangaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to parse latest manga from weebcentral, more details about the error: {}", self.reason)
    }
}

impl<T: Into<String>> From<T> for LatestMangaError {
    fn from(value: T) -> Self {
        let reason: String = value.into();
        Self { reason }
    }
}

impl Error for LatestMangaError {}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct LatestMangItem {
    pub(super) page_url: String,
    pub(super) cover_url: String,
    pub(super) title: String,
    pub(super) latest_chapter: Option<String>,
}

/// How to scrape the latest mangas from weebcentral:
/// - The `section` which contains the mangas is the second one
#[derive(Debug, Default)]
pub(super) struct LatestMangas {
    pub(super) mangas: Vec<LatestMangItem>,
}

impl ParseHtml for LatestMangItem {
    type ParseError = LatestMangaError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let div = html::Html::parse_fragment(html.as_str());

        let img = div.select(&"img".as_selector()).next().ok_or("No image found")?;

        let article = div
            .select(&"article".as_selector())
            .next()
            .ok_or("The tag article containing most info was not found")?;

        let cover_url = img.attr("src").ok_or("no source of image found")?.to_string();

        let page_url = article.select(&"a".as_selector()).next().ok_or("no tag containing page url found")?;
        let page_url = page_url.attr("href").ok_or("no href found")?.to_string();

        let title_selector = ".truncate".as_selector();

        let title = article.select(&title_selector).next().ok_or("no title was found")?.inner_html();

        let latest_chapter = article.select(&title_selector).last().map(|tag| tag.inner_html());

        Ok(Self {
            page_url,
            cover_url,
            title,
            latest_chapter,
        })
    }
}

impl ParseHtml for LatestMangas {
    type ParseError = PopularMangaParseError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let doc = html::Html::parse_document(html.as_str());
        let section_containing_mangas_selector = "main > section".as_selector();

        let mut mangas: Vec<Result<PopularMangaItem, <PopularMangaItem as ParseHtml>::ParseError>> = vec![];

        let section_containing_mangas = doc
            .select(&section_containing_mangas_selector)
            .next()
            .ok_or("The section containing mangas was not found")?;

        let mangas_selector = "abbr".as_selector();

        for div in section_containing_mangas.select(&mangas_selector) {
            mangas.push(PopularMangaItem::parse_html(HtmlElement::new(div.html())));
        }

        Ok(Self::default())
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use fake::rand::seq::IndexedRandom;
    use pretty_assertions::assert_eq;
    use scraper::Html;

    use super::{PopularMangaItem, PopularMangasWeebCentral};
    use crate::backend::html_parser::{HtmlElement, ParseHtml};

    static HOME_PAGE_DOC: &str = include_str!("../../../../data_test/weebcentral/home_page.txt");

    #[test]
    fn popular_manga_is_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = HOME_PAGE_DOC;

        let expected: PopularMangaItem = PopularMangaItem {
            page_url: "https://weebcentral.com/chapters/01JP3RJKND66Q25D590768N7XQ".to_string(),
            cover_url: "https://temp.compsci88.com/cover/fallback/01J76XY7HF84B23C36Q8536BP7.jpg".to_string(),
            title: "Hajime no Ippo".to_string(),
            latest_chapter: Some("Round 1486".to_string()),
        };

        let result = PopularMangasWeebCentral::parse_html(HtmlElement::new(html))?;

        assert!(result.mangas.len() > 1);

        let manga = result.mangas.iter().find(|man| man.page_url == expected.page_url).unwrap();

        assert_eq!(expected, *manga);

        Ok(())
    }

    #[test]
    fn latest_manga_is_parsed_from_html() {
        let html = HOME_PAGE_DOC;
    }
}
