use std::collections::HashMap;
use std::error::Error;
use std::fmt::Display;
use std::num::ParseIntError;

use chrono::NaiveDate;
use regex::Regex;
use scraper::selectable::Selectable;
use scraper::{html, Selector};
use serde::{Deserialize, Serialize};

use super::ManganatoProvider;
use crate::backend::html_parser::scraper::AsSelector;
use crate::backend::html_parser::{HtmlElement, ParseHtml};
use crate::backend::manga_provider::{
    ChapterReader, Genres, GetMangasResponse, ListOfChapters, MangaStatus, PopularManga, Rating, SearchManga, SortedChapters,
    SortedVolumes, Volumes,
};

pub(super) fn extract_id_from_url<T: AsRef<str>>(url: T) -> String {
    let as_string: &str = url.as_ref();
    let as_string = as_string.split("/").last().unwrap_or_default();
    as_string.to_string()
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

#[derive(Debug, Default)]
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
        let a_selector = Selector::parse(".item div h3 a").unwrap();
        let a_selector_additional_info = Selector::parse(".item div a").unwrap();
        let img_selector = Selector::parse(".item img").unwrap();

        let a_tag = div.select(&a_selector).next().ok_or("Could not find div element containing manga info")?;
        let a_tag_additional_info = div
            .select(&a_selector_additional_info)
            .last()
            .ok_or("could not find a tag containing additional information")?;
        let title = a_tag.attr("title").ok_or("Could not find manga title")?;
        let manga_page_url = a_tag.attr("href").ok_or("Could not find manga page url")?;

        let additiona_info = a_tag_additional_info.inner_html();

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
        let selector = Selector::parse("#owl-slider > *").unwrap();

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
    pub(super) id: String,
    pub(super) id_tooltip: String,
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
        let a_selector = "a".as_selector();
        let a_tag = div.select(&a_selector).next().ok_or("no a tag found")?;
        let id_tool_tip = a_tag.attr("data-tooltip").ok_or("no tooltip attribute found")?;

        let id_tooltip = id_tool_tip.split("_").last().ok_or("no tooltip id found")?.to_string();

        let id = a_tag.attr("href").ok_or("no href attribute found on a tag")?;

        let id = id.to_string();

        Ok(Self { id_tooltip, id })
    }
}

impl ParseHtml for NewAddedMangas {
    type ParseError = NewAddedMangasError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let doc = html::Html::parse_document(html.as_str());

        let selector = ".panel-content-homepage > div".as_selector();

        let mut mangas: Vec<Result<NewMangaAddedItem, <NewMangaAddedItem as ParseHtml>::ParseError>> = vec![];

        for child in doc.select(&selector).take(5) {
            mangas.push(NewMangaAddedItem::parse_html(HtmlElement::new(child.html())));
        }

