use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Write};
use std::num::ParseIntError;

use chrono::NaiveDate;
use regex::Regex;
use scraper::selectable::Selectable;
use scraper::{element_ref, html, Selector};
use serde::{Deserialize, Serialize};

use super::{MangakakalotProvider, MANGAKAKALOT_BASE_URL};
use crate::backend::html_parser::scraper::AsSelector;
use crate::backend::html_parser::{HtmlElement, ParseHtml};
use crate::backend::manga_provider::{
    ChapterReader, Genres, GetMangasResponse, ListOfChapters, MangaStatus, PopularManga, Rating, RecentlyAddedManga, SearchManga,
    SortedChapters, SortedVolumes, Volumes,
};

pub(super) fn extract_id_from_url<T: AsRef<str>>(url: T) -> String {
    let as_string: &str = url.as_ref();
    let as_string = as_string.split("/").last().unwrap_or_default();
    as_string.to_string()
}

/// from a string like: mangakakalot.gg rate : 3.27 / 5 - 87 votes
/// extract the "3.27"
pub(super) fn extract_rating<T: AsRef<str>>(text: T) -> String {
    let s = text.as_ref();
    let parts = s.split(":").last().unwrap_or_default().trim();
    parts.split(" ").next().unwrap_or_default().to_string()
}

#[derive(Debug, Default, Clone, PartialEq)]
struct ChapterTitle<'a> {
    title: Option<&'a str>,
    volume_number: Option<&'a str>,
    number: &'a str,
}

pub(super) fn from_timestamp(timestamp: i64) -> Option<NaiveDate> {
    chrono::DateTime::from_timestamp(timestamp, 0).map(|time| time.date_naive())
}

fn extract_chapter_title(raw_title: &str) -> ChapterTitle<'_> {
    let volume_regex = Regex::new(r"Vol\.(\d+)").unwrap();
    let title_regex = Regex::new(r"Chapter \d+: (.+)").unwrap();
    let number_regex = Regex::new(r"Chapter (\d+(\.\d+)?)").unwrap();

    let volume = volume_regex.captures(raw_title).and_then(|caps| caps.get(1)).map(|m| m.as_str());

    let title = title_regex.captures(raw_title).and_then(|caps| caps.get(1)).map(|m| m.as_str());

    let number = number_regex
        .captures(raw_title)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str())
        .unwrap_or("0");
    ChapterTitle {
        title,
        volume_number: volume,
        number,
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct PopularMangaItem {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) cover_img_url: String,
    pub(super) additional_data: String,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(super) struct ManganatoGenre {
    pub(super) name: String,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(super) struct ManganatoStatus {
    pub(super) name: String,
}

#[derive(Debug)]
pub(super) struct PopularMangaItemError {
    reason: String,
}

impl<T: Into<String>> From<T> for PopularMangaItemError {
    fn from(value: T) -> Self {
        let reason: String = value.into();
        Self { reason }
    }
}

impl Display for PopularMangaItemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not parse html to get popular manga on manganato, reason: {}", self.reason)
    }
}

impl Error for PopularMangaItemError {}

impl ParseHtml for PopularMangaItem {
    type ParseError = PopularMangaItemError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let div = html::Html::parse_fragment(html.as_str());
        let a_selector = ".item div h3 a".as_selector();
        let a_selector_additional_info = ".item div a".as_selector();
        let img_selector = Selector::parse(".item img").unwrap();

        let a_tag = div.select(&a_selector).next().ok_or("Could not find div element containing manga info")?;
        let a_tag_additional_info = div
            .select(&a_selector_additional_info)
            .last()
            .ok_or("could not find a tag containing additional information")?;
        let title = a_tag.attr("title").ok_or("Could not find manga title")?;
        let manga_page_url = a_tag.attr("href").ok_or("Could not find manga page url")?;

        let additiona_info = a_tag_additional_info.inner_html().trim().to_string();

        let img_element = div
            .select(&img_selector)
            .next()
            .ok_or("Could not find the img element containing the cover")?;

        let cover_img_url = img_element.attr("src").ok_or("Could not find the cover img url")?;

        Ok(Self {
            id: manga_page_url.to_string(),
            title: title.to_string(),
            cover_img_url: cover_img_url.to_string(),
            additional_data: format!("Latest chapter: {additiona_info}"),
        })
    }
}

impl From<PopularMangaItem> for PopularManga {
    fn from(value: PopularMangaItem) -> Self {
        PopularManga {
            id: value.id.to_string(),
            title: value.title.to_string(),
            genres: vec![],
            description: value.additional_data,
            status: None,
            cover_img_url: value.cover_img_url.to_string(),
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct GetPopularMangasResponse {
    pub(super) mangas: Vec<PopularMangaItem>,
}

impl ParseHtml for GetPopularMangasResponse {
    type ParseError = PopularMangaItemError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let doc = html::Html::parse_document(html.as_str());
        let selector = Selector::parse(".owl-carousel > *").unwrap();

        let mut mangas: Vec<Result<PopularMangaItem, <PopularMangaItem as ParseHtml>::ParseError>> = vec![];

        for child in doc.select(&selector) {
            mangas.push(PopularMangaItem::parse_html(HtmlElement::new(child.html())));
        }

        Ok(Self {
            mangas: mangas.into_iter().flatten().collect(),
        })
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(super) struct NewMangaAddedItem {
    pub(super) page_url: String,
    pub(super) title: String,
    pub(super) cover_img_url: String,
    pub(super) latest_chapters: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub(super) struct NewAddedMangas {
    pub(super) mangas: Vec<NewMangaAddedItem>,
}

#[derive(Debug)]
pub(super) struct NewAddedMangasError {
    reason: String,
}

impl<T: Into<String>> From<T> for NewAddedMangasError {
    fn from(value: T) -> Self {
        let reason: String = value.into();
        Self { reason }
    }
}

impl Display for NewAddedMangasError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not parse html to recently added mangason manganato, reason: {}", self.reason)
    }
}

impl Error for NewAddedMangasError {}

impl ParseHtml for NewMangaAddedItem {
    type ParseError = NewAddedMangasError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let div = html::Html::parse_fragment(html.as_str());

        let url_selector = "h3 > a".as_selector();

        let a_tag = div.select(&url_selector).next().ok_or("tag a was not found")?;

        let page_url = a_tag.attr("href").ok_or("could not find page url")?.to_string();

        let title = a_tag.inner_html().trim().to_string();

        let img_selector = "img".as_selector();

        let cover_img_url = div
            .select(&img_selector)
            .next()
            .and_then(|tag| tag.attr("src"))
            .ok_or("no img tag was found")?
            .to_string();

        let chapters_selector = "li > span > a".as_selector();

        let latest_chapters: Vec<String> = div.select(&chapters_selector).map(|tag| tag.inner_html()).collect();

        Ok(Self {
            page_url,
            title,
            cover_img_url,
            latest_chapters,
        })
    }
}

impl From<NewMangaAddedItem> for RecentlyAddedManga {
    fn from(value: NewMangaAddedItem) -> Self {
        let description: String = value.latest_chapters.into_iter().fold(String::new(), |mut chap, next| {
            let _ = write!(chap, "\n{}", next);
            chap
        });
        Self {
            id: value.page_url,
            title: value.title,
            description,
            cover_img_url: value.cover_img_url,
        }
    }
}

impl ParseHtml for NewAddedMangas {
    type ParseError = NewAddedMangasError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let doc = html::Html::parse_document(html.as_str());

        let selector = ".doreamon > div".as_selector();

        let mut mangas: Vec<Result<NewMangaAddedItem, <NewMangaAddedItem as ParseHtml>::ParseError>> = vec![];

        for child in doc.select(&selector).take(5) {
            mangas.push(NewMangaAddedItem::parse_html(HtmlElement::new(child.html())));
        }

        Ok(NewAddedMangas {
            mangas: mangas.into_iter().flatten().collect(),
        })
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct SearchMangaItem {
    pub(super) page_url: String,
    pub(super) cover_url: String,
    pub(super) title: String,
    pub(super) latest_chapters: Vec<String>,
    pub(super) author: Option<String>,
}

#[derive(Debug)]
pub(super) struct SearchMangaResponseError {
    reason: String,
}

impl<T: Into<String>> From<T> for SearchMangaResponseError {
    fn from(value: T) -> Self {
        let reason: String = value.into();
        Self { reason }
    }
}

impl Display for SearchMangaResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not parse html to search mangas on manganato, reason: {}", self.reason)
    }
}

impl Error for SearchMangaResponseError {}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct SearchMangaResponse {
    pub(super) mangas: Vec<SearchMangaItem>,
    pub(super) total_mangas: u32,
}

impl ParseHtml for SearchMangaResponse {
    type ParseError = SearchMangaResponseError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let doc = html::Html::parse_document(html.as_str());
        let check_search_is_not_found = Selector::parse(".panel_story_list").unwrap();

