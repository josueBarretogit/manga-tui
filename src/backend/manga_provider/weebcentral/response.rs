use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Write};
use std::num::ParseIntError;
use std::path::Path;

use chrono::NaiveDate;
use image::GenericImageView;
use regex::Regex;
use scraper::selectable::Selectable;
use scraper::{ElementRef, Selector, html};
use serde::{Deserialize, Serialize};

use crate::backend::html_parser::scraper::AsSelector;
use crate::backend::html_parser::{HtmlElement, ParseHtml};
use crate::backend::manga_provider::{
    Author, Chapter, ChapterPageUrl, ChapterReader, Genres, GetChaptersResponse, GetMangasResponse, Languages, ListOfChapters,
    Manga, MangaStatus, PopularManga, Rating, RecentlyAddedManga, SearchManga, SortedChapters, SortedVolumes, Volumes,
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
    pub(super) id: String,
    pub(super) cover_url: String,
    pub(super) title: String,
    pub(super) latest_chapter: Option<String>,
}

impl From<PopularMangaItem> for PopularManga {
    fn from(manga: PopularMangaItem) -> Self {
        Self {
            id: manga.id,
            title: manga.title,
            genres: vec![],
            description: format!("Latest chapter: {}", manga.latest_chapter.unwrap_or_default()),
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
        let page_url = page_url.attr("href").ok_or("no href found")?;

        let title_selector = ".truncate".as_selector();

        let title = article.select(&title_selector).next().ok_or("no title was found")?.inner_html();

        let latest_chapter = article.select(&"span".as_selector()).last().map(|tag| tag.inner_html());

        Ok(Self {
            id: extract_manga_id_from_url(page_url),
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
            .nth(1)
            .ok_or("The section containing mangas was not found")?;

        let mangas_selector = "article.md\\:hidden".as_selector();

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
    pub(super) id: String,
    pub(super) cover_url: String,
    pub(super) title: String,
    pub(super) latest_chapter: Option<String>,
}

impl From<LatestMangItem> for RecentlyAddedManga {
    fn from(manga: LatestMangItem) -> Self {
        Self {
            id: manga.id,
            title: manga.title,
            description: manga.latest_chapter.unwrap_or_default(),
            cover_img_url: manga.cover_url,
        }
    }
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
        let page_url = page_url.attr("href").ok_or("no href found")?;

        let title_selector = ".truncate".as_selector();

        let title = article
            .select(&title_selector)
            .next()
            .ok_or("no title was found")?
            .inner_html()
            .trim()
            .to_string();

        let latest_chapter = "span".as_selector();

        let latest_chapter = article.select(&latest_chapter).next().map(|tag| tag.inner_html());

        Ok(Self {
            id: extract_manga_id_from_url(&page_url),
            cover_url,
            title,
            latest_chapter,
        })
    }
}

impl ParseHtml for LatestMangas {
    type ParseError = LatestMangaError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let doc = html::Html::parse_document(html.as_str());
        let section_containing_mangas_selector = "main > section".as_selector();

        let mut mangas: Vec<Result<LatestMangItem, <LatestMangItem as ParseHtml>::ParseError>> = vec![];

        let section_containing_mangas = doc
            .select(&section_containing_mangas_selector)
            .nth(2)
            .ok_or("The section containing mangas was not found")?;

        let mangas_selector = "article".as_selector();

        for div in section_containing_mangas.select(&mangas_selector) {
            mangas.push(LatestMangItem::parse_html(HtmlElement::new(div.html())));
        }

        Ok(Self {
            mangas: mangas.into_iter().map(|may| may.unwrap()).collect(),
        })
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct WeebcentralStatus {
    pub(super) name: String,
}

impl From<WeebcentralStatus> for MangaStatus {
    fn from(value: WeebcentralStatus) -> Self {
        match value.name.to_lowercase().as_str() {
            "ongoing" => MangaStatus::Ongoing,
            _ => MangaStatus::default(),
        }
    }
}
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct WeebcentralTag {
    pub(super) name: String,
}

impl From<WeebcentralTag> for Genres {
    fn from(value: WeebcentralTag) -> Self {
        let rating = match value.name.to_lowercase().as_str() {
            "ecchi" => Rating::Moderate,
            "adult" => Rating::Nsfw,
            _ => Rating::default(),
        };

        Genres::new(value.name, rating)
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct MangaPageData {
    pub(super) id: String,
    pub(super) cover_url: String,
    pub(super) title: String,
    pub(super) description: Option<String>,
    pub(super) authors: Vec<String>,
    pub(super) tags: Vec<WeebcentralTag>,
    pub(super) status: WeebcentralStatus,
}

impl From<MangaPageData> for Manga {
    fn from(manga: MangaPageData) -> Self {
        let author = if manga.authors.is_empty() {
            None
        } else {
            Some(Author {
                id: "".to_string(),
                name: manga.authors.into_iter().fold(String::new(), |mut init, auth| {
                    let _ = write!(init, "{},", auth);
                    init
                }),
            })
        };

        Self {
            id: manga.id.clone(),
            id_safe_for_download: manga.id,
            title: manga.title,
            genres: manga.tags.into_iter().map(Genres::from).collect(),
            description: manga.description.unwrap_or("No description".to_string()),
            status: manga.status.into(),
            cover_img_url: manga.cover_url,
            languages: vec![Languages::English],
            rating: "".to_string(),
            artist: None,
            author,
        }
    }
}

/// Extracts the id from the manga page
/// # Examples
/// https://weebcentral.com/series/01J76XYCT4JVR13RN6NT1480MD/Tengoku-Daimakyou
/// returns : 01J76XYCT4JVR13RN6NT1480MD
pub(super) fn extract_manga_id_from_url(url: &str) -> String {
    let mut parts: Vec<&str> = url.split("/").collect();

    parts.reverse();

    parts.get(1).map(|id| id.to_string()).unwrap_or_default()
}

/// From a url replaces the last part after `/` with `full-chapter-list`
/// # Examples
/// https://weebcentral.com/series/01J76XYCT4JVR13RN6NT1480MD/Tengoku-Daimakyou
/// returns : https://weebcentral.com/series/01J76XYCT4JVR13RN6NT1480MD/full-chapter-list
pub(super) fn replace_last_segment_url(url: &str) -> String {
    let mut parts: Vec<&str> = url.rsplitn(2, '/').collect();
    if parts.len() > 1 {
        format!("{}/full-chapter-list", parts[1])
    } else {
        url.to_string() // If there's no "/", return the original URL
    }
}

#[derive(Debug)]
pub(super) struct MangaPageError {
    reason: String,
}

impl Display for MangaPageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to parse manga page from weebcentral, more details about the error: {}", self.reason)
    }
}

impl<T: Into<String>> From<T> for MangaPageError {
    fn from(value: T) -> Self {
        let reason: String = value.into();
        Self { reason }
    }
}

impl Error for MangaPageError {}

impl ParseHtml for MangaPageData {
    type ParseError = MangaPageError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let doc = html::Html::parse_document(html.as_str());
        let page_url_selector = r#"link[rel="canonical"]"#.as_selector();
        let page_url = doc
            .select(&page_url_selector)
            .next()
            .ok_or("Page url tag was not found")?
            .attr("href")
            .ok_or("No href found for link")?
            .to_string();

        let cover_url = doc.select(&"picture > source".as_selector()).next().ok_or("No cover tag was found")?;
        let cover_url = cover_url
            .attr("srcset")
            .ok_or("No attribute which contains cover url was found")?
            .to_string();

        let title = doc.select(&"h1".as_selector()).next().ok_or("No title was found")?.inner_html();

        let description = doc
            .select(&"p.whitespace-pre-wrap.break-words".as_selector())
            .next()
            .map(|el| el.inner_html());

        let binding = "li".as_selector();

        let more_data_list = doc
            .select(&"ul.flex.flex-col.gap-4".as_selector())
            .next()
            .ok_or("section which contains authors was not found")?
            .select(&binding);

        let mut authors: Vec<String> = vec![];
        let mut tags: Vec<WeebcentralTag> = vec![];
        let mut status = WeebcentralStatus::default();

        let selector = "span > a".as_selector();
        for list_item in more_data_list {
            let which_type = list_item
                .select(&"strong".as_selector())
                .next()
                .map(|el| el.inner_html())
                .unwrap_or_default();

            let which_type = which_type.trim();

            if which_type == "Author(s):" {
                list_item.select(&selector).for_each(|a| authors.push(a.inner_html()));
            } else if which_type == "Status:" {
                status = WeebcentralStatus {
                    name: list_item.select(&"a".as_selector()).next().ok_or("status was not found")?.inner_html(),
                };
            } else if which_type == "Tags(s):" {
                list_item.select(&selector).for_each(|a| {
                    tags.push(WeebcentralTag {
                        name: a.inner_html(),
                    })
                });
            }
        }

        Ok(Self {
            id: extract_manga_id_from_url(&page_url),
            cover_url,
            title,
            description,
            authors,
            status,
            tags,
        })
    }
}

#[derive(Debug)]
pub(super) struct ChaptersError {
    reason: String,
}

impl Display for ChaptersError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to parse chapter list from weebcentral, more details about the error: {}", self.reason)
    }
}

impl<T: Into<String>> From<T> for ChaptersError {
    fn from(value: T) -> Self {
        let reason: String = value.into();
        Self { reason }
    }
}

impl Error for ChaptersError {}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct WeebcentralChapter {
    pub(super) id: String,
    pub(super) number: String,
    pub(super) datetime: NaiveDate,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct WeebcentralChapters {
    pub(super) chapters: Vec<WeebcentralChapter>,
}

fn parse_chapter_from_tag(a: ElementRef<'_>) -> Result<WeebcentralChapter, ChaptersError> {
    let page_url = a.attr("href").ok_or("No href found")?.to_string();
    let id = page_url.split("/").last().ok_or("No chapter id found")?.to_string();

    let span_with_chapter = a
        .select(&"span.grow.flex.items-center.gap-2".as_selector())
        .next()
        .ok_or("No tag which contains chap number found")?;

    let chapter = span_with_chapter
        .select(&"span".as_selector())
        .next()
        .ok_or("No tag with chapter title")?;

    let number = chapter.inner_html();
    let number = number.split(" ").last().ok_or("No number found")?;

    let datetime = a.select(&"time".as_selector()).next().ok_or("No datetime found")?;
    let datetime = datetime.attr("datetime").ok_or("No datetime attribute found")?.to_string();

    let chapter: WeebcentralChapter = WeebcentralChapter {
        id,
        number: number.to_string(),
        datetime: chrono::DateTime::parse_from_rfc3339(&datetime).unwrap_or_default().date_naive(),
    };
    Ok(chapter)
}

impl ParseHtml for WeebcentralChapters {
    type ParseError = ChaptersError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let doc = html::Html::parse_document(html.as_str());
        let chapter_selector = "div > a".as_selector();

