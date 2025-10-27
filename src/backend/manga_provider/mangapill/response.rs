use std::error::Error;
use std::fmt::{Display, Write};
use std::path::Path;

use chrono::NaiveDate;
use scraper::{ElementRef, html};

use crate::backend::html_parser::scraper::AsSelector;
use crate::backend::html_parser::{HtmlElement, HtmlParser, ParseHtml};
use crate::backend::manga_provider::{
    Author, ChapterPageUrl, ChapterReader, Genres, GetMangasResponse, Languages, ListOfChapters, Manga, MangaStatus, PopularManga,
    Rating, RecentlyAddedManga, SearchManga, SortedChapters, SortedVolumes, Volumes,
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
pub(super) struct PopularMangasMangaPill<T: HtmlParser> {
    parser: T,
}

impl<T: HtmlParser> PopularMangasMangaPill<T> {
    pub(super) fn new(parser: T) -> Self {
        Self { parser }
    }

    fn parse_popular_manga(&self, div: &HtmlElement) -> Result<PopularMangaItem, PopularMangaParseError> {
        let a_tags = self.parser.get_matching_elements_from(div, "a");

        let img = a_tags
            .first()
            .and_then(|a| self.parser.get_element_from(a, "img"))
            .and_then(|img| self.parser.get_element_attr(&img, "data-src"))
            .ok_or("img source not found")?;

        let chapter_number = a_tags.get(1).ok_or("Chapter number not found")?;

        let chapter_number = self.parser.get_element_from(chapter_number, "div").ok_or("element not found")?;
        let chapter_number = self.parser.get_inner_text(&chapter_number);

        let title_and_id = a_tags.last().ok_or("Title not found")?;

        let title = self.parser.get_element_from(title_and_id, "div").ok_or("title not found")?;
        let title = self.parser.get_inner_text(&title);

        let id = self
            .parser
            .get_element_attr(title_and_id, "href")
            .ok_or("a tag containing id was not found")?;

        let id = id.split("/").nth(2).ok_or("id wasnt succesfully extracted")?;

        Ok(PopularMangaItem {
            id: id.to_string(),
            title,
            cover_url: img,
            latest_chapter: Some(chapter_number),
        })
    }

    pub fn scrape_popular_mangas(self) -> Result<Vec<PopularMangaItem>, PopularMangaParseError> {
        let selector_popular_mangas = ".featured-grid > div";

        let mut res = vec![];

        let div_containing_mangas = self.parser.get_matching_elements(selector_popular_mangas);

        for el in div_containing_mangas.iter() {
            res.push(self.parse_popular_manga(el));
        }

        Ok(res.into_iter().flatten().collect())
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
pub(super) struct LatestMangas<T: HtmlParser> {
    parser: T,
}

impl<T: HtmlParser> LatestMangas<T> {
    pub(super) fn new(parser: T) -> Self {
        Self { parser }
    }

    fn parse_latest_manga(&self, div: &HtmlElement) -> Result<LatestMangItem, LatestMangaError> {
        let a_containing_latest_chapter = self
            .parser
            .get_element_from(dbg!(div), "a > div:nth-of-type(1)")
            .map(|el| self.parser.get_inner_text(&el));

        let img = self
            .parser
            .get_element_from(div, "img")
            .and_then(|el| self.parser.get_element_attr(&el, "data-src"))
            .ok_or("img src was not found")?;

        let a_containing_title_and_id = self
            .parser
            .get_element_from(div, ".px-1 > a:nth-of-type(2)")
            .ok_or("title was not found")?;

        let title = self
            .parser
            .get_element_from(&a_containing_title_and_id, "div")
            .ok_or("no title was found")?;

        let title = self.parser.get_inner_text(&title);

        let id = self
            .parser
            .get_element_attr(dbg!(&a_containing_title_and_id), "href")
            .ok_or("no id found")?;

        let id = id.split("/").nth(2).ok_or("failed to parse id")?;

        Ok(LatestMangItem {
            id: id.to_string(),
            title,
            cover_url: img,
            latest_chapter: a_containing_latest_chapter,
        })
    }

    pub(super) fn scrape_latest_mangas(self) -> Result<Vec<LatestMangItem>, LatestMangaError> {
        let selector_popular_mangas = "div.col-span-4:nth-child(1) > div:nth-child(2) > div";

        let mut res = vec![];

        let div_containing_mangas = self.parser.get_matching_elements(selector_popular_mangas);

        for el in div_containing_mangas.iter().take(5) {
            res.push(self.parse_latest_manga(el));
        }

        Ok(dbg!(res.into_iter().flatten().collect()))
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct MangaPillStatus {
    pub(super) name: String,
}

impl From<MangaPillStatus> for MangaStatus {
    fn from(value: MangaPillStatus) -> Self {
        match value.name.to_lowercase().as_str() {
            "ongoing" | "publishing" => MangaStatus::Ongoing,
            _ => MangaStatus::default(),
        }
    }
}
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct MangaPillTag {
    pub(super) name: String,
}

impl From<MangaPillTag> for Genres {
    fn from(value: MangaPillTag) -> Self {
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
    pub(super) tags: Vec<MangaPillTag>,
    pub(super) status: MangaPillStatus,
}

impl From<MangaPageData> for Manga {
    fn from(manga: MangaPageData) -> Self {
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
            author: None,
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

pub(super) struct MangaPageDataParser<T: HtmlParser> {
    scraper: T,
    id: String,
}

impl<T: HtmlParser> MangaPageDataParser<T> {
    pub(super) fn new(scraper: T, id: String) -> Self {
        Self { scraper, id }
    }

    pub(super) fn scrape_manga_page(self) -> Result<MangaPageData, MangaPageError> {
        let main_page_selector = ".sm\\:flex-row";

        let main_content = self.scraper.get_element(main_page_selector).ok_or("main content not found")?;

        let img = self
            .scraper
            .get_element_from(&main_content, "img")
            .and_then(|img| self.scraper.get_element_attr(&img, "data-src"))
            .ok_or("img src not found")?;

        let title = self.scraper.get_element_from(&main_content, "h1").ok_or("no title was found")?;

        let title = self.scraper.get_inner_text(&title);

        let description = self
            .scraper
            .get_element_from(&main_content, "p")
            .map(|el| self.scraper.get_inner_text(&el));

        let status = self
            .scraper
            .get_element_from(&main_content, "div.grid:nth-child(3) > div:nth-child(2) > div:nth-child(2)")
            .ok_or("No status was found")?;

        let status = self.scraper.get_inner_text(&status);

        let genres: Vec<MangaPillTag> = self
            .scraper
            .get_matching_elements_from(&main_content, "a.text-sm")
            .iter()
            .map(|a| MangaPillTag {
                name: self.scraper.get_inner_text(a),
            })
            .collect();

        let id = self.id;

        Ok(MangaPageData {
            cover_url: img,
            title,
            description,
            status: MangaPillStatus { name: status },
            tags: genres,
            id,
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
                /// Safe unwrapping because the src is always a valid url and thus has a extension
                let extension = Path::new(&sr).extension().unwrap().to_str().unwrap().to_string();
                pages.push(WeebcentralPage { url: sr, extension });
            }
        }

        Ok(Self { pages })
    }
}

/// Weeb central does not provide volume of mangas so they are all grouped in the `none` volume
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
        write!(f, "Failed to parse chapter page from weebcentral, more details about the error: {}", self.reason)
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

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct SearchPageItem {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) cover_url: String,
    pub(super) status: MangaPillStatus,
    pub(super) authors: Vec<String>,
    pub(super) tags: Vec<MangaPillTag>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct SearchPageMangas {
    pub(super) mangas: Vec<SearchPageItem>,
    /// Indicated wether or not more mangas can be fetched
    pub(super) more_result: bool,
}

#[derive(Debug)]
pub(super) struct SearchPageError {
    reason: String,
}

impl Display for SearchPageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to parse search page from weebcentral, more details about the error: {}", self.reason)
    }
}

impl<T: Into<String>> From<T> for SearchPageError {
    fn from(value: T) -> Self {
        let reason: String = value.into();
        Self { reason }
    }
}

impl Error for SearchPageError {}

impl ParseHtml for SearchPageItem {
    type ParseError = SearchPageError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let article = html::Html::parse_fragment(html.as_str());

        let a_selector = "span.tooltip.tooltip-bottom a".as_selector();

        let manga_url = article
            .select(&a_selector)
            .next()
            .ok_or(format!("No a tag containing id was found in {}", html.as_str()))?
            .attr("href")
            .ok_or("No href found")?;

        let img_selector = "source".as_selector();

        let cover_url = article
            .select(&img_selector)
            .next()
            .ok_or("No cover url tag found")?
            .attr("srcset")
            .ok_or("No srcset attribute found")?
            .to_string();

        let title_selector = ".line-clamp-1".as_selector();

        let title = article.select(&title_selector).next().ok_or("No title found")?.inner_html();

        let authors_selector =
            "article.bg-base-300:nth-child(1) > section:nth-child(2) > div:nth-child(5) > span > a".as_selector();

        let authors = article.select(&authors_selector).map(|a| a.inner_html().trim().to_string()).collect();

        let status_selector =
            "article.bg-base-300:nth-child(1) > section:nth-child(2) > div:nth-child(3) > span:nth-child(2)".as_selector();

        let status = MangaPillStatus {
            name: article.select(&status_selector).next().ok_or("No status tag found")?.inner_html(),
        };

        let tags_selector = "article.bg-base-300:nth-child(1) > section:nth-child(2) > div:nth-child(6) > span".as_selector();

        let tags = article
            .select(&tags_selector)
            .map(|tag| MangaPillTag {
                name: tag.inner_html().replace(",", ""),
            })
            .collect();

        Ok(Self {
            cover_url,
            id: extract_manga_id_from_url(manga_url),
            title,
            authors,
            status,
            tags,
        })
    }
}

impl ParseHtml for SearchPageMangas {
    type ParseError = SearchPageError;

    fn parse_html(html: HtmlElement) -> Result<Self, Self::ParseError> {
        let doc = html::Html::parse_fragment(html.as_str());
        let article_selector = "article.bg-base-300".as_selector();
        let more_mangas_button = "button.col-span-2".as_selector();

        let mut mangas: Vec<Result<SearchPageItem, <SearchPageItem as ParseHtml>::ParseError>> = vec![];

        for article in doc.select(&article_selector) {
            mangas.push(SearchPageItem::parse_html(HtmlElement::new(article.html())).inspect_err(|e| println!("{e}")));
        }

        let more_result = doc.select(&more_mangas_button).next().is_some();

        Ok(Self {
            mangas: mangas.into_iter().flatten().take(24).collect(),
            more_result,
        })
    }
}

/// Weebcentral does not provide the description of the manga and the artist
impl From<SearchPageItem> for SearchManga {
    fn from(manga: SearchPageItem) -> Self {
        Self {
            id: manga.id,
            title: manga.title,
            genres: manga.tags.into_iter().map(Genres::from).collect(),
            description: None,
            status: Some(manga.status.into()),
            cover_img_url: manga.cover_url,
            languages: vec![Languages::English],
            artist: None,
            author: None,
        }
    }
}

/// There is no way of knowing the total mangas of the search
impl From<SearchPageMangas> for GetMangasResponse {
    fn from(value: SearchPageMangas) -> Self {
        let amount_mangas = value.mangas.len();
        Self {
            mangas: value.mangas.into_iter().map(SearchManga::from).collect(),
            /// Since v0.7.0 weebcentral does not provide the total mangas and the pagination
            /// implementation requires it so that it knows it can query more mangas, so for now we
            /// put a ver large number as total mangas
            total_mangas: if value.more_result { 1000000 } else { amount_mangas as u32 },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::backend::html_parser::scraper::Scraper;
    use crate::backend::html_parser::{HtmlElement, ParseHtml};

    static HOME_PAGE_DOC: &str = include_str!("../../../../data_test/mangapill/home_page.txt");

    static MANGA_PAGE_DOC: &str = include_str!("../../../../data_test/mangapill/manga_page.txt");

    /// Obtained via: curl https://weebcentral.com/series/01J76XYCT4JVR13RN6NT1480MD/full-chapter-list
    static CHAPTER_LIST: &str = include_str!("../../../../data_test/weebcentral/full_chapters.txt");

    /// Obtained via: curl https://weebcentral.com/chapters/01JJB9BP43FHYCHAAZDVXKPSEW/images?is_prev=False&current_page=1&reading_style=long_strip
    static CHAPTER_PAGE_IMAGES_LIST: &str = include_str!("../../../../data_test/weebcentral/chapter_page_images.txt");

    /// Obtained via: curl https://weebcentral.com/search/data?limit=32&offset=0&sort=Best%20Match&order=Descending&official=Any&anime=Any&adult=Any&display_mode=Full%20Display
    static SEARCH_PAGE_DOC: &str = include_str!("../../../../data_test/weebcentral/search_page_paginated.txt");
    static SEARCH_PAGE_DOC_NOT_PAGINATED: &str = include_str!("../../../../data_test/weebcentral/search_page_no_more_result.txt");

    #[test]
    fn popular_manga_is_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = HOME_PAGE_DOC;

        let expected: PopularMangaItem = PopularMangaItem {
            id: "5281".to_string(),
            cover_url: "https://cdn.readdetectiveconan.com/file/mangapill/i/5281.jpeg".to_string(),
            title: "Sakamoto Days".to_string(),
            latest_chapter: Some("#234".to_string()),
        };

        let expected2: PopularMangaItem = PopularMangaItem {
            id: "2".to_string(),
            cover_url: "https://cdn.readdetectiveconan.com/file/mangapill/i/2.webp?h=01971742-5d7f-7f32-8d2b-d038279f8a73"
                .to_string(),
            title: "One Piece".to_string(),
            latest_chapter: Some("#1163".to_string()),
        };

        let scraper = PopularMangasMangaPill::new(Scraper::new(HtmlElement::new(html)));

        let result = scraper.scrape_popular_mangas().expect("failed to parse html");

        assert_eq!(8, result.len());

        let first = result.iter().find(|man| man.id == expected.id).unwrap();
        let second = result.iter().find(|man| man.id == expected2.id).unwrap();

        assert_eq!(&expected, first);
        assert_eq!(&expected2, second);

        Ok(())
    }

    #[test]
    fn latest_manga_is_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = HOME_PAGE_DOC;

        let expected: LatestMangItem = LatestMangItem {
            id: "8834".to_string(),
            cover_url: "https://cdn.readdetectiveconan.com/file/mangapill/i/8834.jpeg".to_string(),
            title: "Haikyo no Meshi".to_string(),
            latest_chapter: Some("#7".to_string()),
        };

        let expected2: LatestMangItem = LatestMangItem {
            id: "2140".to_string(),
            cover_url: "https://cdn.readdetectiveconan.com/file/mangapill/i/2140.jpg".to_string(),
            title: "Kakegurui".to_string(),
            latest_chapter: Some("#121".to_string()),
        };

        let scraper = LatestMangas::new(Scraper::new(HtmlElement::new(html)));

        let result = scraper.scrape_latest_mangas().expect("failed to parse html");

        assert_eq!(5, result.len());

        let first = result.iter().find(|man| man.id == expected.id).unwrap();
        let second = result.iter().find(|man| man.id == expected2.id).unwrap();

        assert_eq!(&expected, first);
        assert_eq!(&expected2, second);

        Ok(())
    }

    #[test]
    fn manga_page_is_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = MANGA_PAGE_DOC;

        let description = r#"Tarou Sakamoto was the ultimate assassin, feared by villains and admired by hitmen. But one day...he fell in love! Retirement, marriage, fatherhood and then... Sakamoto gained weight! The chubby guy who runs the neighborhood store is actually a former legendary hitman! Can he protect his family from danger? Get ready to experience a new kind of action comedy series!"#.to_string();

        let tags = vec![
            MangaPillTag {
                name: "Action".to_string(),
            },
            MangaPillTag {
                name: "Comedy".to_string(),
            },
            MangaPillTag {
                name: "Shounen".to_string(),
            },
            MangaPillTag {
                name: "Supernatural".to_string(),
            },
        ];

        let expected: MangaPageData = MangaPageData {
            id: "5281".to_string(),
            cover_url: "https://cdn.readdetectiveconan.com/file/mangapill/i/5281.jpeg".to_string(),
            title: "Sakamoto Days".to_string(),
            description: Some(description),
            tags,
            status: super::MangaPillStatus {
                name: "publishing".to_string(),
            },
        };

        let scraper = MangaPageDataParser::new(Scraper::new(HtmlElement::new(html)), "5281".to_string());

        let result = scraper.scrape_manga_page().unwrap();

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

    #[test]
    fn search_page_is_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = SEARCH_PAGE_DOC;
        let html2 = SEARCH_PAGE_DOC_NOT_PAGINATED;

        let tags = vec![
            MangaPillTag {
                name: "Comedy".to_string(),
            },
            MangaPillTag {
                name: "Drama".to_string(),
            },
            MangaPillTag {
                name: "Fantasy".to_string(),
            },
            MangaPillTag {
                name: "Romance".to_string(),
            },
        ];

        let expected: SearchPageItem = SearchPageItem {
            id: "01J76XY7E2VCSR0ZCC21KGXS1K".to_string(),
            cover_url: "https://temp.compsci88.com/cover/normal/01J76XY7E2VCSR0ZCC21KGXS1K.webp".to_string(),
            title: "Kobato.".to_string(),
            status: MangaPillStatus {
                name: "Complete".to_string(),
            },
            authors: vec!["APAPA Mokona".to_string(), "CLAMP".to_string(), "OHKAWA Ageha".to_string()],
            tags,
        };

        let result = SearchPageMangas::parse_html(HtmlElement::new(html))?;
        let result2 = SearchPageMangas::parse_html(HtmlElement::new(html2))?;

        assert!(result.more_result);
        assert!(!result.mangas.is_empty());

        let page = result.mangas.iter().find(|page| page.id == expected.id).unwrap();

        assert_eq!(expected, *page);

        assert!(!result2.more_result);

        Ok(())
    }
}