        if doc.select(&check_search_is_not_found).next().is_none() {
            return Ok(SearchMangaResponse {
                mangas: vec![],
                total_mangas: 0,
            });
        }

        // at this point search contains mangas

        let selector_div_containing_mangas = ".panel_story_list > *".as_selector();

        let selector_total_mangas = ".panel_page_number > .group_qty > div".as_selector();

        let mut mangas: Vec<Result<SearchMangaItem, <SearchMangaItem as ParseHtml>::ParseError>> = vec![];

        for div in doc.select(&selector_div_containing_mangas) {
            mangas.push(SearchMangaItem::parse_html(HtmlElement::new(div.html())));
        }

        let maybe_total_mangas = doc.select(&selector_total_mangas).next();

        // if this tag is not present then there is only one page
        let total_mangas: u32 = if let Some(total) = maybe_total_mangas {
            let total_mangas: Result<u32, ParseIntError> = {
                let total_mangas = total.inner_html();

                let total_mangas = total_mangas.split(" ").skip(1).next().ok_or("total mangas number was not found")?;
                total_mangas.parse()
            };

            total_mangas.map_err(|e| e.to_string())?
        } else {
            mangas.len() as u32
        };

        Ok(Self {
            mangas: mangas.into_iter().flatten().collect(),
            total_mangas,
        })
    }
}

#[derive(Debug)]
pub(super) struct SearchMangaItemError {
    reason: String,
}

impl<T: Into<String>> From<T> for SearchMangaItemError {
    fn from(value: T) -> Self {
        let reason: String = value.into();
        Self { reason }
    }
}

impl Display for SearchMangaItemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not parse html to get manga item on search on manganato, reason: {}", self.reason)
    }
}

impl Error for SearchMangaItemError {}

impl ParseHtml for SearchMangaItem {
    type ParseError = SearchMangaItemError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let div = html::Html::parse_fragment(html.as_str());

        let img_selector = "img".as_selector();

        let img = div.select(&img_selector).next().ok_or("no img")?;
        let cover_url = img.attr("src").ok_or("no cover")?;

        let a_containing_manga_page_url = "a".as_selector();
        let a_manga_page_url = div.select(&a_containing_manga_page_url).next().ok_or("no a tag found")?;

        let title = div.select(&".story_name > a".as_selector()).next().ok_or("no title")?.inner_html();
        let manga_page_url = a_manga_page_url.attr("href").ok_or("no href")?;

        let latest_chapters_selector = ".story_chapter > a".as_selector();
        let latest_chapters: Vec<String> =
            div.select(&latest_chapters_selector).map(|a| a.inner_html().trim().to_string()).collect();

        let author = div
            .select(&"span".as_selector())
            .next()
            .map(|tag| tag.inner_html().trim().split(":").last().unwrap_or_default().to_string());

        Ok(Self {
            page_url: manga_page_url.to_string(),
            title: title.to_string(),
            cover_url: cover_url.to_string(),
            latest_chapters,
            author,
        })
    }
}

impl From<SearchMangaItem> for SearchManga {
    fn from(value: SearchMangaItem) -> Self {
        Self {
            id: value.page_url,
            title: value.title,
            genres: vec![],
            description: Some(value.latest_chapters.into_iter().fold(String::new(), |mut word, acc| {
                let _ = write!(word, "\n{}", acc);
                word
            })),
            status: None,
            cover_img_url: value.cover_url,
            languages: MangakakalotProvider::MANGANATO_MANGA_LANGUAGE.into(),
            artist: None,
            author: None,
        }
    }
}

impl From<SearchMangaResponse> for GetMangasResponse {
    fn from(value: SearchMangaResponse) -> Self {
        Self {
            mangas: value.mangas.into_iter().map(SearchManga::from).collect(),
            total_mangas: value.total_mangas,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(super) struct MangaPageData {
    pub(super) title: String,
    pub(super) authors: Option<String>,
    pub(super) status: ManganatoStatus,
    pub(super) genres: Vec<ManganatoGenre>,
    pub(super) rating: String,
    pub(super) cover_url: String,
    pub(super) description: String,
}

#[derive(Debug)]
pub(super) struct MangaPageDataError {
    reason: String,
}

impl<T: Into<String>> From<T> for MangaPageDataError {
    fn from(value: T) -> Self {
        let reason: String = value.into();
        Self { reason }
    }
}

impl Display for MangaPageDataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not parse html to get manga on manganato, reason: {}", self.reason)
    }
}

impl Error for MangaPageDataError {}

impl ParseHtml for MangaPageData {
    type ParseError = MangaPageDataError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let div = html::Html::parse_fragment(html.as_str());
        let right_div_selector = Selector::parse(".manga-info-text").unwrap();

        let right_div_containing_most_info = div
            .select(&right_div_selector)
            .next()
            .ok_or("The div containing most information on the manga was not found")?;

        let title_selector = Selector::parse("h1").unwrap();