        let mut chapters: Vec<Result<WeebcentralChapter, ChaptersError>> = vec![];

        for a in doc.select(&chapter_selector) {
            chapters.push(parse_chapter_from_tag(a));
        }

        Ok(Self {
            chapters: chapters.into_iter().flatten().collect(),
        })
    }
}

#[derive(Debug)]
pub(super) struct ChapterPagesError {
    reason: String,
}

impl Display for ChapterPagesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to parse chapter list from weebcentral, more details about the error: {}", self.reason)
    }
}

impl<T: Into<String>> From<T> for ChapterPagesError {
    fn from(value: T) -> Self {
        let reason: String = value.into();
        Self { reason }
    }
}

impl Error for ChapterPagesError {}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct WeebcentralPage {
    pub(super) url: String,
    pub(super) extension: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct ChapterPagesLinks {
    pub(super) pages: Vec<WeebcentralPage>,
}

impl From<WeebcentralPage> for ChapterPageUrl {
    fn from(value: WeebcentralPage) -> Self {
        Self {
            url: value.url.parse().unwrap(),
            extension: value.extension,
        }
    }
}

impl ParseHtml for ChapterPagesLinks {
    type ParseError = ChapterPagesError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let section = html::Html::parse_fragment(html.as_str());

        let mut pages: Vec<WeebcentralPage> = vec![];

