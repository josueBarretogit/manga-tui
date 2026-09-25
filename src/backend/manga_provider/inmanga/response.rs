use std::error::Error;
use std::fmt::Display;

use chrono::NaiveDate;
use reqwest::Url;
use scraper::{Html, Selector};
use serde::Deserialize;

use crate::backend::manga_provider::{
    Chapter, ChapterPageUrl, ChapterReader, Languages, Manga, MangaStatus, PopularManga, RecentlyAddedManga, SearchManga,
};

pub static INMANGA_BASE_URL: &str = "https://inmanga.com";
pub static INMANGA_CDN_URL: &str = "https://cdn1.intomanga.com";

#[derive(Debug)]
pub struct InmangaParseError {
    reason: String,
}

impl Display for InmangaParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to parse InManga response: {}", self.reason)
    }
}

impl<T: Into<String>> From<T> for InmangaParseError {
    fn from(value: T) -> Self {
        Self {
            reason: value.into(),
        }
    }
}

impl Error for InmangaParseError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InmangaMangaItem {
    pub id: String,
    pub title: String,
    pub cover_url: String,
    pub status: Option<MangaStatus>,
}

impl From<InmangaMangaItem> for PopularManga {
    fn from(item: InmangaMangaItem) -> Self {
        Self {
            id: item.id,
            title: item.title,
            genres: vec![],
            description: "Popular en InManga".to_string(),
            status: item.status,
            cover_img_url: item.cover_url,
        }
    }
}

impl From<InmangaMangaItem> for RecentlyAddedManga {
    fn from(item: InmangaMangaItem) -> Self {
        Self {
            id: item.id,
            title: item.title,
            description: String::new(),
            cover_img_url: item.cover_url,
        }
    }
}

impl From<InmangaMangaItem> for SearchManga {
    fn from(item: InmangaMangaItem) -> Self {
        Self {
            id: item.id,
            title: item.title,
            genres: vec![],
            description: None,
            status: item.status,
            cover_img_url: item.cover_url,
            languages: vec![Languages::Spanish],
            artist: None,
            author: None,
        }
    }
}

impl InmangaMangaItem {
    pub fn parse_consult_results(html_content: &str) -> Vec<Self> {
        let fragment = Html::parse_fragment(html_content);
        let link_selector = Selector::parse("a.manga-result, a[href*='/ver/manga/']").unwrap();
        let title_selector = Selector::parse("h4.m0").unwrap();
        let img_selector = Selector::parse("img").unwrap();

        let mut results = vec![];

        for element in fragment.select(&link_selector) {
            let href = match element.value().attr("href") {
                Some(h) => h,
                None => continue,
            };

            let id = match href.trim_end_matches('/').rsplit('/').next() {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => continue,
            };

            let title = element
                .select(&title_selector)
                .next()
                .map(|el| el.text().collect::<String>().trim().to_string())
                .unwrap_or_else(|| "Sin título".to_string());

            let cover_url = element
                .select(&img_selector)
                .next()
                .and_then(|el| el.value().attr("data-src").or_else(|| el.value().attr("src")))
                .map(
                    |src| {
                        if src.starts_with("http") { src.to_string() } else { format!("{INMANGA_CDN_URL}/i/m/{id}/t/o/{id}.jpg") }
                    },
                )
                .unwrap_or_else(|| format!("{INMANGA_CDN_URL}/i/m/{id}/t/o/{id}.jpg"));

            let raw_text = element.text().collect::<String>();
            let status = if raw_text.contains("Finalizado") {
                Some(MangaStatus::Completed)
            } else if raw_text.contains("En emisión") {
                Some(MangaStatus::Ongoing)
            } else {
                Some(MangaStatus::Ongoing)
            };

            results.push(Self {
                id,
                title,
                cover_url,
                status,
            });
        }

        results
    }
}

#[derive(Debug, Clone)]
pub struct InmangaMangaDetails {
    pub id: String,
    pub title: String,
    pub description: String,
    pub cover_url: String,
    pub status: MangaStatus,
}