        let title = right_div_containing_most_info
            .select(&title_selector)
            .next()
            .ok_or("No title found")?
            .inner_html();

        let li_selector = "li".as_selector();
        let genres_selector = ".genres > a".as_selector();

        let lis: Vec<element_ref::ElementRef<'_>> = right_div_containing_most_info.select(&li_selector).skip(1).take(2).collect();
        let genres = right_div_containing_most_info
            .select(&genres_selector)
            .map(|tag| ManganatoGenre {
                name: tag.inner_html().trim().to_string(),
            })
            .collect();

        let authors = lis
            .get(0)
            .and_then(|tag| tag.select(&"a".as_selector()).next())
            .and_then(|tag| Some(tag.inner_html()));

        let status = lis
            .get(1)
            .and_then(|tag| {
                Some(ManganatoStatus {
                    name: tag.inner_html().split(":").last().unwrap_or_default().trim().to_string(),
                })
            })
            .unwrap_or_default();

        let rating_selector = "#rate_row_cmd".as_selector();
        let rating = right_div_containing_most_info
            .select(&rating_selector)
            .next()
            .ok_or("no rating tag")?
            .inner_html();

        let rating = format!("{} out of 5", extract_rating(rating));

        let img_selector = Selector::parse(".manga-info-pic img").unwrap();

        let cover_url = div
            .select(&img_selector)
            .next()
            .ok_or("no img tag found")?
            .attr("src")
            .ok_or("no src attribute on img tag")?
            .to_string();

        let description_selector = Selector::parse("#contentBox").unwrap();
        let description_div = div.select(&description_selector).next().ok_or("no description tag found")?;
        let mut description = String::new();

        // This is how we can obtain the inner text of a element without whithout tags
        for node in html::Html::parse_fragment(&description_div.inner_html()).tree {
            if let scraper::node::Node::Text(text) = node {
                description = text.to_string().trim().to_string();
            }
        }

        Ok(Self {
            description,
            title,
            authors,
            status,
            genres,
            rating,
            cover_url,
        })
    }
}

impl From<ManganatoGenre> for Genres {
    fn from(value: ManganatoGenre) -> Self {
        let rating = match value.name.to_lowercase().as_str() {
            "smut" => Rating::Nsfw,
            "adult" => Rating::Nsfw,
            "erotica" => Rating::Nsfw,
            "pornographic" => Rating::Nsfw,
            "ecchi" => Rating::Moderate,
            "doujinshi" => Rating::Doujinshi,
            "mature" => Rating::Nsfw,
            _ => Rating::Normal,
        };

        Genres::new(value.name, rating)
    }
}

impl From<ManganatoStatus> for MangaStatus {
    fn from(value: ManganatoStatus) -> Self {
        match value.name.to_lowercase().as_str() {
            "ongoing" => MangaStatus::Ongoing,
            "completed" => MangaStatus::Completed,
            _ => MangaStatus::Ongoing,
        }
    }
}

#[derive(Debug)]
pub(super) struct ChapterParseError {
    reason: String,
}

impl<T: Into<String>> From<T> for ChapterParseError {
    fn from(value: T) -> Self {
        let reason: String = value.into();
        Self { reason }
    }
}

impl Display for ChapterParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not parse html to get chapter on manganato, reason: {}", self.reason)
    }
}

impl Error for ChapterParseError {}

/// Represent a chapter item displayed in `pages/manga.rs`
#[derive(Debug, Default, Clone, PartialEq)]
pub(super) struct ManganatoChapter {
    pub(super) page_url: String,
    pub(super) title: Option<String>,
    pub(super) number: String,
    pub(super) volume: Option<String>,
    pub(super) uploaded_at: String,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(super) struct ManganatoChaptersResponse {
    pub(super) chapters: Vec<ManganatoChapter>,
    pub(super) total_chapters: u32,
}

impl ParseHtml for ManganatoChapter {
    type ParseError = ChapterParseError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let div = html::Html::parse_fragment(html.as_str());
        let a_selector = "a".as_selector();

        let a_tag = div.select(&a_selector).next().ok_or("no a tag found")?;

        let page_url = a_tag.attr("href").ok_or("no href on tag a")?;
        let chapter_title = a_tag.inner_html().trim().to_string();
        let chapter_title = extract_chapter_title(&chapter_title);

        let uploaded_at = div
            .select(&"span".as_selector())
            .last()
            .and_then(|span| span.attr("title"))
            .ok_or("no uploaded at info was found")?
            .to_string();

        Ok(Self {
            page_url: page_url.to_string(),
            title: None,
            number: chapter_title.number.to_string(),
            volume: None,
            uploaded_at,
        })
    }
}

impl ParseHtml for ManganatoChaptersResponse {
    type ParseError = ChapterParseError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let div = html::Html::parse_fragment(html.as_str());
        let chapters_selector = Selector::parse(".chapter-list > div").unwrap();

        let mut chapters: Vec<Result<ManganatoChapter, <ManganatoChapter as ParseHtml>::ParseError>> = vec![];

        for li_chapter in div.select(&chapters_selector) {
            chapters.push(ManganatoChapter::parse_html(HtmlElement::new(li_chapter.html())));
        }

        let chapters: Vec<ManganatoChapter> = chapters.into_iter().flatten().collect();

        let total_chapters = chapters.len() as u32;

        Ok(Self {
            chapters,
            total_chapters,
        })
    }
}

#[derive(Debug)]
pub(super) struct ChapterPageError {
    reason: String,
}

impl<T: Into<String>> From<T> for ChapterPageError {
    fn from(value: T) -> Self {
        let reason: String = value.into();
        Self { reason }
    }
}

impl Display for ChapterPageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not parse html to get chapter page on manganato, reason: {}", self.reason)
    }
}

impl Error for ChapterPageError {}

#[derive(Debug, Default, Clone, PartialEq)]
pub(super) struct ChapterUrls {
    pub(super) urls: Vec<String>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(super) struct ChapterPageResponse {
    pub(super) title: Option<String>,
    pub(super) number: String,
    pub(super) volume_number: Option<String>,
    pub(super) pages_url: ChapterUrls,
    pub(super) chapters_list: ChaptersList,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(super) struct ChaptersListItem {
    pub(super) page_url: String,
    pub(super) number: String,
    pub(super) title: Option<String>,
    pub(super) volume_number: Option<String>,
}

/// This represents the list of pages in the chapter page
#[derive(Debug, Default, Clone, PartialEq)]
pub(super) struct ChaptersList {
    pub(super) chapters: Vec<ChaptersListItem>,
}

impl ParseHtml for ChapterUrls {
    type ParseError = ChapterPageError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let mut urls: Vec<String> = vec![];
        let doc = html::Html::parse_document(html.as_str());

        let img_selector = ".container-chapter-reader img".as_selector();

        for img in doc.select(&img_selector) {
            let url = img.attr("src").unwrap_or_default();
            urls.push(url.to_string());
        }

        Ok(Self { urls })
    }
}

impl ParseHtml for ChaptersList {
    type ParseError = ChapterPageError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let doc = html::Html::parse_document(html.as_str());