        for img in section.select(&"img".as_selector()) {
            let src = img.attr("src").map(|src| src.to_string());
            if let Some(sr) = src {
                let extension = Path::new(&sr).extension().unwrap().to_str().unwrap().to_string();
                pages.push(WeebcentralPage { url: sr, extension });
            }
        }

        Ok(Self { pages })
    }
}

impl From<WeebcentralChapters> for ListOfChapters {
    fn from(value: WeebcentralChapters) -> Self {
        let chapters: Vec<ChapterReader> = value
            .chapters
            .into_iter()
            .map(|chap| ChapterReader {
                id: chap.id,
                number: chap.number,
                volume: "none".to_string(),
            })
            .collect();

        Self {
            volumes: SortedVolumes::new(vec![Volumes {
                volume: "none".to_string(),
                chapters: SortedChapters::new(chapters),
            }]),
        }
    }
}

#[derive(Debug)]
pub(super) struct ChapterPageDataError {
    reason: String,
}

impl Display for ChapterPageDataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to parse chapter list from weebcentral, more details about the error: {}", self.reason)
    }
}

impl<T: Into<String>> From<T> for ChapterPageDataError {
    fn from(value: T) -> Self {
        let reason: String = value.into();
        Self { reason }
    }
}

impl Error for ChapterPageDataError {}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct ChapterPageData {
    pub(super) number: String,
}