impl InmangaMangaDetails {
    pub fn parse(html_content: &str, manga_id: &str) -> Result<Self, InmangaParseError> {
        let doc = Html::parse_document(html_content);
        let h1_selector = Selector::parse("div.col-md-9 h1, h1").unwrap();
        let body_selector = Selector::parse("div.col-md-9 div.panel-body, div.panel-body").unwrap();
        let col3_selector = Selector::parse("div.col-md-3").unwrap();
        let img_selector = Selector::parse("div.col-md-3 img").unwrap();

        let title = doc
            .select(&h1_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| {
                // Try from meta og:title
                let meta_selector = Selector::parse("meta[property='og:title']").unwrap();
                doc.select(&meta_selector)
                    .next()
                    .and_then(|m| m.value().attr("content"))
                    .map(|c| c.replace(" Manga Online - InManga", "").trim().to_string())
                    .unwrap_or_else(|| manga_id.to_string())
            });

        let description = doc
            .select(&body_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        let cover_url = doc
            .select(&img_selector)
            .next()
            .and_then(|el| el.value().attr("src"))
            .filter(|src| src.starts_with("http"))
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{INMANGA_CDN_URL}/i/m/{manga_id}/t/o/{manga_id}.jpg"));

        let col3_text = doc
            .select(&col3_selector)
            .next()
            .map(|el| el.text().collect::<String>())
            .unwrap_or_default();

        let status = if col3_text.contains("Finalizado") { MangaStatus::Completed } else { MangaStatus::Ongoing };

        Ok(Self {
            id: manga_id.to_string(),
            title,
            description,
            cover_url,
            status,
        })
    }
}

impl From<InmangaMangaDetails> for Manga {
    fn from(details: InmangaMangaDetails) -> Self {
        Self {
            id: details.id.clone(),
            id_safe_for_download: details.id,
            title: details.title,
            genres: vec![],
            description: details.description,
            status: details.status,
            cover_img_url: details.cover_url,
            languages: vec![Languages::Spanish],
            rating: "10".to_string(),
            artist: None,
            author: None,
        }
    }
}

#[derive(Deserialize)]
pub struct RawInmangaChaptersResponse {
    pub data: String,
}

#[derive(Deserialize)]
pub struct InnerInmangaChaptersResult {
    pub result: Vec<InmangaChapterDto>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct InmangaChapterDto {
    pub pages_count: Option<u32>,
    pub number: Option<f64>,
    pub registration_date: Option<String>,
    pub identification: String,
    pub friendly_chapter_number: Option<String>,
    pub description: Option<String>,
}

impl InmangaChapterDto {
    pub fn parse_publication_date(&self) -> Option<NaiveDate> {
        let date_str = self.registration_date.as_ref()?;
        if date_str.len() >= 10 { NaiveDate::parse_from_str(&date_str[..10], "%Y-%m-%d").ok() } else { None }
    }

    pub fn to_chapter(&self, manga_id: &str) -> Chapter {
        let chapter_num_str = self
            .friendly_chapter_number
            .clone()
            .unwrap_or_else(|| self.number.map(|n| n.to_string()).unwrap_or_default());

        let title = match &self.description {
            Some(desc) if !desc.trim().is_empty() => desc.trim().to_string(),
            _ => format!("Capítulo {chapter_num_str}"),
        };

        Chapter {
            id: self.identification.clone(),
            id_safe_for_download: self.identification.clone(),
            manga_id: manga_id.to_string(),
            title,
            language: Languages::Spanish,
            chapter_number: chapter_num_str,
            volume_number: None,
            scanlator: None,
            publication_date: self.parse_publication_date(),
        }
    }