        let chapters_list_selector = ".navi-change-chapter".as_selector();

        let select_containing_chapters = doc
            .select(&chapters_list_selector)
            .next()
            .ok_or("the select containing the chapter list was not found")?;

        let mut chapters: Vec<ChaptersListItem> = vec![];

        for item in select_containing_chapters.select(&"option".as_selector()) {
            let number = item.inner_html();
            let page_url = item.attr("data-c").unwrap_or_default();

            chapters.push(ChaptersListItem {
                page_url: format!("{MANGAKAKALOT_BASE_URL}{page_url}"),
                number: number.trim().split(" ").last().unwrap_or_default().to_string(),
                title: None,
                volume_number: None,
            });
        }

        Ok(ChaptersList { chapters })
    }
}

impl ParseHtml for ChapterPageResponse {
    type ParseError = ChapterPageError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let doc = html::Html::parse_document(html.as_str());

        let title_selector_div = "h2".as_selector();

        let raw_title = doc.select(&title_selector_div).next().ok_or("no title tag was found")?.inner_html();
        let number = raw_title.trim().split(" ").last().ok_or("no chapter number was found")?;

        Ok(Self {
            title: None,
            number: number.to_string(),
            volume_number: None,
            pages_url: ChapterUrls::parse_html(HtmlElement::new(html.as_str()))?,
            chapters_list: ChaptersList::parse_html(HtmlElement::new(html.as_str()))?,
        })
    }
}

impl From<ChaptersList> for ListOfChapters {
    fn from(value: ChaptersList) -> Self {
        let mut volumes: HashMap<String, Vec<ChapterReader>> = HashMap::new();
        for chap in value.chapters {
            match chap.volume_number {
                Some(vol_number) => {
                    let already_existing_volume = volumes.entry(vol_number.clone()).or_default();

                    already_existing_volume.push(ChapterReader {
                        volume: vol_number,
                        number: chap.number,
                        id: chap.page_url,
                    });
                },
                None => {
                    let already_existing_volume = volumes.entry("none".to_string()).or_default();

                    already_existing_volume.push(ChapterReader {
                        volume: "none".to_string(),
                        number: chap.number,
                        id: chap.page_url,
                    });
                },
            }
        }

        let mut volumes_to_sort: Vec<Volumes> = vec![];

        for vol in volumes {
            volumes_to_sort.push(Volumes {
                volume: vol.0,
                chapters: SortedChapters::new(vol.1),
            });
        }

        Self {
            volumes: SortedVolumes::new(volumes_to_sort),
        }
    }
}
#[cfg(test)]
mod tests {
    use std::error::Error;

    use chrono::Datelike;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_extracts_id_from_url() {
        let url = "https://manganato.com/manga-ck979419";

        assert_eq!("manga-ck979419", extract_id_from_url(url));

        let url = "https://chapmanganato.to/manga-pe992361";

        assert_eq!("manga-pe992361", extract_id_from_url(url));

        let url = "https://chapmanganato.to/manga-gv952204";

        assert_eq!("manga-gv952204", extract_id_from_url(url));
    }

    #[test]
    fn it_extract_chapter_title() {
        let title = "Vol.1 Chapter 6";

        let expected = ChapterTitle {
            volume_number: Some("1"),
            number: "6",
            title: None,
        };

        assert_eq!(expected, extract_chapter_title(title));

        let title = "Vol.10 Chapter 20: Hostile Relationship";

        let expected = ChapterTitle {
            volume_number: Some("10"),
            number: "20",
            title: Some("Hostile Relationship"),
        };

        assert_eq!(expected, extract_chapter_title(title));

        let title = "Chapter 71.1";

        let expected = ChapterTitle {
            volume_number: None,
            number: "71.1",
            title: None,
        };

        assert_eq!(expected, extract_chapter_title(title));

        let title = "Chapter 86: Once I Realized It, I Was In A Game";

        let expected = ChapterTitle {
            volume_number: None,
            number: "86",
            title: Some("Once I Realized It, I Was In A Game"),
        };

        assert_eq!(expected, extract_chapter_title(title));
    }

    #[test]
    fn convert_number_to_datetime() {
        // Sep 29,2024 16:09
        let number = 1727625860;

        // September
        assert_eq!(9, from_timestamp(number).unwrap_or_default().month());
    }

    #[test]
    fn it_extracts_rating_from_text() {
        let example = "mangakakalot.gg rate : 3.27 / 5 - 87 votes";

        assert_eq!("3.27", extract_rating(example));

        let example = "mangakakalot.gg rate : 1 / 5 - 1 votes";

        assert_eq!("1", extract_rating(example));

        let example = "mangakakalot.gg rate : 1.4 / 5 - 10 votes";

        assert_eq!("1.4", extract_rating(example));

        let example = "mangakakalot.gg rate : 3 / 5 - 10 votes";

        assert_eq!("3", extract_rating(example));
    }

    #[test]
    fn popular_manga_item_parses_from_html() -> Result<(), Box<dyn Error>> {
        let html = r#"
                <div class="item">
                    <img src="https://imgs-2.2xstorage.com/thumb/the-ruthless-boss-can-only-cry-after-being-toyed-with.webp"
                        loading="lazy" onerror="javascript:this.src='/images/404-avatar.webp';"
                        alt="The Ruthless Boss Can Only Cry After Being Toyed With">
                    <div class="slide-caption">
                        <h3>
                            <a href="https://www.mangakakalot.gg/manga/the-ruthless-boss-can-only-cry-after-being-toyed-with"
                                title="The Ruthless Boss Can Only Cry After Being Toyed With">
                                The Ruthless Boss Can Only Cry After Being Toyed With
                            </a>
                        </h3>
                        <a href="https://www.mangakakalot.gg/manga/the-ruthless-boss-can-only-cry-after-being-toyed-with/chapter-3"
                            title="Chapter 3">Chapter 3
                        </a>
                    </div>
                </div>
        "#;

        let expected: PopularMangaItem = PopularMangaItem {
            id: "https://www.mangakakalot.gg/manga/the-ruthless-boss-can-only-cry-after-being-toyed-with".to_string(),
            title: "The Ruthless Boss Can Only Cry After Being Toyed With".to_string(),
            cover_img_url: "https://imgs-2.2xstorage.com/thumb/the-ruthless-boss-can-only-cry-after-being-toyed-with.webp"
                .to_string(),
            additional_data: "Latest chapter: Chapter 3".to_string(),
        };

        let popular_manga = PopularMangaItem::parse_html(HtmlElement::new(html.to_string()))?;

        assert_eq!(expected, popular_manga);

        Ok(())
    }