impl ParseHtml for ChapterPageData {
    type ParseError = ChapterPageDataError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let html = html::Html::parse_document(html.as_str());
        let number = html
            .select(&"#nav-top > div:nth-child(1) > div:nth-child(1) > button:nth-child(2)".as_selector())
            .next()
            .ok_or("Button with chapter number was not found")?;

        let number = number
            .select(&"span".as_selector())
            .next()
            .ok_or("No span containing chapter number was found")?
            .inner_html();

        let number = number.split(" ").last().ok_or("No number found")?;

        Ok(Self {
            number: number.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use fake::rand::seq::IndexedRandom;
    use pretty_assertions::assert_eq;
    use reqwest::Url;
    use scraper::Html;

    use super::*;
    use crate::backend::html_parser::{HtmlElement, ParseHtml};

    /// Obtained via: curl https://weebcentral.com/
    static HOME_PAGE_DOC: &str = include_str!("../../../../data_test/weebcentral/home_page.txt");
    /// Obtained via: curl https://weebcentral.com/series/01J76XYCT4JVR13RN6NT1480MD/Tengoku-Daimakyou
    static MANGA_PAGE_DOC: &str = include_str!("../../../../data_test/weebcentral/manga_page.txt");

    /// Obtained via: curl
    /// https://weebcentral.com/series/01J76XYCT4JVR13RN6NT1480MD/full-chapter-list
    static CHAPTER_LIST: &str = include_str!("../../../../data_test/weebcentral/full_chapters.txt");

    /// Obtained via: curl
    /// https://weebcentral.com/chapters/01JJB9BP43FHYCHAAZDVXKPSEW/images?is_prev=False&current_page=1&reading_style=long_strip
    static CHAPTER_PAGE_IMAGES_LIST: &str = include_str!("../../../../data_test/weebcentral/chapter_page_images.txt");

    #[test]
    fn popular_manga_is_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = HOME_PAGE_DOC;

        let expected: PopularMangaItem = PopularMangaItem {
            id: "01J76XYEGBDP7J5P4S4QGZS05N".to_string(),
            cover_url: "https://temp.compsci88.com/cover/fallback/01J76XYEGBDP7J5P4S4QGZS05N.jpg".to_string(),
            title: "The Frozen Player Returns".to_string(),
            latest_chapter: Some("Chapter 160".to_string()),
        };

        let result = PopularMangasWeebCentral::parse_html(HtmlElement::new(html))?;

        assert!(result.mangas.len() > 1);

        let manga = result.mangas.iter().find(|man| man.id == expected.id).unwrap();

        assert_eq!(expected, *manga);

        Ok(())
    }

    #[test]
    fn latest_manga_is_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = HOME_PAGE_DOC;

        let expected: LatestMangItem = LatestMangItem {
            id: "01J76XYCT4JVR13RN6NT1480MD".to_string(),
            cover_url: "https://temp.compsci88.com/cover/fallback/01J76XYCT4JVR13RN6NT1480MD.jpg".to_string(),
            title: "Heavenly Delusion".to_string(),
            latest_chapter: Some("Chapter 71".to_string()),
        };
        let result = LatestMangas::parse_html(HtmlElement::new(html))?;

        assert!(result.mangas.len() > 1);

        let manga = result.mangas.iter().find(|man| man.id == expected.id).unwrap();

        assert_eq!(expected, *manga);

        Ok(())
    }

    #[test]
    fn it_extract_manga_id_from_url() {
        let url = "https://weebcentral.com/series/01J76XYCT4JVR13RN6NT1480MD/Tengoku-Daimakyou";
        assert_eq!("01J76XYCT4JVR13RN6NT1480MD", extract_manga_id_from_url(url));

        let url = "https://weebcentral.com/series/01J76XYHHK0E7Y3JP4ZWKVVN5Q/Anata-tachi-Soredemo-Sensei-Desu-ka";
        assert_eq!("01J76XYHHK0E7Y3JP4ZWKVVN5Q", extract_manga_id_from_url(url));
    }

