//! HTML scraping and response models for the MangaPill provider.
//!
//! This module contains small data structures representing pieces of
//! information scraped from MangaPill (popular items, latest items,
//! manga detail pages, chapter lists, chapter pages and search results)
//! and a set of focused parsers that extract those structures from
//! previously-fetched HTML using the generic `HtmlParser` trait.
//!
//! The parsers here do not perform any network I/O; they only traverse
//! the provided DOM and normalize it into strongly-typed, testable
//! Rust structures that can later be converted into the app's public
//! types (e.g., `Manga`, `ListOfChapters`, etc.).
//!
//! Each parser:
//! - Targets one page/section
//! - Uses CSS selectors tailored to MangaPill's markup
//! - Returns a small DTO-like struct with exactly the data needed
//! - Fails gracefully by skipping invalid entries and returning errors with helpful messages when required fields are missing
//!
//! See the tests at the bottom of the file for examples of expected
//! HTML inputs and parsed outputs.
use std::error::Error;
use std::fmt::{Display, Write};
use std::path::Path;

use chrono::NaiveDate;
use manga_tui::make_error_ty;
use regex::Regex;
use reqwest::Url;
use scraper::{ElementRef, html};

use crate::backend::html_parser::scraper::AsSelector;
use crate::backend::html_parser::{HtmlElement, HtmlParser};
use crate::backend::manga_provider::{
    Author, ChapterPageUrl, ChapterReader, Genres, GetMangasResponse, Languages, ListOfChapters, Manga, MangaStatus, PopularManga,
    Rating, RecentlyAddedManga, SearchManga, SortedChapters, SortedVolumes, Volumes,
};

make_error_ty!(PopularMangaParseError, "Failed to parse popular manga from MangaPill, more details about the error: {}");
make_error_ty!(LatestMangaError, "Failed to parse latest manga from MangaPill, more details about the error: {}");
make_error_ty!(MangaPageError, "Failed to parse manga page from MangaPill, more details about the error: {}");
make_error_ty!(ChaptersError, "Failed to parse chapter list from MangaPill, more details about the error: {}");
make_error_ty!(ChapterPagesError, "Failed to parse chapter list from MangaPill, more details about the error: {}");
make_error_ty!(ChapterPageDataError, "Failed to parse chapter page from MangaPill, more details about the error: {}");
make_error_ty!(SearchPageError, "Failed to parse search page from MangaPill, more details about the error: {}");

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct PopularMangaItem {
    pub(super) id: String,
    pub(super) page_url: String,
    pub(super) cover_url: String,
    pub(super) title: String,
    pub(super) latest_chapter: Option<String>,
}

/// How to scrape the popoular mangas from MangaPill:
#[derive(Debug)]
pub(super) struct PopularMangasMangaPill<T: HtmlParser> {
    parser: T,
}

impl<T: HtmlParser> PopularMangasMangaPill<T> {
    /// Creates a new parser instance to scrape the "Popular" section from
    /// MangaPill's home page.
    pub(super) fn new(parser: T) -> Self {
        Self { parser }
    }

    /// Extracts one `PopularMangaItem` from the given container `div`.
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

        let page_url = self
            .parser
            .get_element_attr(title_and_id, "href")
            .ok_or("a tag containing id was not found")?;

        let id = page_url.split("/").nth(2).ok_or("id wasnt succesfully extracted")?;