    #[test]
    fn popular_manga_response_gets_inner_items() -> Result<(), Box<dyn Error>> {
        let html = r#"
                <div id="owl-demo" class="owl-carousel">
                    <div class="item">
                        <img src="https://imgs-2.2xstorage.com/thumb/the-ruthless-boss-can-only-cry-after-being-toyed-with.webp"
                            loading="lazy" onerror="javascript:this.src='/images/404-avatar.webp';"
                            alt="The Ruthless Boss Can Only Cry After Being Toyed With">
                        <div class="slide-caption">
                            <h3>
                                <a href="https://www.mangakakalot.gg/manga/the-ruthless-boss-can-only-cry-after-being-toyed-with"
                                    title="The Ruthless Boss Can Only Cry After Being Toyed With">
                                    The Ruthless Boss Can Only Cry After Being Toyed With
                                </a>
                            </h3>
                            <a href="https://www.mangakakalot.gg/manga/the-ruthless-boss-can-only-cry-after-being-toyed-with/chapter-3"
                                title="Chapter 3">Chapter 3
                            </a>
                        </div>
                    </div>
                    <div class="item">
                        <img src="https://imgs-2.2xstorage.com/thumb/the-ruthless-boss-can-only-cry-after-being-toyed-with.webp"
                            loading="lazy" onerror="javascript:this.src='/images/404-avatar.webp';"
                            alt="The Ruthless Boss Can Only Cry After Being Toyed With">
                        <div class="slide-caption">
                            <h3>
                                <a href="https://www.mangakakalot.gg/manga/the-ruthless-boss-can-only-cry-after-being-toyed-with"
                                    title="The Ruthless Boss Can Only Cry After Being Toyed With">
                                    The Ruthless Boss Can Only Cry After Being Toyed With
                                </a>
                            </h3>
                            <a href="https://www.mangakakalot.gg/manga/the-ruthless-boss-can-only-cry-after-being-toyed-with/chapter-3"
                                title="Chapter 3">Chapter 3
                            </a>
                        </div>
                    </div>
                </div>
        "#;

        let response = GetPopularMangasResponse::parse_html(HtmlElement::new(html.to_string()))?;

        assert_eq!(2, response.mangas.len());

        Ok(())
    }