    #[test]
    fn it_extracts_id_from_chapter_url() {
        let url = "https://weebcentral.com/series/01J76XYCT4JVR13RN6NT1480MD/Tengoku-Daimakyou";
        assert_eq!("01J76XYCT4JVR13RN6NT1480MD", extract_manga_id_from_url(url));

        let url = "https://weebcentral.com/series/01J76XYHHK0E7Y3JP4ZWKVVN5Q/Anata-tachi-Soredemo-Sensei-Desu-ka";
        assert_eq!("01J76XYHHK0E7Y3JP4ZWKVVN5Q", extract_manga_id_from_url(url));
    }

    #[test]
    fn it_replaces_title_with_full_chapter_list() {
        let url = "https://weebcentral.com/series/01J76XYCT4JVR13RN6NT1480MD/Tengoku-Daimakyou";
        assert_eq!("https://weebcentral.com/series/01J76XYCT4JVR13RN6NT1480MD/full-chapter-list", replace_last_segment_url(url));

        let url = "https://weebcentral.com/series/01J76XYHHK0E7Y3JP4ZWKVVN5Q/Anata-tachi-Soredemo-Sensei-Desu-ka";
        assert_eq!("https://weebcentral.com/series/01J76XYHHK0E7Y3JP4ZWKVVN5Q/full-chapter-list", replace_last_segment_url(url));
    }

    #[test]
    fn manga_page_is_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = MANGA_PAGE_DOC;

        let description = r#"The story is set in two distinct worlds. Tokio lives with other children inside a world surrounded by a beautiful wall, but one day he receives a message that reads, "Do you want to go outside?" Meanwhile, a boy named Maru travels with an older woman, eking out a meager existence in a ruined world as they search for "paradise.""#.to_string();

        let tags = vec![
            WeebcentralTag {
                name: "Adventure".to_string(),
            },
            WeebcentralTag {
                name: "Mystery".to_string(),
            },
            WeebcentralTag {
                name: "Romance".to_string(),
            },
            WeebcentralTag {
                name: "Sci-fi".to_string(),
            },
            WeebcentralTag {
                name: "Seinen".to_string(),
            },
            WeebcentralTag {
                name: "Tragedy".to_string(),
            },
        ];

        let expected: MangaPageData = MangaPageData {
            id: "01J76XYCT4JVR13RN6NT1480MD".to_string(),
            cover_url: "https://temp.compsci88.com/cover/normal/01J76XYCT4JVR13RN6NT1480MD.webp".to_string(),
            title: "Heavenly Delusion".to_string(),
            description: Some(description),
            authors: vec!["ISHIGURO Masakazu".to_string()],
            tags,
            status: super::WeebcentralStatus {
                name: "Ongoing".to_string(),
            },
        };

        let result = MangaPageData::parse_html(HtmlElement::new(html))?;

        assert_eq!(expected, result);

        Ok(())
    }

    #[test]
    fn list_of_chapters_if_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = CHAPTER_LIST;

        let expected: WeebcentralChapter = WeebcentralChapter {
            id: "01JQ5N6QKKBWVX427RT44K14WV".to_string(),
            number: "71".to_string(),
            datetime: chrono::DateTime::parse_from_rfc3339("2025-03-25T03:23:13.393Z")
                .unwrap_or_default()
                .date_naive(),
        };

        let result = WeebcentralChapters::parse_html(HtmlElement::new(html))?;
        assert!(result.chapters.len() > 70);

        let chap = result.chapters.iter().find(|chap| chap.id == expected.id).unwrap();

        assert_eq!(expected, *chap);

        Ok(())
    }

    #[test]
    fn chapter_pages_are_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = CHAPTER_PAGE_IMAGES_LIST;

        let expected = WeebcentralPage {
            url: "https://scans.lastation.us/manga/Tengoku-Daimakyou/0070-001.png".to_string(),
            extension: "png".to_string(),
        };

        let result = ChapterPagesLinks::parse_html(HtmlElement::new(html))?;

        assert!(!result.pages.is_empty());

        let page = result.pages.iter().find(|page| page.url == expected.url).unwrap();

        assert_eq!(expected, *page);

        Ok(())
    }
}