        Ok(PopularMangaItem {
            id: id.to_string(),
            page_url: page_url.replacen("/", "", 1),
            title,
            cover_url: img,
            latest_chapter: Some(chapter_number),
        })
    }

    /// Scrapes the list of popular mangas found on the home page.
    pub fn scrape_popular_mangas(self) -> Result<Vec<PopularMangaItem>, PopularMangaParseError> {
        let selector_popular_mangas = ".featured-grid > div";

        let mut res = vec![];

        let div_containing_mangas = self.parser.get_matching_elements(selector_popular_mangas);

        for el in div_containing_mangas.iter() {
            if let Ok(parsed) = self.parse_popular_manga(el) {
                res.push(parsed);
            }
        }

        Ok(res)
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct LatestMangItem {
    pub(super) id: String,
    pub(super) page_url: String,
    pub(super) cover_url: String,
    pub(super) title: String,
    pub(super) latest_chapter: Option<String>,
}

/// How to scrape the latest mangas from MangaPill:
#[derive(Debug, Default)]
pub(super) struct LatestMangas<T: HtmlParser> {
    parser: T,
}

impl<T: HtmlParser> LatestMangas<T> {
    /// Creates a new parser instance to scrape the "Latest" section from
    /// MangaPill's home page.
    pub(super) fn new(parser: T) -> Self {
        Self { parser }
    }

    /// Extracts one `LatestMangItem` from the given container `div`.
    fn parse_latest_manga(&self, div: &HtmlElement) -> Result<LatestMangItem, LatestMangaError> {
        let a_containing_latest_chapter = self
            .parser
            .get_element_from(div, "a > div:nth-of-type(1)")
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

        let page_url = self.parser.get_element_attr(&a_containing_title_and_id, "href").ok_or("no id found")?;

        let id = page_url.split("/").nth(2).ok_or("failed to parse id")?;

        Ok(LatestMangItem {
            id: id.to_string(),
            page_url: page_url.replacen("/", "", 1),
            title,
            cover_url: img,
            latest_chapter: a_containing_latest_chapter,
        })
    }

    /// Scrapes at most the first five items from the latest updates list.
    pub(super) fn scrape_latest_mangas(self) -> Result<Vec<LatestMangItem>, LatestMangaError> {
        let selector_popular_mangas = "div.col-span-4:nth-child(1) > div:nth-child(2) > div";

        let mut res = vec![];

        let div_containing_mangas = self.parser.get_matching_elements(selector_popular_mangas);

        for el in div_containing_mangas.iter().take(5) {
            if let Ok(parsed) = self.parse_latest_manga(el) {
                res.push(parsed);
            }
        }

        Ok(res)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct MangaPillStatus {
    pub(super) name: String,
}

impl Default for MangaPillStatus {
    fn default() -> Self {
        Self {
            name: "publishing".to_string(),
        }
    }
}

impl From<MangaPillStatus> for MangaStatus {
    /// Maps MangaPill status names to the app-wide `MangaStatus` enum.
    fn from(value: MangaPillStatus) -> Self {
        match value.name.to_lowercase().as_str() {
            "ongoing" | "publishing" => MangaStatus::Ongoing,
            "finished" => MangaStatus::Completed,
            "on hiatus" => MangaStatus::Hiatus,
            "discontinued" => MangaStatus::Cancelled,
            _ => MangaStatus::default(),
        }
    }
}
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct MangaPillTag {
    pub(super) name: String,
}

impl From<MangaPillTag> for Genres {
    /// Converts a MangaPill tag into a `Genres` value and infers a content
    /// rating for specific tags where applicable.
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
    pub(super) cover_url: String,
    pub(super) title: String,
    pub(super) description: Option<String>,
    pub(super) tags: Vec<MangaPillTag>,
    pub(super) status: MangaPillStatus,
}

pub(super) struct MangaPageDataParser<T: HtmlParser> {
    scraper: T,
}

impl<T: HtmlParser> MangaPageDataParser<T> {
    /// Creates a new parser for a MangaPill manga detail page.
    pub(super) fn new(scraper: T) -> Self {
        Self { scraper }
    }

    /// Scrapes the manga detail page for basic metadata: cover, title,
    /// optional description, tags and status.
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

        Ok(MangaPageData {
            cover_url: img,
            title,
            description,
            status: MangaPillStatus { name: status },
            tags: genres,
        })
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct MangaPillChapterListItem {
    pub(super) id: String,
    pub(super) full_url: String,
    pub(super) number: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct MangaPillChapters {
    pub(super) chapters: Vec<MangaPillChapterListItem>,
}

#[derive(Debug)]
pub(super) struct MangaPillChaptersParser<T: HtmlParser> {
    scraper: T,
}

impl<T: HtmlParser> MangaPillChaptersParser<T> {
    /// Creates a new parser for the list of chapter links on a manga page.
    pub(super) fn new(scraper: T) -> Self {
        Self { scraper }
    }

    /// Scrapes the chapter list section, extracting id, full URL and number
    /// for each chapter.
    pub(super) fn scrape_list_of_chapters(self) -> Result<MangaPillChapters, ChaptersError> {
        let chapters_selector = "a.border";

        let scrape_a = |a: &HtmlElement| -> Option<MangaPillChapterListItem> {
            let url = self.scraper.get_element_attr(a, "href")?;
            let id = url.split("/").nth(2)?.split("-").last()?;

            Some(MangaPillChapterListItem {
                id: id.to_string(),
                full_url: url.replacen("/", "", 1),
                number: self.scraper.get_inner_text(a).split(" ").nth(1)?.to_string(),
            })
        };

        let chapters: Vec<MangaPillChapterListItem> = self
            .scraper
            .get_matching_elements(chapters_selector)
            .iter()
            .filter_map(scrape_a)
            .collect();

        Ok(MangaPillChapters { chapters })
    }
}

/// Manga pill central does not provide volume of mangas so they are all grouped in the `none` volume
impl From<MangaPillChapters> for ListOfChapters {
    /// Groups all chapters into a synthetic single volume named "none" and
    /// converts them to the app's `ListOfChapters` representation.
    fn from(value: MangaPillChapters) -> Self {
        let chapters: Vec<ChapterReader> = value
            .chapters
            .into_iter()
            .map(|chap| ChapterReader {
                id: chap.full_url,
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

#[derive(Debug, Default, PartialEq)]
pub(super) struct ChapterPageData {
    pub(super) manga_title: String,
    pub(super) number: f64,
    pub(super) pages_url: Vec<Url>,
}

#[derive(Debug)]
pub(super) struct ChapterPageDataParser<T: HtmlParser> {
    scraper: T,
}

impl<T: HtmlParser> ChapterPageDataParser<T> {
    /// Creates a new parser for a MangaPill chapter reader page.
    pub(super) fn new(scraper: T) -> Self {
        Self { scraper }
    }

    /// Scrapes one chapter reader page: extracts the full displayed title,
    /// the parsed numeric chapter number and the list of image page URLs.
    pub(super) fn scrape_page(self) -> Result<ChapterPageData, ChapterPageDataError> {
        let pages_selector = ".js-page";

        let title = self
            .scraper
            .get_element("h1")
            .map(|h1| self.scraper.get_inner_text(&h1))
            .unwrap_or("No title".to_string());
        let regex_find_number = Regex::new(r"\d+(\.\d+)?").unwrap();

        let number = regex_find_number
            .find(&title)
            .and_then(|mat| mat.as_str().parse::<f64>().ok())
            .unwrap_or_default();

        let pages_url: Vec<Url> = self
            .scraper
            .get_matching_elements(pages_selector)
            .iter()
            .filter_map(|img| self.scraper.get_element_attr(img, "data-src").and_then(|url| Url::parse(&url).ok()))
            .collect();

        Ok(ChapterPageData {
            manga_title: title,
            number,
            pages_url,
        })
    }
}

#[derive(Debug)]
pub(super) struct ChapterPagesScraper<T: HtmlParser> {
    scraper: T,
}

impl<T: HtmlParser> ChapterPagesScraper<T> {
    /// Creates a new parser for a MangaPill chapter reader page.
    pub(super) fn new(scraper: T) -> Self {
        Self { scraper }
    }

    /// Scrapes one chapter reader page: extracts the full displayed title,
    /// the parsed numeric chapter number and the list of image page URLs.
    pub(super) fn scrape_pages_from_chapter(self) -> Result<Vec<ChapterPageUrl>, ChapterPageDataError> {
        let pages_selector = ".js-page";

        let pages_url: Vec<ChapterPageUrl> = self
            .scraper
            .get_matching_elements(pages_selector)
            .iter()
            .filter_map(|img| {
                let url = self
                    .scraper
                    .get_element_attr(img, "data-src")
                    .and_then(|url| Url::parse(&url).ok())
                    .and_then(|url| {
                        let extension = Path::new(url.path()).extension().and_then(|ex| ex.to_str()).map(|ex| ex.to_string())?;
                        Some(ChapterPageUrl { extension, url })
                    });
                url
            })
            .collect();

        Ok(pages_url)
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct SearchPageItem {
    pub(super) page_url: String,
    pub(super) cover_url: String,
    pub(super) title: String,
    pub(super) status: MangaPillStatus,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct SearchPageMangas {
    pub(super) mangas: Vec<SearchPageItem>,
    /// Indicated wether or not more mangas can be fetched
    pub(super) next_page: Option<ButtonSearchPagination>,
    pub(super) previous_page: Option<ButtonSearchPagination>,
}

#[derive(Debug)]
pub(super) struct SearchPageMangasParser<T: HtmlParser> {
    scraper: T,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct ButtonSearchPagination {
    url: String,
}

impl<T: HtmlParser> SearchPageMangasParser<T> {
    /// Creates a new parser for MangaPill search result pages.
    pub(super) fn new(scraper: T) -> Self {
        Self { scraper }
    }

    /// Extracts a single `SearchPageItem` (cover, page URL, title, status)
    /// from the grid item `div`.
    fn scrape_search_item(&self, div: &HtmlElement) -> Result<SearchPageItem, SearchPageError> {
        let cover_url = self
            .scraper
            .get_element_from(div, "img")
            .and_then(|img| self.scraper.get_element_attr(&img, "data-src"))
            .ok_or("No img tag found")?;

        let a_containing_url_and_title = self.scraper.get_element_from(div, "a.mb-2").ok_or("a containing most info not found")?;

        let page_url = self
            .scraper
            .get_element_attr(&a_containing_url_and_title, "href")
            .map(|attr| attr.replacen("/", "", 1))
            .ok_or("Page url not found")?;

        let title = self
            .scraper
            .get_element_from(&a_containing_url_and_title, "div")
            .map(|el| self.scraper.get_inner_text(&el))
            .ok_or("No title found")?;

        let status = self
            .scraper
            .get_element_from(div, "div.text-xs.leading-5:last-of-type")
            .map(|di| MangaPillStatus {
                name: self.scraper.get_inner_text(&di),
            })
            .unwrap_or_default();

        Ok(SearchPageItem {
            page_url,
            cover_url,
            title,
            status,
        })
    }

    /// Scrapes the entire search results page including next/previous
    /// pagination button (if present).
    pub(super) fn scrape_search_page(self) -> Result<SearchPageMangas, SearchPageError> {
        let divs_selector = "div.my-3:nth-child(3) > div";
        let selector_button_next_previous_page = "a.btn";

        let div_containing_search_items = self.scraper.get_matching_elements(divs_selector);
        let mut mangas: Vec<_> = vec![];

        for div in div_containing_search_items.iter() {
            if let Ok(scraped) = self.scrape_search_item(div) {
                mangas.push(scraped);
            }
        }

        let mut next_page = None;
        let mut previous = None;

        let button = self.scraper.get_element(selector_button_next_previous_page);

        if let Some(btn) = button {
            let url = self.scraper.get_element_attr(&btn, "href").unwrap_or_default();
            if self.scraper.get_inner_text(&btn).to_lowercase() == "next" {
                next_page = Some(ButtonSearchPagination { url })
            } else {
                previous = Some(ButtonSearchPagination { url });
            }
        }

        Ok(SearchPageMangas {
            mangas,
            next_page,
            previous_page: previous,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::backend::html_parser::HtmlElement;
    use crate::backend::html_parser::scraper::Scraper;

    static HOME_PAGE_DOC: &str = include_str!("../../../../data_test/mangapill/home_page.txt");

    static MANGA_PAGE_DOC: &str = include_str!("../../../../data_test/mangapill/manga_page.txt");

    static CHAPTER_PAGE: &str = include_str!("../../../../data_test/mangapill/chapter-page.txt");

    static SEARCH_PAGE_DOC: &str = include_str!("../../../../data_test/mangapill/search_page.txt");
    static SEARCH_PAGE_DOC_NOT_FOUND: &str = include_str!("../../../../data_test/mangapill/search_page_not_found.txt");

    #[test]
    fn popular_manga_is_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = HOME_PAGE_DOC;

        let expected: PopularMangaItem = PopularMangaItem {
            id: "5281".to_string(),
            page_url: "manga/5281/sakamoto-days".to_string(),
            cover_url: "https://cdn.readdetectiveconan.com/file/mangapill/i/5281.jpeg".to_string(),
            title: "Sakamoto Days".to_string(),
            latest_chapter: Some("#234".to_string()),
        };

        let expected2: PopularMangaItem = PopularMangaItem {
            id: "2".to_string(),
            page_url: "manga/2/one-piece".to_string(),
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
            page_url: "manga/8834/haikyo-no-meshi".to_string(),
            cover_url: "https://cdn.readdetectiveconan.com/file/mangapill/i/8834.jpeg".to_string(),
            title: "Haikyo no Meshi".to_string(),
            latest_chapter: Some("#7".to_string()),
        };

        let expected2: LatestMangItem = LatestMangItem {
            id: "2140".to_string(),
            page_url: "manga/2140/kakegurui".to_string(),
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
            cover_url: "https://cdn.readdetectiveconan.com/file/mangapill/i/5281.jpeg".to_string(),
            title: "Sakamoto Days".to_string(),
            description: Some(description),
            tags,
            status: super::MangaPillStatus {
                name: "publishing".to_string(),
            },
        };

        let scraper = MangaPageDataParser::new(Scraper::new(HtmlElement::new(html)));

        let result = scraper.scrape_manga_page().unwrap();

        assert_eq!(expected, result);

        Ok(())
    }

    #[test]
    fn list_of_chapters_if_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = MANGA_PAGE_DOC;

        let expected: MangaPillChapterListItem = MangaPillChapterListItem {
            id: "10234000".to_string(),
            full_url: "chapters/5281-10234000/sakamoto-days-chapter-234".to_string(),
            number: "234".to_string(),
        };

        let expected2: MangaPillChapterListItem = MangaPillChapterListItem {
            id: "10233000".to_string(),
            full_url: "chapters/5281-10233000/sakamoto-days-chapter-233".to_string(),
            number: "233".to_string(),
        };

        let scraper = MangaPillChaptersParser::new(Scraper::new(HtmlElement::new(html)));

        let result = scraper.scrape_list_of_chapters().unwrap();

        assert_eq!(234, result.chapters.len());

        let chap = result.chapters.iter().find(|chap| chap.id == expected.id).unwrap();
        let chap2 = result.chapters.iter().find(|chap| chap.id == expected2.id).unwrap();

        assert_eq!(expected, *chap);
        assert_eq!(expected2, *chap2);

        Ok(())
    }

    #[test]
    fn chapter_page_is_parse_from_html() -> Result<(), Box<dyn Error>> {
        let html = CHAPTER_PAGE;

        let expected_pages: Vec<Url> = [
            "https://cdn.readdetectiveconan.com/file/mangap/9280/10011100/0199dd6e-24c5-7d07-97e8-6b57345542e3/1.jpeg",
            "https://cdn.readdetectiveconan.com/file/mangap/9280/10011100/0199dd6e-24c5-7d07-97e8-6b57345542e3/2.jpeg",
            "https://cdn.readdetectiveconan.com/file/mangap/9280/10011100/0199dd6e-24c5-7d07-97e8-6b57345542e3/3.jpeg",
        ]
        .iter()
        .map(|url| url.parse().unwrap())
        .collect();

        let expected: ChapterPageData = ChapterPageData {
            manga_title: "Tada no Murabito no Boku ga, Sanbyakunenmae no Boukun
                Ouji ni Tensei shite Shimaimashita: Zensei no Chishiki de Ansatsu Flag wo Kaihi shite, Odayaka ni
                Ikinokorimasu! Chapter 11.1"
                .to_string(),
            number: 11.1,
            pages_url: expected_pages.clone(),
        };

        let scraper = ChapterPageDataParser::new(Scraper::new(HtmlElement::new(html)));

        let result = scraper.scrape_page().unwrap();

        assert_eq!(expected.manga_title, result.manga_title);
        assert_eq!(expected.number, result.number);

        assert_eq!(expected_pages[0], result.pages_url[0]);
        assert_eq!(expected_pages[1], result.pages_url[1]);
        assert_eq!(expected_pages[2], result.pages_url[2]);

        assert_eq!(18, result.pages_url.len());
        Ok(())
    }

    #[test]
    fn pages_are_scraped_from_chapter_document() -> Result<(), Box<dyn Error>> {
        let html = include_str!("../../../../data_test/mangapill/chapter_page_special.txt");

        let expected_pages: Vec<ChapterPageUrl> = [
            "https://cdn.readdetectiveconan.com/file/mangap/5281/10001000/1.jpeg?t=1725134176",
            "https://cdn.readdetectiveconan.com/file/mangap/5281/10001000/2.jpeg?t=1725134176",
            "https://cdn.readdetectiveconan.com/file/mangap/5281/10001000/3.jpeg?t=1725134176",
            "https://cdn.readdetectiveconan.com/file/mangap/5281/10001000/4.jpeg?t=1725134176",
        ]
        .iter()
        .map(|url| ChapterPageUrl {
            url: url.parse().unwrap(),
            extension: "jpeg".to_string(),
        })
        .collect();

        let scraper = ChapterPagesScraper::new(Scraper::new(HtmlElement::new(html)));

        let result = scraper.scrape_pages_from_chapter().unwrap();

        for (index, expected) in expected_pages.iter().enumerate() {
            assert_eq!(*expected, result[index]);
        }

        assert_eq!(58, result.len());
        Ok(())
    }

    #[test]
    fn search_page_is_parsed_from_html() -> Result<(), Box<dyn Error>> {
        let html = SEARCH_PAGE_DOC;
        let html_not_found = SEARCH_PAGE_DOC_NOT_FOUND;

        let expected: SearchPageItem = SearchPageItem {
            page_url: "manga/3760/school-days".to_string(),
            cover_url: "https://cdn.readdetectiveconan.com/file/mangapill/i/3760.jpg".to_string(),
            title: "School Days".to_string(),
            status: MangaPillStatus {
                name: "finished".to_string(),
            },
        };

        let expected2: SearchPageItem = SearchPageItem {
            page_url: "manga/3761/school-ningyo".to_string(),
            cover_url: "https://cdn.readdetectiveconan.com/file/mangapill/i/3761.jpg".to_string(),
            title: "School Ningyo".to_string(),
            status: MangaPillStatus {
                name: "finished".to_string(),
            },
        };

        let scraper = SearchPageMangasParser::new(Scraper::new(HtmlElement::new(html)));

        let result = scraper.scrape_search_page()?;

        assert_eq!(
            Some(ButtonSearchPagination {
                url: "/search?q=school&status=&type=&page=2".to_string()
            }),
            result.next_page
        );
        assert!(result.previous_page.is_none());

        let res1 = result.mangas.iter().find(|ma| ma.page_url == expected.page_url).unwrap();
        let res2 = result.mangas.iter().find(|ma| ma.page_url == expected2.page_url).unwrap();

        assert_eq!(expected, *res1);
        assert_eq!(expected2, *res2);

        let scraper = SearchPageMangasParser::new(Scraper::new(HtmlElement::new(html_not_found)));

        let result = scraper.scrape_search_page()?;

        assert_eq!(0, result.mangas.len());

        Ok(())
    }
}