    #[test]
    fn latest_manga_update_parses_from_html() -> Result<(), Box<dyn Error>> {
        let html = r#"
                <div class="doreamon">
                        <div class="itemupdate first">
                            <a data-tooltip="sticky_53064" data-id="53064" class="tooltip cover bookmark_check"
                                href="https://www.mangakakalot.gg/manga/my-level-up-is-strange-reincarnation-of-a-great-man-in-a-different-world">
                                <img src="https://imgs-2.2xstorage.com/thumb/my-level-up-is-strange-reincarnation-of-a-great-man-in-a-different-world.webp"
                                    data-src="https://imgs-2.2xstorage.com/thumb/my-level-up-is-strange-reincarnation-of-a-great-man-in-a-different-world.webp"
                                    alt="My level up is strange! ~ Reincarnation of a great Man in a Different World ["
                                    width="60" height="85" class="lazy">
                            </a>
                            <ul>
                                <li>
                                    <h3>
                                    <a class="tooltip" data-tooltip="sticky_53064"
                                            href="https://www.mangakakalot.gg/manga/my-level-up-is-strange-reincarnation-of-a-great-man-in-a-different-world">My level up is strange! ~ Reincarnation of a great Man in a Different World [</a>
                                    </h3>
                                </li>
                                <li>
                                    <span><a class="sts sts_1"
                                            href="https://www.mangakakalot.gg/manga/my-level-up-is-strange-reincarnation-of-a-great-man-in-a-different-world/chapter-17"
                                            title="Chapter 17">Chapter 17</a></span>
                                    <i>14 minute ago</i>
                                </li>
                                <li>
                                    <span><a class="sts sts_1"
                                            href="https://www.mangakakalot.gg/manga/my-level-up-is-strange-reincarnation-of-a-great-man-in-a-different-world/chapter-16"
                                            title="Chapter 16">Chapter 16</a></span>
                                    <i>14 minute ago</i>
                                </li>
                                <li>
                                    <span><a class="sts sts_1"
                                            href="https://www.mangakakalot.gg/manga/my-level-up-is-strange-reincarnation-of-a-great-man-in-a-different-world/chapter-15"
                                            title="Chapter 15">Chapter 15</a></span>
                                    <i>15 minute ago</i>
                                </li>
                            </ul>
                        </div>
                        
                        <div class="itemupdate first">
                            <a data-tooltip="sticky_53063" data-id="53063" class="tooltip cover bookmark_check"
                                href="https://www.mangakakalot.gg/manga/levelling-up-by-only-eating">
                                <img src="https://imgs-2.2xstorage.com/thumb/levelling-up-by-only-eating.webp"
                                    data-src="https://imgs-2.2xstorage.com/thumb/levelling-up-by-only-eating.webp"
                                    alt="Levelling Up, By Only Eating!" width="60" height="85" class="lazy">
                            </a>
                            <ul>
                                <li>
                                    <h3><a class="tooltip" data-tooltip="sticky_53063"
                                            href="https://www.mangakakalot.gg/manga/levelling-up-by-only-eating">Levelling
                                            Up, By Only Eating!</a></h3>
                                </li>
                                <li>
                                    <span><a class="sts sts_1"
                                            href="https://www.mangakakalot.gg/manga/levelling-up-by-only-eating/chapter-191"
                                            title="Chapter 191">Chapter 191</a></span>
                                    <i>23 minute ago</i>
                                </li>
                                <li>
                                    <span><a class="sts sts_1"
                                            href="https://www.mangakakalot.gg/manga/levelling-up-by-only-eating/chapter-190"
                                            title="Chapter 190">Chapter 190</a></span>
                                    <i>25 minute ago</i>
                                </li>
                                <li>
                                    <span><a class="sts sts_1"
                                            href="https://www.mangakakalot.gg/manga/levelling-up-by-only-eating/chapter-189"
                                            title="Chapter 189">Chapter 189</a></span>
                                    <i>26 minute ago</i>
                                </li>
                            </ul>
                        </div>



                    </div>

                "#;

        let new_added_mangas = NewAddedMangas::parse_html(HtmlElement::new(html.to_string()))?;

        let expected: NewMangaAddedItem = NewMangaAddedItem {
            page_url: "https://www.mangakakalot.gg/manga/my-level-up-is-strange-reincarnation-of-a-great-man-in-a-different-world"
                .to_string(),
            title: "My level up is strange! ~ Reincarnation of a great Man in a Different World [".to_string(),
            cover_img_url:
                "https://imgs-2.2xstorage.com/thumb/my-level-up-is-strange-reincarnation-of-a-great-man-in-a-different-world.webp"
                    .to_string(),
            latest_chapters: vec!["Chapter 17".to_string(), "Chapter 16".to_string(), "Chapter 15".to_string()],
        };

        let result = new_added_mangas.mangas.iter().find(|man| man.page_url == expected.page_url).unwrap();

        assert_eq!(expected, *result);

        assert_eq!(2, new_added_mangas.mangas.len());

        Ok(())
    }

    #[test]
    fn search_results_is_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = r#"
            <div class="daily-update">
                    <h3 class="title update-title">Keyword : oshi_no_ko</h3>
                    <div class="panel_story_list">
                        <div class="story_item" bis_skin_checked="1">
                            <a href="https://www.mangakakalot.gg/manga/oshi-no-ko" bis_skin_checked="1">
                                <img src="https://imgs-2.2xstorage.com/thumb/oshi-no-ko.webp" alt="Oshi No Ko
                                        class=" lazy" onerror="javascript:this.src='/images/404-avatar.webp';"
                                    width="60" height="85
                                    ">
                            </a>
                            <div class="story_item_right" bis_skin_checked="1">
                                <h3 class="story_name">
                                    <a href="https://www.mangakakalot.gg/manga/oshi-no-ko" bis_skin_checked="1">Oshi No Ko</a>

                                </h3>
                                <em class="story_chapter">
                                    <a href="https://www.mangakakalot.gg/manga/oshi-no-ko/chapter-167"
                                        title="Chapter 167">
                                        Chapter 167
                                    </a>
                                </em>
                                <em class="story_chapter">
                                    <a href="https://www.mangakakalot.gg/manga/oshi-no-ko/chapter-166-5"
                                        title="Chapter 166.5">
                                        Chapter 166.5
                                    </a>
                                </em>
                                <span>Author(s) : Akasaka Aka</span>
                                <span>Updated : Feb-18-2025 13:31</span>
                                <span>View : 47,000,000</span>
                            </div>
                        </div>
                        <div class="story_item" bis_skin_checked="1">
                            <a href="https://www.mangakakalot.gg/manga/oshi-no-ko-after-story" bis_skin_checked="1">
                                <img src="https://imgs-2.2xstorage.com/thumb/oshi-no-ko-after-story.webp" alt="【Oshi No Ko】After Story
                                        class=" lazy" onerror="javascript:this.src='/images/404-avatar.webp';"
                                    width="60" height="85
                                    ">
                            </a>

                            <div class="story_item_right" bis_skin_checked="1">
                                <h3 class="story_name">
                                    <a href="https://www.mangakakalot.gg/manga/oshi-no-ko-after-story"
                                        bis_skin_checked="1">【Oshi No Ko】After Story</a>

                                </h3>
                                <em class="story_chapter">
                                    <a href="https://www.mangakakalot.gg/manga/oshi-no-ko-after-story/chapter-1-5"
                                        title="Chapter 1.5">
                                        Chapter 1.5
                                    </a>
                                </em>
                                <em class="story_chapter">
                                    <a href="https://www.mangakakalot.gg/manga/oshi-no-ko-after-story/chapter-1"
                                        title="Chapter 1">
                                        Chapter 1
                                    </a>
                                </em>
                                <span>Author(s) : Cyrus Tmk</span>
                                <span>Updated : Jan-21-2025 13:52</span>
                                <span>View : 1,200</span>
                            </div>
                        </div>
                    </div>
                </div>
                <div style="clear: both"></div>
                <div class="panel_page_number">
                    <div class="group_page">
                        <a href="http://www.mangakakalot.gg/search/story/oshi_no_ko?page=1"
                            class="page_blue">First(1)</a>
                        <div class="page_select">1</div>
                        <a href="http://www.mangakakalot.gg/search/story/oshi_no_ko?page=1"
                            class="page_blue page_last">Last(1)</a>
                    </div>
                    <div class="group_qty">
                        <div class="page_blue">Total: 7 stories</div>
                    </div>
                </div>
        "#;

        let search_manga_response = SearchMangaResponse::parse_html(HtmlElement::new(html.to_string()))?;

        let expected1 = SearchMangaItem {
            page_url: "https://www.mangakakalot.gg/manga/oshi-no-ko".to_string(),
            title: "Oshi No Ko".to_string(),
            latest_chapters: vec!["Chapter 167".to_string(), "Chapter 166.5".to_string()],
            cover_url: "https://imgs-2.2xstorage.com/thumb/oshi-no-ko.webp".to_string(),
            author: Some(" Akasaka Aka".to_string()),
        };

        assert!(search_manga_response.mangas.len() >= 2);
        assert_eq!(7, search_manga_response.total_mangas);
        assert_eq!(expected1, search_manga_response.mangas[0]);

        let not_found_html = "<h1>not found</h1>".to_string();

        let response = SearchMangaResponse::parse_html(HtmlElement::new(not_found_html))?;

        assert!(response.mangas.is_empty());
        assert_eq!(response.total_mangas, 0);

        let not_found_html = "<h1>not found</h1>".to_string();

        let response = SearchMangaResponse::parse_html(HtmlElement::new(not_found_html))?;

        assert!(response.mangas.is_empty());
        assert_eq!(response.total_mangas, 0);

        Ok(())
    }