        Ok(NewAddedMangas {
            mangas: mangas.into_iter().flatten().collect(),
        })
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ToolTipItem {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) image: String,
    pub(super) description: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct SearchMangaItem {
    pub(super) id: String,
    pub(super) cover_url: String,
    pub(super) title: String,
    pub(super) rating: String,
    pub(super) latest_chapters: String,
    pub(super) description: Option<String>,
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
        let check_search_is_not_found = Selector::parse(".panel-content-genres").unwrap();

        if doc.select(&check_search_is_not_found).next().is_none() {
            return Ok(SearchMangaResponse {
                mangas: vec![],
                total_mangas: 0,
            });
        }

        // at this point search contains mangas

        let selector_div_containing_mangas = ".panel-content-genres > *".as_selector();

        let selector_total_mangas = ".panel-page-number > .group-qty > a".as_selector();

        let mut mangas: Vec<Result<SearchMangaItem, <SearchMangaItem as ParseHtml>::ParseError>> = vec![];

        for div in doc.select(&selector_div_containing_mangas) {
            mangas.push(SearchMangaItem::parse_html(HtmlElement::new(div.html())));
        }

        let maybe_total_mangas = doc.select(&selector_total_mangas).next();

        // if this tag is not present then there is only one page
        let total_mangas: u32 = if let Some(total) = maybe_total_mangas {
            let total_mangas: Result<u32, ParseIntError> = {
                let total_mangas = total.inner_html();

                let total_mangas = total_mangas.split(":").last().ok_or("no total")?;
                let total_mangas: String = total_mangas.split(",").collect();
                total_mangas.trim().parse()
            };

            total_mangas.map_err(|e| e.to_string())?
        } else {
            mangas.len() as u32
        };

        Ok(Self {
            mangas: mangas.into_iter().map(|may| may.unwrap()).collect(),
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

        let img_selector = Selector::parse("img").unwrap();

        let img = div.select(&img_selector).next().ok_or("no img")?;
        let cover_url = img.attr("src").ok_or("no cover")?;

        let rating_selector = Selector::parse(".genres-item-rate").unwrap();
        let rating = div.select(&rating_selector).next().ok_or("no rating tag found")?;

        let a_containing_manga_page_url = Selector::parse("a").unwrap();
        let a_manga_page_url = div.select(&a_containing_manga_page_url).next().ok_or("no a tag found")?;

        let title = a_manga_page_url.attr("title").ok_or("title not found")?;
        let manga_page_url = a_manga_page_url.attr("href").ok_or("no href")?;

        let latest_chapters_selector = Selector::parse(".genres-item-chap").unwrap();
        let latest_chapters: String = div.select(&latest_chapters_selector).map(|a| a.inner_html()).collect();

        let description_selector = Selector::parse(".genres-item-description").unwrap();
        let description = div.select(&description_selector).next().map(|desc| desc.inner_html().trim().to_string());

        Ok(Self {
            id: manga_page_url.to_string(),
            title: title.to_string(),
            cover_url: cover_url.to_string(),
            rating: rating.inner_html(),
            latest_chapters: latest_chapters.trim().to_string(),
            description,
        })
    }
}

impl From<SearchMangaItem> for SearchManga {
    fn from(value: SearchMangaItem) -> Self {
        Self {
            id: value.id,
            title: value.title,
            genres: vec![],
            description: value.description,
            status: None,
            cover_img_url: value.cover_url,
            languages: ManganatoProvider::MANGANATO_MANGA_LANGUAGE.into(),
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
    pub(super) authors: String,
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
        let right_div_selector = Selector::parse(".story-info-right").unwrap();

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

        let row_selector = Selector::parse("tr").unwrap();

        let authors_selector = Selector::parse(".table-label > .info-author").unwrap();
        let value_selector_a_tag = Selector::parse(".table-value > a").unwrap();

        let status_selector = Selector::parse(".table-label > .info-status").unwrap();

        let status_selector_value = Selector::parse(".table-value").unwrap();
        let genres_selector = Selector::parse(".table-label > .info-genres").unwrap();

        let mut authors = String::new();
        let mut status = ManganatoStatus {
            name: String::new(),
        };
        let mut genres: Vec<ManganatoGenre> = vec![];

        for rows in right_div_containing_most_info.select(&row_selector) {
            if rows.select(&authors_selector).next().is_some() {
                if let Some(tag) = rows.select(&value_selector_a_tag).next() {
                    authors = tag.inner_html().trim().to_string();
                }
            } else if rows.select(&status_selector).next().is_some() {
                status.name = rows
                    .select(&status_selector_value)
                    .next()
                    .ok_or("no status tag")?
                    .inner_html()
                    .trim()
                    .to_string();
            } else if rows.select(&genres_selector).next().is_some() {
                for genre in rows.select(&value_selector_a_tag) {
                    genres.push(ManganatoGenre {
                        name: genre.inner_html(),
                    });
                }
            }
        }

        let rating_selector = Selector::parse(r#"em[property="v:average"]"#).unwrap();
        let rating = right_div_containing_most_info
            .select(&rating_selector)
            .next()
            .ok_or("no rating tag")?
            .inner_html();

        let rating = format!("{rating} out of 5");

        let img_selector = Selector::parse(".story-info-left .img-loading").unwrap();

        let cover_url = div
            .select(&img_selector)
            .next()
            .ok_or("no img tag found")?
            .attr("src")
            .ok_or("no src attribute on img tag")?
            .to_string();

        let description_selector = Selector::parse("#panel-story-info-description").unwrap();
        let description_div = div.select(&description_selector).next().ok_or("no description tag found")?;
        let mut description = String::new();

        // This is how we can obtain the inner text of a element without whithout tags
        for node in html::Html::parse_fragment(&description_div.inner_html()).tree {
            if let scraper::node::Node::Text(text) = node {
                description = text.to_string();
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

/// On the website the title of the chapter looks like this:
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
        let li = html::Html::parse_fragment(html.as_str());
        let a_selector = "a".as_selector();

        let a_tag = li.select(&a_selector).next().ok_or("no a tag found")?;

        let id = a_tag.attr("href").ok_or("no href on tag a")?;

        let raw_title = a_tag.inner_html().trim().to_string();

        let title_parts = extract_chapter_title(&raw_title);

        let uploaded_at_selector = ".chapter-time".as_selector();
        let uploaded_at = li
            .select(&uploaded_at_selector)
            .next()
            .ok_or("no span with uploaded at selector")?
            .attr("data-fn-time")
            .ok_or("could not get timestamp")?
            .to_string();

        Ok(Self {
            page_url: id.to_string(),
            title: title_parts.title.map(|title| title.to_string()),
            number: title_parts.number.to_string(),
            volume: title_parts.volume_number.map(|vol| vol.to_string()),
            uploaded_at,
        })
    }
}

impl ParseHtml for ManganatoChaptersResponse {
    type ParseError = ChapterParseError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let div = html::Html::parse_fragment(html.as_str());
        let chapters_selector = Selector::parse(".row-content-chapter > li").unwrap();

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

        let base_url_selector = ".panel-breadcrumb".as_selector();

        let div_with_base_url = doc.select(&base_url_selector).next().ok_or("no title tag was found")?;

        let base_page_url = div_with_base_url
            .select(&":last-child".as_selector())
            .next()
            .ok_or("no a tag found")?
            .attr("href")
            .ok_or("no href was found")?;

        let remove_chapter_id: String = base_page_url.split("/").last().unwrap().to_string();
        let base_page_url = base_page_url.replace(&remove_chapter_id, "");

        let chapters_list_selector = ".navi-change-chapter".as_selector();

        let select_containing_chapters = doc
            .select(&chapters_list_selector)
            .next()
            .ok_or("the select containing the chapter list was not found")?;

        let mut chapters: Vec<ChaptersListItem> = vec![];
        for item in select_containing_chapters.select(&"option".as_selector()) {
            let inner_html = item.inner_html();
            let title_parts = extract_chapter_title(&inner_html);

            chapters.push(ChaptersListItem {
                page_url: format!("{base_page_url}chapter-{}", title_parts.number),
                number: title_parts.number.to_string(),
                title: title_parts.title.map(|title| title.to_string()),
                volume_number: title_parts.volume_number.map(|num| num.to_string()),
            });
        }

        Ok(ChaptersList { chapters })
    }
}

impl ParseHtml for ChapterPageResponse {
    type ParseError = ChapterPageError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let doc = html::Html::parse_document(html.as_str());

        let title_selector_div = ".panel-breadcrumb".as_selector();

        let div_with_title = doc.select(&title_selector_div).next().ok_or("no title tag was found")?;

        let a_containing_title_and_url = div_with_title
            .select(&":last-child".as_selector())
            .next()
            .ok_or("no a tag found")?
            .inner_html();

        let title_parts = extract_chapter_title(&a_containing_title_and_url);

        Ok(Self {
            title: title_parts.title.map(|title| title.to_string()),
            number: title_parts.number.to_string(),
            volume_number: title_parts.volume_number.map(|vol| vol.to_string()),
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
    fn popular_manga_item_parses_from_html() -> Result<(), Box<dyn Error>> {
        let html = r#"
                <div class="item"> 
                <img class="img-loading"
                        src="https://avt.mkklcdnv6temp.com/fld/90/v/14-1733306029-nw.webp"
                        onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                        alt="Yuusha Party O Oida Sareta Kiyou Binbou" />

                    <div class="slide-caption">
                        <h3>
                            <a class="text-nowrap a-h" href="https://manganato.com/manga-sn995770"
                                title="Yuusha Party O Oida Sareta Kiyou Binbou">Yuusha Party O Oida Sareta Kiyou
                                Binbou</a>
                        </h3>
                        <a rel="nofollow" class="text-nowrap a-h"
                            href="https://chapmanganato.to/manga-sn995770/chapter-30.1"
                            title="Yuusha Party O Oida Sareta Kiyou Binbou Chapter 30.1">Chapter 30.1</a>
                    </div>
                </div>
        "#;

        let popular_manga = PopularMangaItem::parse_html(HtmlElement::new(html.to_string()))?;

        assert_eq!(popular_manga.id, "https://manganato.com/manga-sn995770");
        assert_eq!(popular_manga.title, "Yuusha Party O Oida Sareta Kiyou Binbou");
        assert_eq!(popular_manga.cover_img_url, "https://avt.mkklcdnv6temp.com/fld/90/v/14-1733306029-nw.webp");
        assert_eq!(popular_manga.additional_data, "Latest chapter: Chapter 30.1");

        Ok(())
    }

    #[test]
    fn popular_manga_response_gets_inner_items() -> Result<(), Box<dyn Error>> {
        let html = r#"
                <div id="owl-slider" class="owl-carousel">
                    <div class="item"> <img class="img-loading"
                            src="https://avt.mkklcdnv6temp.com/fld/87/w/13-1733298325-nw.webp"
                            onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                            alt="Inside the Cave of Obscenity" />
                        <div class="slide-caption">
                            <h3><a class="text-nowrap a-h" href="https://manganato.com/manga-pk993067"
                                    title="Inside the Cave of Obscenity">Inside The Cave Of Obscenity</a></h3><a
                                rel="nofollow" class="text-nowrap a-h"
                                href="https://chapmanganato.to/manga-pk993067/chapter-21"
                                title="Inside the Cave of Obscenity Chapter 21">Chapter 21</a>
                        </div>
                    </div>
                    <div class="item"> <img class="img-loading"
                            src="https://avt.mkklcdnv6temp.com/fld/24/n/15-1733308050-nw.webp"
                            onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                            alt="Creepy Pharmacist: All My Patients are Horrific" />
                        <div class="slide-caption">
                            <h3><a class="text-nowrap a-h" href="https://manganato.com/manga-tp996650"
                                    title="Creepy Pharmacist: All My Patients are Horrific">Creepy Pharmacist: All My
                                    Patients Are Horrific</a></h3><a rel="nofollow" class="text-nowrap a-h"
                                href="https://chapmanganato.to/manga-tp996650/chapter-109"
                                title="Creepy Pharmacist: All My Patients are Horrific Chapter 109">Chapter 109</a>
                        </div>
                    </div>
                    <div class="item"> <img class="img-loading"
                            src="https://avt.mkklcdnv6temp.com/fld/90/v/14-1733306029-nw.webp"
                            onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                            alt="Yuusha Party O Oida Sareta Kiyou Binbou" />
                        <div class="slide-caption">
                            <h3><a class="text-nowrap a-h" href="https://manganato.com/manga-sn995770"
                                    title="Yuusha Party O Oida Sareta Kiyou Binbou">Yuusha Party O Oida Sareta Kiyou
                                    Binbou</a></h3><a rel="nofollow" class="text-nowrap a-h"
                                href="https://chapmanganato.to/manga-sn995770/chapter-30.1"
                                title="Yuusha Party O Oida Sareta Kiyou Binbou Chapter 30.1">Chapter 30.1</a>
                        </div>
                    </div>
                </div>
        "#;

        let response = GetPopularMangasResponse::parse_html(HtmlElement::new(html.to_string()))?;

        assert_eq!(response.mangas[0].title, "Inside the Cave of Obscenity");
        assert_eq!(response.mangas[1].title, "Creepy Pharmacist: All My Patients are Horrific");
        assert_eq!(response.mangas[2].title, "Yuusha Party O Oida Sareta Kiyou Binbou");

        Ok(())
    }

    #[test]
    fn latest_manga_update_parses_from_html() -> Result<(), Box<dyn Error>> {
        let html = r#"
                <div class="panel-content-homepage">
                    <h1 class="content-homepage-title">READ MANGA ONLINE - LATEST UPDATES</h1>
                    <div class="content-homepage-item">
                        <a rel="nofollow" data-tooltip="sticky_54452" class=" tooltip item-img bookmark_check "
                            data-id="NTQ0NTI=" href="https://chapmanganato.to/manga-ca1005809"
                            title="Until I Break My Husband’s Family">
                            <img class="img-loading" src="https://avt.mkklcdnv6temp.com/fld/75/r/18-1733381622-nw.webp"
                                onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                                alt="Until I Break My Husband’s Family" />
                            <em class="item-rate">4.2</em> </a>
                        <div class="content-homepage-item-right">
                            <h3 class="item-title">
                                <a rel="nofollow" data-tooltip="sticky_54452" class="tooltip a-h text-nowrap"
                                    href="https://chapmanganato.to/manga-ca1005809"
                                    title="Until I Break My Husband’s Family">Until I Break My Husband’s Family</a>
                            </h3>
                            <span class="text-nowrap item-author" title="Loreen Author">Loreen</span>
                            <p class="a-h item-chapter">
                                <a rel="nofollow" class="text-nowrap"
                                    href="https://chapmanganato.to/manga-ca1005809/chapter-12"
                                    title="Until I Break My Husband’s Family Chapter 12">Chapter 12</a>
                                <i class="fn-cover-item-time" data-fn-time="1739265335">Feb 11,2025</i>
                            </p>
                            <p class="a-h item-chapter">
                                <a rel="nofollow" class="text-nowrap"
                                    href="https://chapmanganato.to/manga-ca1005809/chapter-11"
                                    title="Until I Break My Husband’s Family Chapter 11">Chapter 11</a>
                                <i class="fn-cover-item-time" data-fn-time="1738596877">Feb 03,2025</i>
                            </p>
                            <p class="a-h item-chapter">
                                <a rel="nofollow" class="text-nowrap"
                                    href="https://chapmanganato.to/manga-ca1005809/chapter-10"
                                    title="Until I Break My Husband’s Family Chapter 10">Chapter 10</a>
                                <i class="fn-cover-item-time" data-fn-time="1738585778">Feb 03,2025</i>
                            </p>
                        </div>
                    </div>
                    <div class="content-homepage-item">
                        <a rel="nofollow" data-tooltip="sticky_53713" class=" tooltip item-img bookmark_check "
                            data-id="NTM3MTM=" href="https://chapmanganato.to/manga-bn1005070"
                            title="Do as You Please (_chut_)">
                            <img class="img-loading" src="https://avt.mkklcdnv6temp.com/fld/46/j/18-1733329988-nw.webp"
                                onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                                alt="Do as You Please (_chut_)" />
                            <em class="item-rate">3.9</em> </a>
                        <div class="content-homepage-item-right">
                            <h3 class="item-title">
                                <a rel="nofollow" data-tooltip="sticky_53713" class="tooltip a-h text-nowrap"
                                    href="https://chapmanganato.to/manga-bn1005070" title="Do as You Please (_chut_)">Do
                                    As You Please (_Chut_)</a>
                            </h3>
                            <span class="text-nowrap item-author" title="_chut_ , 出途 Author">_chut_ , 出途</span>
                            <p class="a-h item-chapter">
                                <a rel="nofollow" class="text-nowrap"
                                    href="https://chapmanganato.to/manga-bn1005070/chapter-9"
                                    title="Do as You Please (_chut_) Chapter 9">Chapter 9</a>
                                <i class="fn-cover-item-time" data-fn-time="1739265302">Feb 11,2025</i>
                            </p>
                            <p class="a-h item-chapter">
                                <a rel="nofollow" class="text-nowrap"
                                    href="https://chapmanganato.to/manga-bn1005070/chapter-8"
                                    title="Do as You Please (_chut_) Chapter 8">Chapter 8</a>
                                <i class="fn-cover-item-time" data-fn-time="1738069283">Jan 28,2025</i>
                            </p>
                            <p class="a-h item-chapter">
                                <a rel="nofollow" class="text-nowrap"
                                    href="https://chapmanganato.to/manga-bn1005070/chapter-7"
                                    title="Do as You Please (_chut_) Chapter 7">Chapter 7</a>
                                <i class="fn-cover-item-time" data-fn-time="1736145038">Jan 06,2025</i>
                            </p>
                        </div>
                    </div>
                    <div class="content-homepage-item">
                        <a rel="nofollow" data-tooltip="sticky_50081" class=" tooltip item-img bookmark_check "
                            data-id="NTAwODE=" href="https://chapmanganato.to/manga-yd1001438"
                            title="Legal Pirate Parfait">
                            <img class="img-loading" src="https://avt.mkklcdnv6temp.com/fld/7/j/17-1733321452-nw.webp"
                                onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                                alt="Legal Pirate Parfait" />
                            <em class="item-rate">4</em> </a>
                        <div class="content-homepage-item-right">
                            <h3 class="item-title">
                                <a rel="nofollow" data-tooltip="sticky_50081" class="tooltip a-h text-nowrap"
                                    href="https://chapmanganato.to/manga-yd1001438" title="Legal Pirate Parfait">Legal
                                    Pirate Parfait</a>
                            </h3>
                            <span class="text-nowrap item-author" title="BonesBloodFlesh (뼈피살) Author">BonesBloodFlesh
                                (뼈피살)</span>
                            <p class="a-h item-chapter">
                                <a rel="nofollow" class="text-nowrap"
                                    href="https://chapmanganato.to/manga-yd1001438/chapter-63"
                                    title="Legal Pirate Parfait Chapter 63: I Shall Enact Destruction">Chapter 63: I
                                    Shall Enact Destruction</a>
                                <i class="fn-cover-item-time" data-fn-time="1739265294">Feb 11,2025</i>
                            </p>
                            <p class="a-h item-chapter">
                                <a rel="nofollow" class="text-nowrap"
                                    href="https://chapmanganato.to/manga-yd1001438/chapter-62"
                                    title="Legal Pirate Parfait Chapter 62: Encounter In The Grass">Chapter 62:
                                    Encounter In The Grass</a>
                                <i class="fn-cover-item-time" data-fn-time="1738670853">Feb 04,2025</i>
                            </p>
                            <p class="a-h item-chapter">
                                <a rel="nofollow" class="text-nowrap"
                                    href="https://chapmanganato.to/manga-yd1001438/chapter-61"
                                    title="Legal Pirate Parfait Chapter 61: A Jewel That Grants Wishes">Chapter 61: A
                                    Jewel That Grants Wishes</a>
                                <i class="fn-cover-item-time" data-fn-time="1738058829">Jan 28,2025</i>
                            </p>
                        </div>
                    </div>
                    <a href="https://manganato.com/genre-all" class="content-homepage-more a-h">
                        << MORE>>
                    </a>
                </div>

                "#;

        let new_added_mangas = NewAddedMangas::parse_html(HtmlElement::new(html.to_string()))?;

        let expected = NewMangaAddedItem {
            id_tooltip: "54452".to_string(),
            id: "https://chapmanganato.to/manga-ca1005809".to_string(),
        };

        assert_eq!(
            expected,
            *new_added_mangas
                .mangas
                .iter()
                .find(|man| man.id == expected.id)
                .ok_or("expected manga was not parsed from html")?
        );
        assert_eq!(new_added_mangas.mangas.len(), 3);

        Ok(())
    }

    #[test]
    fn search_results_is_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = r#"
            <div class="panel-content-genres">
                <div class="content-genres-item">
                    <a rel="nofollow" class="genres-item-img bookmark_check" data-id="MzM2MTU="
                        href="https://chapmanganato.to/manga-hp984972" title="National School Prince Is A Girl">
                        <img class="img-loading" src="https://avt.mkklcdnv6temp.com/fld/87/v/10-1732803283-nw.webp"
                            onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                            alt="National School Prince Is A Girl" />
                        <em class="genres-item-rate">4.7</em> </a>
                    <div class="genres-item-info">
                        <h3><a rel="nofollow" class="genres-item-name text-nowrap a-h"
                                href="https://chapmanganato.to/manga-hp984972"
                                title="National School Prince Is A Girl">National School Prince Is A Girl</a></h3>
                        <a rel="nofollow" class="genres-item-chap text-nowrap a-h"
                            href="https://chapmanganato.to/manga-hp984972/chapter-504"
                            title="National School Prince Is A Girl Chapter 504">Chapter 504</a>

                        <p class="genres-item-view-time text-nowrap">
                            <span class="genres-item-view">21.4M</span>
                            <span class="genres-item-time">Feb 06,25</span>
                            <span class="genres-item-author">Warring Young Seven,战七少 战七少</span>
                        </p>
                        <div class="genres-item-description">
                            Fu Jiu appears to be a normal lad in high school on the surface.
                        </div>
                        <a rel="nofollow" class="genres-item-readmore"
                            href="https://chapmanganato.to/manga-hp984972">Read more</a>
                    </div>
                </div>
                <div class="content-genres-item">
                    <a rel="nofollow" class="genres-item-img bookmark_check" data-id="NTI5NzE="
                        href="https://chapmanganato.to/manga-at1004328"
                        title="26 Sai Shojo, Charao Joushi ni Dakaremashita">
                        <img class="img-loading" src="https://avt.mkklcdnv6temp.com/fld/18/b/18-1733328343-nw.webp"
                            onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                            alt="26 Sai Shojo, Charao Joushi ni Dakaremashita" />
                        <em class="genres-item-rate">4.4</em> </a>
                    <div class="genres-item-info">
                        <h3><a rel="nofollow" class="genres-item-name text-nowrap a-h"
                                href="https://chapmanganato.to/manga-at1004328"
                                title="26 Sai Shojo, Charao Joushi ni Dakaremashita">26 Sai Shojo, Charao Joushi Ni
                                Dakaremashita</a></h3>
                        <a rel="nofollow" class="genres-item-chap text-nowrap a-h"
                            href="https://chapmanganato.to/manga-at1004328/chapter-15"
                            title="26 Sai Shojo, Charao Joushi ni Dakaremashita Chapter 15">Chapter 15</a>

                        <p class="genres-item-view-time text-nowrap">
                            <span class="genres-item-view">96.9K</span>
                            <span class="genres-item-time">Feb 06,25</span>
                            <span class="genres-item-author">Ryo Nakaharu , NAKAHARU Ryou</span>
                        </p>
                        <div class="genres-item-description">
                            He's going at her so hard. She knows it's wrong, but it feels so good, she can't stop!
                            Chikage Ayashiro (26) has been thrown into a project with her boss, Toru Aogiri, who's a
                            playboy and hard to grasp. After a kick-off party, she ends up alone with him for some
                            reason! But, once the two talk, he seems more serious and... Drunk, they wind up at a hotel
                            where Toru turns out to be kind... As her bod <i class="genres-item-description-linear"></i>
                        </div>
                        <a rel="nofollow" class="genres-item-readmore"
                            href="https://chapmanganato.to/manga-at1004328">Read more</a>
                    </div>
                </div>
                <div class="content-genres-item">
                    <a rel="nofollow" class="genres-item-img bookmark_check" data-id="NDkwOTM="
                        href="https://chapmanganato.to/manga-xp1000450" title="Vanilla land">
                        <img class="img-loading" src="https://avt.mkklcdnv6temp.com/fld/69/o/16-1733318212-nw.webp"
                            onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                            alt="Vanilla land" />
                        <em class="genres-item-rate">3.5</em> </a>
                    <div class="genres-item-info">
                        <h3><a rel="nofollow" class="genres-item-name text-nowrap a-h"
                                href="https://chapmanganato.to/manga-xp1000450" title="Vanilla land">Vanilla Land</a>
                        </h3>
                        <a rel="nofollow" class="genres-item-chap text-nowrap a-h"
                            href="https://chapmanganato.to/manga-xp1000450/chapter-69.5"
                            title="Vanilla land Chapter 69.5: The Plan">Chapter 69.5: The Plan</a>

                        <p class="genres-item-view-time text-nowrap">
                            <span class="genres-item-view">46.6K</span>
                            <span class="genres-item-time">Feb 06,25</span>
                            <span class="genres-item-author">Couple Comics</span>
                        </p>
                        <div class="genres-item-description">
                            In lands far..far away, where scary monsters and horrific creatures live. where hidden
                            treasures and magic exists, where there are still undiscovered kingdoms and empires mankind
                            discovered a new source of energy to empower their mechanical machines & weapons. we
                            discovered vanilla the source of unlimited energy. follow the journey of gen orchid, rosa &
                            altamiras as they seek the djinn chef master <i class="genres-item-description-linear"></i>
                        </div>
                        <a rel="nofollow" class="genres-item-readmore"
                            href="https://chapmanganato.to/manga-xp1000450">Read more</a>
                    </div>
                </div>
            </div>

                <div class="panel-page-number">
                    <div class="group-page"><a href="https://manganato.com/search/story/death?page=1"
                            class="page-blue">FIRST(1)</a><a class="page-blue">1</a><a
                            href="https://manganato.com/search/story/death?page=2">2</a><a
                            href="https://manganato.com/search/story/death?page=3">3</a><a
                            href="https://manganato.com/search/story/death?page=12"
                            class="page-blue page-last">LAST(12)</a></div>
                    <div class="group-qty"><a class="page-blue">TOTAL : 3</a></div>
                </div>
        "#;

        let search_manga_response = SearchMangaResponse::parse_html(HtmlElement::new(html.to_string()))?;

        let expected1 = SearchMangaItem {
            id: "https://chapmanganato.to/manga-hp984972".to_string(),
            title: "National School Prince Is A Girl".to_string(),
            latest_chapters: "Chapter 504".to_string(),
            rating: "4.7".to_string(),
            cover_url: "https://avt.mkklcdnv6temp.com/fld/87/v/10-1732803283-nw.webp".to_string(),
            description: Some("Fu Jiu appears to be a normal lad in high school on the surface.".to_string()),
        };

        assert!(search_manga_response.mangas.len() > 2);
        assert_eq!(search_manga_response.total_mangas, 3);
        assert_eq!(expected1, search_manga_response.mangas[0]);

        let not_found_html = "<h1>not found</h1>".to_string();

        let response = SearchMangaResponse::parse_html(HtmlElement::new(not_found_html))?;

        assert!(response.mangas.is_empty());
        assert_eq!(response.total_mangas, 0);

        let not_found_html = "<h1>not found</h1>".to_string();

        let response = SearchMangaResponse::parse_html(HtmlElement::new(not_found_html))?;

        assert!(response.mangas.is_empty());
        assert_eq!(response.total_mangas, 0);

        // When there is no more total mangas than what is found
        let html = r#"
              <div class="panel-content-genres">
                <div class="content-genres-item">
                    <a rel="nofollow" class="genres-item-img bookmark_check" data-id="MzM2MTU="
                        href="https://chapmanganato.to/manga-hp984972" title="National School Prince Is A Girl">
                        <img class="img-loading" src="https://avt.mkklcdnv6temp.com/fld/87/v/10-1732803283-nw.webp"
                            onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';"
                            alt="National School Prince Is A Girl" />
                        <em class="genres-item-rate">4.7</em> </a>
                    <div class="genres-item-info">
                        <h3><a rel="nofollow" class="genres-item-name text-nowrap a-h"
                                href="https://chapmanganato.to/manga-hp984972"
                                title="National School Prince Is A Girl">National School Prince Is A Girl</a></h3>
                        <a rel="nofollow" class="genres-item-chap text-nowrap a-h"
                            href="https://chapmanganato.to/manga-hp984972/chapter-504"
                            title="National School Prince Is A Girl Chapter 504">Chapter 504</a>

                        <p class="genres-item-view-time text-nowrap">
                            <span class="genres-item-view">21.4M</span>
                            <span class="genres-item-time">Feb 06,25</span>
                            <span class="genres-item-author">Warring Young Seven,战七少 战七少</span>
                        </p>
                        <div class="genres-item-description">
                            Fu Jiu appears to be a normal lad in high school on the surface.
                        </div>
                        <a rel="nofollow" class="genres-item-readmore"
                            href="https://chapmanganato.to/manga-hp984972">Read more</a>
                    </div>
                </div>
            </div>
        "#;

        let response = SearchMangaResponse::parse_html(HtmlElement::new(html.to_string()))?;

        assert_eq!(response.total_mangas, 1);
        Ok(())
    }

    #[test]
    fn manga_page_response_is_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = r#"

                            <!-- this img tag may seem random but there are multiple imgs with this classname-->
                            <img class="img-loading" src="https://notthisone.webp"
                                alt="Dungeon ni Hisomu Yandere na Kanojo ni Ore wa Nando mo Korosareru"
                                title="Dungeon ni Hisomu Yandere na Kanojo ni Ore wa Nando mo Korosareru"
                                onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';" />

                <div class="panel-story-info">
                    <div class="story-info-left">
                        <span class="info-image">
                            <img class="img-loading" src="https://avt.mkklcdnv6temp.com/fld/43/y/15-1733309270-nw.webp"
                                alt="Dungeon ni Hisomu Yandere na Kanojo ni Ore wa Nando mo Korosareru"
                                title="Dungeon ni Hisomu Yandere na Kanojo ni Ore wa Nando mo Korosareru"
                                onerror="javascript:this.src='https://manganato.com/themes/hm/images/404_not_found.png';" />
                            <em class="item-hot"></em> </span>

                        <span class="style-btn btn-chapterlist">READ CHAPTER LIST</span>
                    </div>

                    <div class="story-info-right">
                        <h1>Dungeon Ni Hisomu Yandere Na Kanojo Ni Ore Wa Nando Mo Korosareru</h1>

                        <table class="variations-tableInfo">
                            <tbody>
                                <tr>
                                    <td class="table-label"><i class="info-alternative"></i>Alternative :</td>
                                    <td class="table-value">
                                        <h2>ダンジョンに潜むヤンデレな彼女に俺は何度も殺される ; My Yandere Girlfriend Lurks in the Dungeon and
                                            Kills Me Over and Over Again</h2>
                                    </td>
                                </tr>
                                <tr>
                                    <td class="table-label"><i class="info-author"></i>Author(s) :</td>
                                    <td class="table-value">
                                        <a rel="nofollow" class='a-h'
                                            href='https://manganato.com/author/story/a2l0YWdhd2FfbmlraXRh'>Kitagawa Nikita</a> - <a rel="nofollow" class='a-h'
                                            href='https://manganato.com/author/story/fHxub211cmFfZWpp'>Nomura Eji</a>
                                    </td>
                                </tr>
                                <tr>
                                    <td class="table-label"><i class="info-status"></i>Status :</td>
                                    <td class="table-value">Ongoing</td>
                                </tr>
                                <tr>
                                    <td class="table-label"><i class="info-genres"></i>Genres :</td>
                                    <td class="table-value">
                                        <a class='a-h' href='https://manganato.com/genre-2'>Action</a> - <a class='a-h'
                                            href='https://manganato.com/genre-4'>Adventure</a> - <a class='a-h'
                                            href='https://manganato.com/genre-10'>Drama</a> - <a class='a-h'
                                            href='https://manganato.com/genre-12'>Fantasy</a> - <a class='a-h'
                                            href='https://manganato.com/genre-14'>Harem</a> - <a class='a-h'
                                            href='https://manganato.com/genre-27'>Romance</a> - <a class='a-h'
                                            href='https://manganato.com/genre-30'>Seinen</a> - <a class='a-h'
                                            href='https://manganato.com/genre-38'>Supernatural</a> 
                                    </td>
                                </tr>
                            </tbody>
                        </table>

                        <div class="story-info-right-extent">
                            <p><span class="stre-label"><i class="info-time"></i>Updated :</span><span
                                    class="stre-value">Jan 31,2025 - 23:03 PM</span></p>
                            <p><span class="stre-label"><i class="info-view"></i>View :</span><span
                                    class="stre-value">4.5M</span></p>
                            <p>
                                <span class="stre-label"><i class="info-rate"></i>Rating :</span>
                                <span class="stre-value">
                                    <em class="rate_row" id="rate_row"></em>
                                    <em class="rate_row_result"></em>
                                </span>
                            </p>
                            <p>
                                <em id="rate_row_cmd">
                                    <em xmlns:v="http://rdf.data-vocabulary.org/#" typeof="v:Review-aggregate">
                                        <em property="v:itemreviewed">MangaNato.com</em>
                                        <em rel="v:rating">
                                            <em typeof="v:Rating">rate :
                                                <em property="v:average">4.49</em>/
                                                <em property="v:best">5</em>
                                            </em>
                                        </em> - <em property="v:votes">3281</em> votes
                                    </em>
                                </em>
                            </p>
                            <p class="user_btn_follow_i info-bookmark"></p>
                            <p class="noti-bookmark"></p>

                            <p class="fb-save" data-uri="https://manganato.com/manga-tc997159" data-size="small"
                                id="savefilmfb"></p>
                            <div class="fb-like" data-href="https://manganato.com/manga-tc997159"
                                data-layout="button_count" data-action="like" data-show-faces="false" data-share="true">
                            </div>

                        </div>
                    </div>

                    <div class="panel-story-info-description" id="panel-story-info-description"> <h3>Description :</h3> "Dungeon ni Hisomu Yandere na Kanojo ni Ore wa Nando mo Korosareru" is a fantasy story about Kiska, a boy who is mistreated by the villagers. He manages to survive thanks to his childhood friend Namia, who was the only one by his side. One day, Namia is murdered by three men</div>
                    <div id="panel-description-linear">
                        <i id="pn-description-linear-gradient" style="display: none;"></i>
                    </div>
                    <span style="display: none;" id="panel-story-info-description-show-more" class="a-h">SHOW MORE
                        ⇩</span>
                    <span style="display: initial;" id="panel-story-info-description-show-less" class="a-h">SHOW LESS
                        ⇧</span>
                </div>
        "#;

        let genres: Vec<ManganatoGenre> = ["Action", "Adventure", "Drama", "Fantasy", "Harem", "Romance", "Seinen", "Supernatural"]
            .into_iter()
            .map(|gen| ManganatoGenre {
                name: gen.to_string(),
            })
            .collect();

        let expected: MangaPageData = MangaPageData {
            title: "Dungeon Ni Hisomu Yandere Na Kanojo Ni Ore Wa Nando Mo Korosareru".to_string(),
            authors: "Kitagawa Nikita".to_string(),
            status: ManganatoStatus{name : "Ongoing".to_string()},
            genres,
            rating: "4.49 out of 5".to_string(),
            description: r#" "Dungeon ni Hisomu Yandere na Kanojo ni Ore wa Nando mo Korosareru" is a fantasy story about Kiska, a boy who is mistreated by the villagers. He manages to survive thanks to his childhood friend Namia, who was the only one by his side. One day, Namia is murdered by three men"#
            .to_string(),
            cover_url: "https://avt.mkklcdnv6temp.com/fld/43/y/15-1733309270-nw.webp".to_string(),
        };

        let result = MangaPageData::parse_html(HtmlElement::new(html.to_string()))?;

        assert_eq!(expected, result);
        Ok(())
    }

    #[test]
    fn chaptes_is_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = r#"
                <div class="panel-story-chapter-list">
                    <p class="row-title-chapter">
                        <span class="row-title-chapter-name">Chapter name</span>
                        <span class="row-title-chapter-view">View</span>
                        <span class="row-title-chapter-time">Uploaded</span>
                    </p>
                    <ul class="row-content-chapter">
                        <li class="a-h">
                            <a rel="nofollow" class="chapter-name text-nowrap"
                                href="https://chapmanganato.to/manga-jv986530/chapter-3"
                                title="My Death Flags Show No Sign of Ending chapter Chapter 3: Self-Performed Rescue Act"
                                title="My Death Flags Show No Sign of Ending Chapter 3: Self-Performed Rescue Act">Chapter
                                3: Self-Performed Rescue Act</a>
                            <span class="chapter-view text-nowrap">248</span>
                            <span class="chapter-time text-nowrap fn-cover-item-time" data-fn-time="1599562556"
                                title="Sep 08,2020 10:09">Sep 08,2020</span>
                        </li>
                        <li class="a-h">
                            <a rel="nofollow" class="chapter-name text-nowrap"
                                href="https://chapmanganato.to/manga-jv986530/chapter-2"
                                title="My Death Flags Show No Sign of Ending chapter Chapter 2: Save the Heroine's Mother!"
                                title="My Death Flags Show No Sign of Ending Chapter 2: Save the Heroine's Mother!">Vol.2 Chapter 2: Save The Heroine's Mother!</a>
                            <span class="chapter-view text-nowrap">270</span>
                            <span class="chapter-time text-nowrap fn-cover-item-time" data-fn-time="1599562482"
                                title="Sep 08,2020 10:09">Sep 08,2020</span>
                        </li>
                        <li class="a-h">
                        <a rel="nofollow" class="chapter-name text-nowrap" href="https://chapmanganato.to/manga-jv986530/chapter-1" title="My Death Flags Show No Sign of Ending chapter Chapter 1: Once I Realized It, I Was In A Game">Chapter 1: Once I Realized It, I Was In A Game</a>
                        <span class="chapter-view text-nowrap">371</span>
                        <span class="chapter-time text-nowrap fn-cover-item-time" data-fn-time="1599532412" title="Sep 08,2020 02:09">Sep 08,2020</span>
                                </li>
                    </ul>
                </div>
        "#;

        let expected_chapter: ManganatoChapter = ManganatoChapter {
            page_url: "https://chapmanganato.to/manga-jv986530/chapter-1".to_string(),
            title: Some("Once I Realized It, I Was In A Game".to_string()),
            number: "1".to_string(),
            volume: None,
            uploaded_at: "1599532412".to_string(),
        };

        let expected_chapter2: ManganatoChapter = ManganatoChapter {
            page_url: "https://chapmanganato.to/manga-jv986530/chapter-2".to_string(),
            title: Some("Save The Heroine's Mother!".to_string()),
            number: "2".to_string(),
            volume: Some("2".to_string()),
            uploaded_at: "1599562482".to_string(),
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

        assert_eq!(3, result.total_chapters);

        let html = "<h1> the div containing the chapters is not there</h1>";

        let result = ManganatoChaptersResponse::parse_html(HtmlElement::new(html))?;

        assert_eq!(0, result.total_chapters);

        Ok(())
    }

    #[test]
    fn chapter_page_is_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = r#"
        <div class="container">
            <div class="panel-breadcrumb">
                <a class="a-h" href="https://manganato.com/" title="Read Manga Online">Read Manga Online</a>
                <span>»</span>
                <a class="a-h" href="https://manganato.com/manga-zt1003076" title="Tonari no Kurokawa-san">Tonari No
                    Kurokawa-San</a>
                <span>»</span>
                <a class="a-h" href="https://chapmanganato.to/manga-zt1003076/chapter-11"
                    title="Chapter 11: A Heart-Pounding Outing!">Chapter 11: A Heart-Pounding Outing!</a>
            </div>

            <div class="panel-navigation">
                <select class="navi-change-chapter">
                    <option data-c="8">Chapter 8: After School Crisis (Part 2)</option>
                    <option data-c="7">Chapter 7: After School Crisis (Part 1)</option>
                    <option data-c="6">Chapter 6: I Want To Exchange</option>
                    <option data-c="5">Chapter 5: Crossed Feelings</option>
                    <option data-c="4">Chapter 4: I'm Just Curious</option>
                    <option data-c="3">Chapter 3: It's Not Naughty!</option>
                    <option data-c="2">Chapter 2: She's A Friend</option>
                    <option data-c="1">Chapter 1: Is She The Heroine?</option>
                </select>
                <div class="navi-change-chapter-btn"><a rel="nofollow" class="navi-change-chapter-btn-prev a-h"
                        href="https://chapmanganato.to/manga-zt1003076/chapter-10"><i></i>PREV CHAPTER</a><a
                        rel="nofollow" class="navi-change-chapter-btn-next a-h"
                        href="https://chapmanganato.to/manga-zt1003076/chapter-12">NEXT CHAPTER<i></i></a></div>
            </div>

            <div class="panel-chapter-info-top">
                <h1>TONARI NO KUROKAWA-SAN CHAPTER 11: A HEART-POUNDING OUTING!</h1>
                <div class="server-image">
                    <p class="server-image-caption">Image shows slow or error, you should choose another IMAGE SERVER
                    </p>
                    <span style="font-size: 12px;">
                        <span style="display: block;">
                            <span class="server-image-name">IMAGES SERVER: </span>
                            <a rel="nofollow" class="server-image-btn isactive">1</a>
                            <a data-l='https://chapmanganato.to/content_server_s2' rel="nofollow"
                                class="server-image-btn a-h">2</a>
                        </span>
                        <span style="display: block;">
                            <span class="server-image-name" style="margin-left: 10px;">LOAD ALL IMAGES AT ONCE:</span>
                            <label class="label-switch label-switch-on"
                                data-l="https://chapmanganato.to/content_lazyload_on">
                                <span class="switch-front"></span>
                                <span class="switch-end"></span>
                            </label>
                        </span>
                        <span style="display: block;">
                            <span class="server-image-name">IMAGES MARGIN: </span>
                            <select class="server-cbb-content-margin">
                                <option value="0">0</option>
                                <option value="1">1</option>
                                <option value="2">2</option>
                                <option value="3">3</option>
                                <option value="4">4</option>
                                <option value="5">5</option>
                                <option value="6">6</option>
                                <option value="7">7</option>
                                <option value="8">8</option>
                                <option value="9">9</option>
                                <option value="10">10</option>
                            </select>
                        </span>
                    </span>
                </div>
            </div>
        </div>

        <div class="container-chapter-reader">
            <div
                style="text-align: center; max-width: 620px; max-height: 310px; margin: 10px auto; overflow: hidden; display: block;">
                <div style="max-width: 300px; max-height: 300px; float: left; overflow: hidden;">
                    <div id="bg_162325351"></div>
                    <script data-cfasync="false" type="text/javascript"
                        src="//platform.bidgear.com/ads.php?domainid=1623&sizeid=2&zoneid=5351"></script>
                </div>
                <div style="max-width: 300px; max-height: 310px; float: left; margin-left: 20px;">
                    <div>
                        <script data-ssr="1" data-cfasync="false" async type="text/javascript"
                            src="//fz.untenseleopold.com/t67a0d5a4bec30/42238"></script>
                    </div>
                </div>
            </div><img
                src="https://v9.mkklcdnv6tempv3.com/img/tab_29/05/17/19/zt1003076/chapter_11_a_heartpounding_outing/1-1733210232-o.webp"
                alt="Tonari no Kurokawa-san Chapter 11: A Heart-Pounding Outing! page 1 - MangaNato.com"
                title="Tonari no Kurokawa-san Chapter 11: A Heart-Pounding Outing! page 1 - MangaNato.com" /><img
                src="https://v9.mkklcdnv6tempv3.com/img/tab_29/05/17/19/zt1003076/chapter_11_a_heartpounding_outing/2-1733210234-o.webp"
                alt="Tonari no Kurokawa-san Chapter 11: A Heart-Pounding Outing! page 2 - MangaNato.com"
                title="Tonari no Kurokawa-san Chapter 11: A Heart-Pounding Outing! page 2 - MangaNato.com" /><img
                src="https://v9.mkklcdnv6tempv3.com/img/tab_29/05/17/19/zt1003076/chapter_11_a_heartpounding_outing/3-1733210235-o.webp"
                alt="Tonari no Kurokawa-san Chapter 11: A Heart-Pounding Outing! page 3 - MangaNato.com"
                title="Tonari no Kurokawa-san Chapter 11: A Heart-Pounding Outing! page 3 - MangaNato.com" /><img
                src="https://v9.mkklcdnv6tempv3.com/img/tab_29/05/17/19/zt1003076/chapter_11_a_heartpounding_outing/4-1733210235-o.webp"
                alt="Tonari no Kurokawa-san Chapter 11: A Heart-Pounding Outing! page 4 - MangaNato.com"
                title="Tonari no Kurokawa-san Chapter 11: A Heart-Pounding Outing! page 4 - MangaNato.com" />
            <div
                style="text-align: center; max-width: 620px; max-height: 310px; margin: 10px auto; overflow: hidden; display: block;">
                <div style="max-width: 300px; max-height: 300px; float: left; overflow: hidden;">
                    <div id="bg_162322284"></div>
                    <script data-cfasync="false" type="text/javascript"
                        src="//platform.bidgear.com/ads.php?domainid=1623&sizeid=2&zoneid=2284"></script>
                </div>
                <div style="max-width: 300px; max-height: 310px; float: left; margin-left: 20px;">
                    <div id="pf-6966-1">
                        <script>window.pubfuturetag = window.pubfuturetag || []; window.pubfuturetag.push({unit: "658261d2a4aa62003d239fd7", id: "pf-6966-1"})</script>
                    </div>
                </div>
            </div><img
                src="https://v9.mkklcdnv6tempv3.com/img/tab_29/05/17/19/zt1003076/chapter_11_a_heartpounding_outing/5-1733210235-o.webp"
                alt="Tonari no Kurokawa-san Chapter 11: A Heart-Pounding Outing! page 5 - MangaNato.com"
                title="Tonari no Kurokawa-san Chapter 11: A Heart-Pounding Outing! page 5 - MangaNato.com" /><img
                src="https://v9.mkklcdnv6tempv3.com/img/tab_29/05/17/19/zt1003076/chapter_11_a_heartpounding_outing/6-1733210236-o.webp"
                alt="Tonari no Kurokawa-san Chapter 11: A Heart-Pounding Outing! page 6 - MangaNato.com"
                title="Tonari no Kurokawa-san Chapter 11: A Heart-Pounding Outing! page 6 - MangaNato.com" /><img
                src="https://v9.mkklcdnv6tempv3.com/img/tab_29/05/17/19/zt1003076/chapter_11_a_heartpounding_outing/7-1733210238-o.webp"
                alt="Tonari no Kurokawa-san Chapter 11: A Heart-Pounding Outing! page 7 - MangaNato.com"
                title="Tonari no Kurokawa-san Chapter 11: A Heart-Pounding Outing! page 7 - MangaNato.com" />
        </div>
        <div class="panel-breadcrumb">
                <span>
                    <a class="a-h" href="https://manganato.com/" title="Read Manga Online">
                        <span>Read Manga Online</span>
                    </a>
                </span>
                <span>»</span>
                <span>
                    <a class="a-h" href="https://manganato.com/manga-zt1003076" title="Tonari no Kurokawa-san">
                        <span>Tonari No Kurokawa-San</span>
                    </a>
                </span>
                <span>»</span>
                <span>
                    <a class="a-h" href="https://chapmanganato.to/manga-zt1003076/chapter-11"
                        title="Chapter 11: A Heart-Pounding Outing!">
                        <span>Chapter 11: A Heart-Pounding Outing!</span>
                    </a>
                </span>
            </div>
        "#;

        let result = ChapterPageResponse::parse_html(HtmlElement::new(html))?;

        assert!(result.pages_url.urls.iter().find(|url| *url == "https://v9.mkklcdnv6tempv3.com/img/tab_29/05/17/19/zt1003076/chapter_11_a_heartpounding_outing/1-1733210232-o.webp").is_some());

        let expected_chapter_list_item = ChaptersListItem {
            page_url: "https://chapmanganato.to/manga-zt1003076/chapter-1".to_string(),
            title: Some("Is She The Heroine?".to_string()),
            number: "1".to_string(),
            volume_number: None,
        };

        assert_eq!(
            Some(&expected_chapter_list_item),
            result
                .chapters_list
                .chapters
                .iter()
                .find(|chap| chap.page_url == "https://chapmanganato.to/manga-zt1003076/chapter-1")
        );

        assert_eq!(Some("A Heart-Pounding Outing!".to_string()), result.title);

        Ok(())
    }

    #[test]
    fn chapter_list_is_parsed_from_html() -> Result<(), Box<dyn Error>> {
        //On the website the select which contains the chapter list appears two times so when
        //parsing the html there shouldnt be duplicates
        // see for reference : https://chapmanganato.to/manga-ve998587/chapter-17
        let html = r#"
            <div class="panel-breadcrumb">
                <a class="a-h" href="https://manganato.com/" title="Read Manga Online">Read Manga Online</a>
                <span>»</span>
                <a class="a-h" href="https://manganato.com/manga-zt1003076" title="Tonari no Kurokawa-san">Tonari No
                    Kurokawa-San</a>
                <span>»</span>
                <a class="a-h" href="https://chapmanganato.to/manga-zt1003076/chapter-11"
                    title="Chapter 11: A Heart-Pounding Outing!">Chapter 11: A Heart-Pounding Outing!</a>
            </div>
            <div class="panel-navigation">
                <select class="navi-change-chapter">
                    <option data-c="8">Chapter 8</option>
                    <option data-c="7">Chapter 7</option>
                    <option data-c="6">Chapter 6</option>
                    <option data-c="5">Chapter 5</option>
                    <option data-c="4">Chapter 4</option>
                    <option data-c="3">Chapter 3</option>
                    <option data-c="2">Chapter 2</option>
                    <option data-c="1">Chapter 1</option>
                </select>
                <div class="navi-change-chapter-btn"><a rel="nofollow" class="navi-change-chapter-btn-prev a-h"
                        href="https://chapmanganato.to/manga-ve998587/chapter-17"><i></i>PREV CHAPTER</a></div>
            </div>
            <div class="panel-navigation">
                <select class="navi-change-chapter">
                    <option data-c="8">Chapter 8</option>
                    <option data-c="7">Chapter 7</option>
                    <option data-c="6">Chapter 6</option>
                    <option data-c="5">Chapter 5</option>
                    <option data-c="4">Chapter 4</option>
                    <option data-c="3">Chapter 3</option>
                    <option data-c="2">Chapter 2</option>
                    <option data-c="1">Chapter 1</option>
                </select>
                <div class="navi-change-chapter-btn"><a rel="nofollow" class="navi-change-chapter-btn-prev a-h"
                        href="https://chapmanganato.to/manga-ve998587/chapter-17"><i></i>PREV CHAPTER</a></div>
            </div>

        "#;

        let chapter_list = ChaptersList::parse_html(HtmlElement::new(html))?;

        assert_eq!(8, chapter_list.chapters.len());

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
