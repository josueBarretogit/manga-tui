use std::error::Error;
use std::path::Path;

use chrono::NaiveDate;
use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::backend::manga_provider::{
    Artist, Author, Chapter, ChapterPageUrl, ChapterReader, Genres, Languages, Manga, MangaStatus, PopularManga, Rating,
    RecentlyAddedManga, SearchManga,
};

pub const COMIZY_BASE_URL: &str = "https://comizy.io";
pub const COMIZY_API_URL: &str = "https://api.comizy.io";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComizyApiResponse<T> {
    pub success: Option<bool>,
    pub data: Option<T>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComizyHomeData {
    #[serde(default)]
    pub popular: Vec<ComizyTitleItem>,
    pub latest: Option<ComizyLatestSection>,
    #[serde(default)]
    pub trending: Vec<ComizyTitleItem>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComizyLatestSection {
    #[serde(default)]
    pub items: Vec<ComizyTitleItem>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComizySearchData {
    #[serde(default)]
    pub items: Vec<ComizyTitleItem>,
    pub pagination: Option<ComizyPagination>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComizyPagination {
    pub total: Option<u32>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
    pub total_pages: Option<u32>,
    pub has_next: Option<bool>,
    pub has_previous: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComizyTitleItem {
    pub id: String,
    pub name: String,
    pub slug: Option<String>,
    pub cover: Option<String>,
    pub summary: Option<String>,
    pub status: Option<String>,
    pub rating: Option<serde_json::Value>,
    #[serde(default)]
    pub genres: Vec<ComizyGenre>,
    #[serde(default)]
    pub latest_chapters: Vec<ComizyChapterSummary>,
}

impl ComizyTitleItem {
    pub fn to_popular_manga(self) -> PopularManga {
        let status = self.status.as_deref().map(parse_status);
        let genres = self.genres.into_iter().map(Into::into).collect();
        PopularManga {
            id: self.id,
            title: self.name,
            genres,
            description: self.summary.unwrap_or_default(),
            status,
            cover_img_url: self.cover.unwrap_or_default(),
        }
    }

    pub fn to_recently_added_manga(self) -> RecentlyAddedManga {
        RecentlyAddedManga {
            id: self.id,
            title: self.name,
            description: self.summary.unwrap_or_default(),
            cover_img_url: self.cover.unwrap_or_default(),
        }
    }

    pub fn to_search_manga(self) -> SearchManga {
        let status = self.status.as_deref().map(parse_status);
        let genres = self.genres.into_iter().map(Into::into).collect();
        SearchManga {
            id: self.id,
            title: self.name,
            genres,
            description: self.summary,
            status,
            cover_img_url: self.cover.unwrap_or_default(),
            languages: vec![Languages::English],
            artist: None,
            author: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComizyChapterSummary {
    pub id: String,
    pub name: String,
    pub slug: Option<String>,
    pub number: Option<serde_json::Value>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComizyGenre {
    pub name: String,
    pub slug: Option<String>,
}

impl From<ComizyGenre> for Genres {
    fn from(value: ComizyGenre) -> Self {
        let rating = match value.name.to_lowercase().as_str() {
            "adult" | "smut" | "erotica" | "hentai" | "gore" => Rating::Nsfw,
            "ecchi" | "suggestive" | "mature" => Rating::Moderate,
            "doujinshi" => Rating::Doujinshi,
            _ => Rating::Normal,
        };
        Genres::new(value.name, rating)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComizyTitleDetailsData {
    pub title: ComizyTitleDetails,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComizyTitleDetails {
    pub id: String,
    pub name: String,
    pub slug: Option<String>,
    pub cover: Option<String>,
    pub summary: Option<String>,
    pub status: Option<String>,
    #[serde(default)]
    pub genres: Vec<ComizyGenre>,
    pub authors: Option<Vec<ComizyPerson>>,
    pub artists: Option<Vec<ComizyPerson>>,
    #[serde(default)]
    pub chapters: Vec<ComizyChapterItem>,
}

impl ComizyTitleDetails {
    pub fn to_manga(self) -> Manga {
        let status = parse_status(self.status.as_deref().unwrap_or("ongoing"));
        let genres = self.genres.into_iter().map(Into::into).collect();
        let author = self.authors.as_ref().and_then(|a| a.first()).map(|p| Author {
            id: p.slug.clone().unwrap_or_default(),
            name: p.name.clone(),
        });
        let artist = self.artists.as_ref().and_then(|a| a.first()).map(|p| Artist {
            id: p.slug.clone().unwrap_or_default(),
            name: p.name.clone(),
        });

        let id_safe_for_download = self.slug.clone().unwrap_or_else(|| self.id.clone());

        Manga {
            id: self.id,
            id_safe_for_download,
            title: self.name,
            description: self.summary.unwrap_or_default(),
            status,
            cover_img_url: self.cover.unwrap_or_default(),
            genres,
            languages: vec![Languages::English],
            rating: String::new(),
            artist,
            author,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComizyPerson {
    pub name: String,
    pub slug: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComizyChaptersData {
    #[serde(default)]
    pub chapters: Vec<ComizyChapterItem>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComizyChapterItem {
    pub id: String,
    pub name: String,
    pub slug: Option<String>,
    pub number: Option<serde_json::Value>,
    pub updated_at: Option<String>,
    pub created_at: Option<String>,
    pub url: Option<String>,
}

impl ComizyChapterItem {
    pub fn to_chapter(&self, manga_id: &str) -> Chapter {
        let (number, number_str) = parse_chapter_number(&self.name, self.slug.as_deref(), self.number.as_ref());
        let compound_id = format!("{manga_id}|{}", self.id);
        let id_safe = format!("{manga_id}_{}", self.id);

        Chapter {
            id: compound_id,
            id_safe_for_download: id_safe,
            manga_id: manga_id.to_string(),
            title: self.name.clone(),
            language: Languages::English,
            chapter_number: number_str,
            volume_number: None,
            scanlator: None,
            publication_date: parse_publication_date(self.updated_at.as_deref().or(self.created_at.as_deref())),
        }
    }

    pub fn to_chapter_reader(&self, manga_id: &str) -> ChapterReader {
        let (_number, number_str) = parse_chapter_number(&self.name, self.slug.as_deref(), self.number.as_ref());
        let compound_id = format!("{manga_id}|{}", self.id);
        ChapterReader {
            id: compound_id,
            number: number_str,
            volume: "none".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComizyImagesData {
    #[serde(default)]
    pub images: Vec<String>,
}

/// Parses status string into `MangaStatus`
pub fn parse_status(status_str: &str) -> MangaStatus {
    match status_str.to_lowercase().as_str() {
        "completed" => MangaStatus::Completed,
        "hiatus" => MangaStatus::Hiatus,
        "cancelled" => MangaStatus::Cancelled,
        _ => MangaStatus::Ongoing,
    }
}

/// Extracts chapter number as `(f64, String)`
pub fn parse_chapter_number(name: &str, slug: Option<&str>, number_val: Option<&serde_json::Value>) -> (f64, String) {
    let re_chapter = regex::Regex::new(r#"(?i)chapter\s*([\d\.]+)"#).ok();
    if let Some(ref re) = re_chapter {
        if let Some(caps) = re.captures(name) {
            if let Some(m) = caps.get(1) {
                if let Ok(num) = m.as_str().parse::<f64>() {
                    let num_str = if num.fract() == 0.0 { format!("{:.0}", num) } else { format!("{num}") };
                    return (num, num_str);
                }
            }
        }
    }

    if let Some(slug_str) = slug {
        let re_slug = regex::Regex::new(r#"(?i)chapter-([\d]+(?:\.[\d]+)?)"#).ok();
        if let Some(ref re) = re_slug {
            if let Some(caps) = re.captures(slug_str) {
                if let Some(m) = caps.get(1) {
                    if let Ok(num) = m.as_str().parse::<f64>() {
                        let num_str = if num.fract() == 0.0 { format!("{:.0}", num) } else { format!("{num}") };
                        return (num, num_str);
                    }
                }
            }
        }

        let re_slug_sub = regex::Regex::new(r#"(?i)chapter-([\d]+)-([\d]+)"#).ok();
        if let Some(ref re) = re_slug_sub {
            if let Some(caps) = re.captures(slug_str) {
                if let (Some(m1), Some(m2)) = (caps.get(1), caps.get(2)) {
                    let combined = format!("{}.{}", m1.as_str(), m2.as_str());
                    if let Ok(num) = combined.parse::<f64>() {
                        return (num, combined);
                    }
                }
            }
        }
    }

    if let Some(val) = number_val {
        if let Some(num) = val.as_f64() {
            let num_str = if num.fract() == 0.0 { format!("{:.0}", num) } else { format!("{num}") };
            return (num, num_str);
        } else if let Some(num_i) = val.as_i64() {
            return (num_i as f64, num_i.to_string());
        }
    }

    (0.0, "0".to_string())
}

/// Parses date strings like `"2026-09-25T21:40:12.000Z"` into `NaiveDate`
pub fn parse_publication_date(s: Option<&str>) -> Option<NaiveDate> {
    let date_str = s?.split('T').next()?;
    NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()
}

/// Converts chapter image URLs into `ChapterPageUrl`
pub fn parse_images_to_page_urls(images: &[String]) -> Result<Vec<ChapterPageUrl>, Box<dyn Error>> {
    let mut pages = Vec::with_capacity(images.len());
    for img in images {
        let trimmed = img.trim();
        if trimmed.is_empty() {
            continue;
        }
        let url = Url::parse(trimmed)?;
        let extension = Path::new(url.path())
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("webp")
            .to_string();

        pages.push(ChapterPageUrl { url, extension });
    }
    Ok(pages)
}

/// Extracts title ID and slug from Next.js HTML `__NEXT_DATA__`
pub fn extract_id_from_next_data(html: &str) -> Option<String> {
    let re = regex::Regex::new(r#"<script id="__NEXT_DATA__"[^>]*>(.*?)</script>"#).ok()?;
    let caps = re.captures(html)?;
    let json_str = caps.get(1)?.as_str();

    let val: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let id = val.pointer("/props/pageProps/initialManga/id")?.as_str()?;
    Some(id.to_string())
}