    #[test]
    fn manga_page_response_is_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = r#"
                <div class="manga-info-top">
                    <div class="manga-info-pic">
                        <img src="https://imgs-2.2xstorage.com/thumb/my-level-up-is-strange-reincarnation-of-a-great-man-in-a-different-world.webp"
                            onerror="javascript:this.src='/images/404-avatar.webp';" class="lazy"
                            alt="My level up is strange! ~ Reincarnation of a great Man in a Different World [" />
                        <span onclick="moveToListChapter();" class="btn_chapterslist">CHAPTER LIST</span>
                        <div class="read-chapter">
                            <a href="https://www.mangakakalot.gg/manga/my-level-up-is-strange-reincarnation-of-a-great-man-in-a-different-world/chapter-1"
                                rel="nofollow">Start Reading</a>
                            <a href="https://www.mangakakalot.gg/manga/my-level-up-is-strange-reincarnation-of-a-great-man-in-a-different-world/chapter-17"
                                rel="nofollow">Newest Chapter</a>
                        </div>
                    </div>
                    <ul class="manga-info-text">
                        <li>
                            <h1>My level up is strange! ~ Reincarnation of a great Man in a Different World [</h1>
                            <h2 class="story-alternative">Alternative : Ore no Level Up ga Okashi! ~ Dekiru Otoko no
                                Isekai Tensei ; 俺のレベルアップがおかしい！ ～デキる男の異世界転生～</h2>
                        </li>

                        <li>Author(s) :
                            <a href="https://www.mangakakalot.gg/author/unknown">Unknown</a>
                        </li>
                        <li>Status : Ongoing</li>
                        <li>Last updated : Feb-24-2025 05:35:02 PM</li>
                        <li style="display: none;">TransGroup : </li>
                        <li>View : 648</li>

                        <li class="genres">Genres :
                            <a href="https://www.mangakakalot.gg/genre/action">
                                Action
                            </a>,
                            <a href="https://www.mangakakalot.gg/genre/adventure">
                                Adventure
                            </a>,
                            <a href="https://www.mangakakalot.gg/genre/comedy">
                                Comedy
                            </a>,
                            <a href="https://www.mangakakalot.gg/genre/harem">
                                Harem
                            </a>,
                            <a href="https://www.mangakakalot.gg/genre/mature">
                                Mature
                            </a>
                        </li>
                        <li style="height: 27px;"><span>Rating : </span>
                            <em class="rate_row" id="rate_row"></em>
                            <em class="rate_row_result"></em>
                        </li>
                        <li style="line-height: 20px; font-size: 11px; font-style: italic; padding: 0px 0px 0px 44px;">
                            <span style="line-height: 100%!important;">&nbsp;</span>
                            <em id="rate_row_cmd" style="color: black;">
                                mangakakalot.gg rate : 5 / 5 - 11 votes
                            </em>
                            <script type="application/ld+json"> {
                                "@context": "https://schema.org",
                                "@type": "AggregateRating",
                                "ratingValue": "5",
                                "ratingCount": "11",
                                "itemReviewed": {
                                    "@type": "Book",
                                    "name": "My level up is strange! ~ Reincarnation of a great Man in a Different World [",
                                    "image": "https://imgs-2.2xstorage.com/thumb/my-level-up-is-strange-reincarnation-of-a-great-man-in-a-different-world.webp"
                                }
                            } </script>
                        </li>
                        <li class="li_bookmark"></li>
                        <li>
                            <div>
                                <p class="fb-save" data-uri="/manga/bf935595" data-size="small" id="savefilmfb"></p>
                            </div>
                            <div class="fb-like" data-href="/manga/bf935595" data-layout="standard" data-action="like"
                                data-show-faces="true" data-share="true">
                            </div>
                        </li>
                    </ul>
                </div>

                <div id="contentBox"
                    style="font: 400 14px Open Sans, Tahoma, Geneva, sans-serif; color: #3e3e3e; width: 96%; padding: 10px 2%; background: #FFF; text-align: justify; border-top: 1px dashed #ff530d; margin-bottom: 0px; float: left; overflow: hidden;">
                    <h2>
                        <p style="color: red;">My level up is strange! ~ Reincarnation of a great Man in a Different
                            World [ summary: </p>
                    </h2>
                    You are reading My level up is strange! ~ Reincarnation of a great Man in a Different World [ manga, one of the most popular manga covering in Action, Adventure, Comedy, Harem, Mature genres, written</div>
                    "#;

        let genres: Vec<ManganatoGenre> = ["Action", "Adventure", "Comedy", "Harem", "Mature"]
            .into_iter()
            .map(|gen| ManganatoGenre {
                name: gen.to_string(),
            })
            .collect();

        let expected: MangaPageData = MangaPageData {
            title: "My level up is strange! ~ Reincarnation of a great Man in a Different World [".to_string(),
            authors: Some("Unknown".to_string()),
            status: ManganatoStatus {
                name: "Ongoing".to_string(),
            },
            genres,
            rating: "5 out of 5".to_string(),
            description: r#"You are reading My level up is strange! ~ Reincarnation of a great Man in a Different World [ manga, one of the most popular manga covering in Action, Adventure, Comedy, Harem, Mature genres, written"#
            .to_string(),
            cover_url: "https://imgs-2.2xstorage.com/thumb/my-level-up-is-strange-reincarnation-of-a-great-man-in-a-different-world.webp".to_string(),
        };

        let result = MangaPageData::parse_html(HtmlElement::new(html.to_string()))?;

        assert_eq!(expected, result);
        Ok(())
    }

    #[test]
    fn chaptes_is_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = r#"
                    <div class="chapter-list">
                            <div class="row">
                                <span><a href="https://www.mangakakalot.gg/manga/my-level-up-is-strange-reincarnation-of-a-great-man-in-a-different-world/chapter-17"
                                        title="My level up is strange! ~ Reincarnation of a great Man in a Different World [ Chapter 17">Chapter 17</a></span>
                                <span> 766 </span>
                                <span title="Feb-24-2025 05:35">1 hour ago</span>
                            </div>
                            <div class="row">
                                <span><a href="https://www.mangakakalot.gg/manga/my-level-up-is-strange-reincarnation-of-a-great-man-in-a-different-world/chapter-16"
                                        title="My level up is strange! ~ Reincarnation of a great Man in a Different World [ Chapter 16">Chapter 16</a></span>
                                <span> 639 </span>
                                <span title="Feb-24-2025 05:34">1 hour ago</span>
                            </div>
                    </div>
        "#;

        let expected_chapter: ManganatoChapter = ManganatoChapter {
            page_url: "https://www.mangakakalot.gg/manga/my-level-up-is-strange-reincarnation-of-a-great-man-in-a-different-world/chapter-17".to_string(),
            title: None,
            number: "17".to_string(),
            volume: None,
            uploaded_at: "Feb-24-2025 05:35".to_string(),
        };

        let expected_chapter2: ManganatoChapter = ManganatoChapter {
            page_url: "https://www.mangakakalot.gg/manga/my-level-up-is-strange-reincarnation-of-a-great-man-in-a-different-world/chapter-16".to_string(),
            title: None,
            number: "16".to_string(),
            volume: None,
            uploaded_at: "Feb-24-2025 05:34".to_string(),
        };

        let result = ManganatoChaptersResponse::parse_html(HtmlElement::new(html))?;

        let chapter = result
            .chapters
            .iter()
            .find(|chap| chap.page_url == expected_chapter.page_url)
            .ok_or("Expected chapter was not parsed")?;

        assert_eq!(expected_chapter, *chapter);

        let chapter = result
            .chapters
            .iter()
            .find(|chap| chap.page_url == expected_chapter2.page_url)
            .ok_or("Expected chapter was not parsed")?;

        assert_eq!(expected_chapter2, *chapter);

        assert_eq!(2, result.total_chapters);

        let html = "<h1> the div containing the chapters is not there</h1>";

        let result = ManganatoChaptersResponse::parse_html(HtmlElement::new(html))?;

        assert_eq!(0, result.total_chapters);

        Ok(())
    }

    #[test]
    fn chapter_page_is_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = r#"
        <!-- for the list of chapters -->
        <div class="option_wrap">
            <select class="navi-change-chapter">
                <option data-c="/manga/sometimes-even-reality-is-a-lie/chapter-215">
                    Chapter 215
                </option>
                <option data-c="/manga/sometimes-even-reality-is-a-lie/chapter-214">
                    Chapter 214
                </option>
            </select>
        </div>

        <div class="info-top-chapter">
            <h2>Sometimes Even Reality Is a Lie! Chapter 212</h2>
            <p class="info-top-chapter-text">You're reading <strong>Sometimes Even Reality Is a Lie! Chapter 212</strong> at
                MangaKakalot.</p>
            <p class="info-top-chapter-text">
                Click the
                <strong>
                    <span id="btnBookmarkChapter" data-url="https://www.mangakakalot.gg/action/bookmark/49680"
                        data-action="add" style="cursor: pointer;">
                        🌟 Bookmark
                    </span>
                </strong>
                button now to stay updated on the latest chapters on MangaKakalot!💡Press F11 button to <a
                    href="https://www.mangakakalot.gg"> <strong>read manga</strong></a> in full-screen(PC-only).
                It will be so grateful if you let Mangakakalot be your favorite <strong>manga site</strong>. We hope you'll
                come join us and become a manga reader in this community! Have a beautiful day!
            </p>
            <div class="panel-option">
                <!-- <p style="color: #ca4848;">Image shows slow or error, you should choose another IMAGE SERVER.</p> -->
                <span class="pn-op-img-sv">
                    <span class="pn-op-name">IMAGES SERVER: </span>

                    <span class="pn-op-sv-img-btn a-h isactive" data-cdn="1">1</span>
                </span>

            </div>
        </div>
        

        <!-- for the pages of the chapter -->
        <div class="container-chapter-reader">
            <!-- spooky hidden input -->
            <input type="hidden" name="_token" value="DDSr9IfXkpMCwx0TMMKy5RPqbYGkGp5efbkr3VsC"> 

                <img
                src='https://storage.waitst.com/zin/sometimes-even-reality-is-a-lie/212/0.webp'
                alt='Sometimes Even Reality Is a Lie! Chapter 212 page 1 - MangaKakalot'
                title='Sometimes Even Reality Is a Lie! Chapter 212 page 1 - MangaKakalot'
                onerror="this.onerror=null;this.src='https://imgs-3.2xstorage.com/zin/sometimes-even-reality-is-a-lie/212/0.webp';"
                loading='lazy'>

                <img src='https://storage.waitst.com/zin/sometimes-even-reality-is-a-lie/212/1.webp'
                alt='Sometimes Even Reality Is a Lie! Chapter 212 page 2 - MangaKakalot'
                title='Sometimes Even Reality Is a Lie! Chapter 212 page 2 - MangaKakalot'
                onerror="this.onerror=null;this.src='https://imgs-3.2xstorage.com/zin/sometimes-even-reality-is-a-lie/212/1.webp';"
                loading='lazy'>

                <img src='https://storage.waitst.com/zin/sometimes-even-reality-is-a-lie/212/2.webp'
                alt='Sometimes Even Reality Is a Lie! Chapter 212 page 3 - MangaKakalot'
                title='Sometimes Even Reality Is a Lie! Chapter 212 page 3 - MangaKakalot'
                onerror="this.onerror=null;this.src='https://imgs-3.2xstorage.com/zin/sometimes-even-reality-is-a-lie/212/2.webp';"
                loading='lazy'>
        </div>
        "#;

        let result = ChapterPageResponse::parse_html(HtmlElement::new(html))?;

        assert!(
            result
                .pages_url
                .urls
                .iter()
                .find(|url| *url == "https://storage.waitst.com/zin/sometimes-even-reality-is-a-lie/212/2.webp")
                .is_some()
        );

        let expected_chapter_list_item = ChaptersListItem {
            page_url: "https://www.mangakakalot.gg/manga/sometimes-even-reality-is-a-lie/chapter-215".to_string(),
            title: None,
            number: "215".to_string(),
            volume_number: None,
        };

        assert_eq!(
            Some(&expected_chapter_list_item),
            result
                .chapters_list
                .chapters
                .iter()
                .find(|chap| chap.page_url == "https://www.mangakakalot.gg/manga/sometimes-even-reality-is-a-lie/chapter-215")
        );

        assert_eq!("212", result.number);

        Ok(())
    }