    pub fn to_chapter_reader(&self) -> ChapterReader {
        let chapter_num_str = self
            .friendly_chapter_number
            .clone()
            .unwrap_or_else(|| self.number.map(|n| n.to_string()).unwrap_or_default());

        ChapterReader {
            id: self.identification.clone(),
            number: chapter_num_str,
            volume: "none".to_string(),
        }
    }
}

pub struct InmangaChapterPagesData {
    pub manga_id: String,
    pub chapter_id: String,
    pub chapter_number: String,
    pub pages: Vec<ChapterPageUrl>,
}

impl InmangaChapterPagesData {
    pub fn parse(html_content: &str, fallback_manga_id: &str, chapter_id: &str) -> Result<Self, InmangaParseError> {
        let doc = Html::parse_document(html_content);

        let manga_id_selector = Selector::parse("input#MangaIdentification").unwrap();
        let manga_id = doc
            .select(&manga_id_selector)
            .next()
            .and_then(|el| el.value().attr("value"))
            .filter(|v| !v.trim().is_empty())
            .unwrap_or(fallback_manga_id)
            .to_string();

        let chapter_id_selector = Selector::parse("input#ChapterIdentification").unwrap();
        let resolved_chapter_id = doc
            .select(&chapter_id_selector)
            .next()
            .and_then(|el| el.value().attr("value"))
            .filter(|v| !v.trim().is_empty())
            .unwrap_or(chapter_id)
            .to_string();

        // Extract chapter number from selected option in chapter dropdown
        let selected_option_selector = Selector::parse("select option[selected='selected']").unwrap();
        let chapter_number = doc
            .select(&selected_option_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        // Extract pages from img.ImageContainer or select#PageList option
        let img_selector = Selector::parse("img.ImageContainer").unwrap();
        let mut page_ids = vec![];

        for img in doc.select(&img_selector) {
            if let Some(id) = img.value().attr("id")
                && !id.trim().is_empty()
            {
                page_ids.push(id.to_string());
            }
        }

        if page_ids.is_empty() {
            let option_selector = Selector::parse("select#PageList option, select.PageListClass option").unwrap();
            for opt in doc.select(&option_selector) {
                if let Some(val) = opt.value().attr("value")
                    && !val.trim().is_empty()
                {
                    page_ids.push(val.to_string());
                }
            }
        }

        if page_ids.is_empty() {
            return Err("No pages found in InManga chapter".into());
        }

        let mut pages = vec![];
        for page_id in page_ids {
            let raw_url = format!("{INMANGA_CDN_URL}/i/m/{manga_id}/c/{resolved_chapter_id}/o/{page_id}.jpg");
            if let Ok(url) = Url::parse(&raw_url) {
                pages.push(ChapterPageUrl {
                    url,
                    extension: "jpg".to_string(),
                });
            }
        }

        Ok(Self {
            manga_id,
            chapter_id: resolved_chapter_id,
            chapter_number,
            pages,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_consult_results() {
        let html = r#"
            <a href="/ver/manga/Sousou-no-Frieren/60f0c76f-f921-4748-905c-0030c02401b3" class="manga-result col-md-4 col-sm-6 col-xs-12">
                <div class="panel widget">
                    <h4 class="m0 list-group-item ellipsed-text">Sousou no Frieren</h4>
                    <img data-src="https://cdn1.intomanga.com/i/m/60f0c76f-f921-4748-905c-0030c02401b3/t/o/60f0c76f-f921-4748-905c-0030c02401b3.jpg" class="ImageContainer">
                    <span class="label label-success pull-right">En emisión</span>
                </div>
            </a>
        "#;
        let items = InmangaMangaItem::parse_consult_results(html);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "60f0c76f-f921-4748-905c-0030c02401b3");
        assert_eq!(items[0].title, "Sousou no Frieren");
        assert_eq!(items[0].status, Some(MangaStatus::Ongoing));
        assert_eq!(
            items[0].cover_url,
            "https://cdn1.intomanga.com/i/m/60f0c76f-f921-4748-905c-0030c02401b3/t/o/60f0c76f-f921-4748-905c-0030c02401b3.jpg"
        );
    }

    #[test]
    fn test_parse_chapters_json() {
        let json = r#"{"data":"{\"result\":[{\"PagesCount\":20,\"Number\":146.00,\"RegistrationDate\":\"2025-10-21T09:04:35.8940981\",\"Identification\":\"4085F005-C102-46D1-8A77-56F544CF4804\",\"FriendlyChapterNumber\":\"146\",\"Description\":\"\"}]}"}"#;
        let raw: RawInmangaChaptersResponse = serde_json::from_str(json).unwrap();
        let inner: InnerInmangaChaptersResult = serde_json::from_str(&raw.data).unwrap();
        assert_eq!(inner.result.len(), 1);
        let chap = inner.result[0].to_chapter("test-manga-id");
        assert_eq!(chap.id, "4085F005-C102-46D1-8A77-56F544CF4804");
        assert_eq!(chap.chapter_number, "146");
        assert_eq!(chap.title, "Capítulo 146");
        assert_eq!(chap.publication_date, NaiveDate::from_ymd_opt(2025, 10, 21));
    }

    #[test]
    fn test_parse_chapter_pages() {
        let html = r#"
            <input type="hidden" value="4085f005-c102-46d1-8a77-56f544cf4804" id="ChapterIdentification" />
            <input type="hidden" value="60f0c76f-f921-4748-905c-0030c02401b3" id="MangaIdentification" />
            <select><option selected="selected" value="4085f005-c102-46d1-8a77-56f544cf4804">146</option></select>
            <img id="page-1-uuid" class="ImageContainer" />
            <img id="page-2-uuid" class="ImageContainer" />
        "#;
        let data = InmangaChapterPagesData::parse(html, "fallback", "4085f005-c102-46d1-8a77-56f544cf4804").unwrap();
        assert_eq!(data.manga_id, "60f0c76f-f921-4748-905c-0030c02401b3");
        assert_eq!(data.chapter_number, "146");
        assert_eq!(data.pages.len(), 2);
        assert_eq!(
            data.pages[0].url.as_str(),
            "https://cdn1.intomanga.com/i/m/60f0c76f-f921-4748-905c-0030c02401b3/c/4085f005-c102-46d1-8a77-56f544cf4804/o/page-1-uuid.jpg"
        );
    }
}