    #[test]
    fn chapter_list_is_parsed_from_html() -> Result<(), Box<dyn Error>> {
        //On the website the select which contains the chapter list appears two times so when
        //parsing the html there shouldnt be duplicates
        // see for reference : https://chapmanganato.to/manga-ve998587/chapter-17
        let html = r#"
        <select class="navi-change-chapter">
            <option data-c="/manga/sometimes-even-reality-is-a-lie/chapter-215">
                Chapter 215
            </option>
            <option data-c="/manga/sometimes-even-reality-is-a-lie/chapter-214">
                Chapter 214
            </option>
            <option data-c="/manga/sometimes-even-reality-is-a-lie/chapter-213">
                Chapter 213
            </option>
        </select>

        <select class="navi-change-chapter">
            <option data-c="/manga/sometimes-even-reality-is-a-lie/chapter-215">
                Chapter 215
            </option>
            <option data-c="/manga/sometimes-even-reality-is-a-lie/chapter-214">
                Chapter 214
            </option>
            <option data-c="/manga/sometimes-even-reality-is-a-lie/chapter-213">
                Chapter 213
            </option>
        </select>



        "#;

        let chapter_list = ChaptersList::parse_html(HtmlElement::new(html))?;

        assert_eq!(3, chapter_list.chapters.len());

        Ok(())
    }

    #[test]
    fn chapter_list_to_list_of_chapters_conversio() {
        let chapter_list = ChaptersList {
            chapters: vec![
                ChaptersListItem {
                    page_url: "some_url1".to_string(),
                    title: None,
                    number: "1".to_string(),
                    volume_number: None,
                },
                ChaptersListItem {
                    page_url: "some_url2".to_string(),
                    title: None,
                    number: "2".to_string(),
                    volume_number: Some("1".to_string()),
                },
                ChaptersListItem {
                    page_url: "some_url3".to_string(),
                    title: None,
                    number: "3".to_string(),
                    volume_number: Some("1".to_string()),
                },
                ChaptersListItem {
                    page_url: "some_url4".to_string(),
                    title: None,
                    number: "4".to_string(),
                    volume_number: Some("2".to_string()),
                },
            ],
        };

        let expected = ListOfChapters {
            volumes: SortedVolumes::new(vec![
                Volumes {
                    volume: "none".to_string(),
                    chapters: SortedChapters::new(vec![ChapterReader {
                        id: "some_url1".to_string(),
                        number: "1".to_string(),
                        volume: "none".to_string(),
                    }]),
                },
                Volumes {
                    volume: "1".to_string(),
                    chapters: SortedChapters::new(vec![
                        ChapterReader {
                            id: "some_url2".to_string(),
                            number: "2".to_string(),
                            volume: "1".to_string(),
                        },
                        ChapterReader {
                            id: "some_url3".to_string(),
                            number: "3".to_string(),
                            volume: "1".to_string(),
                        },
                    ]),
                },
                Volumes {
                    volume: "2".to_string(),
                    chapters: SortedChapters::new(vec![ChapterReader {
                        id: "some_url4".to_string(),
                        number: "4".to_string(),
                        volume: "2".to_string(),
                    }]),
                },
            ]),
        };

        assert_eq!(expected, ListOfChapters::from(chapter_list));
    }
}
