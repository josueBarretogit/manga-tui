use std::error::Error;

use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::backend::manga_provider::{
    Chapter, ChapterPageUrl, ChapterReader, Genres, Languages, Manga, MangaStatus, PopularManga, RecentlyAddedManga,
};

pub const LEERCAPITULO_BASE_URL: &str = "https://www.leercapitulo.co";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LeercapituloSearchItem {
    pub name: String,
    #[serde(rename = "type")]
    pub manga_type: Option<String>,
    pub status: Option<String>,
    pub cover_uri: Option<String>,
    pub uri: String,
    pub last_chapter_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeercapituloChapterItem {
    pub id: String,
    pub number: f64,
    pub number_str: String,
    pub title: String,
    pub publication_date: Option<chrono::NaiveDate>,
}

impl LeercapituloChapterItem {
    pub fn to_chapter(&self, manga_id: &str) -> Chapter {
        Chapter {
            id: self.id.clone(),
            id_safe_for_download: self.id.replace('/', "_"),
            manga_id: manga_id.to_string(),
            title: self.title.clone(),
            language: Languages::Spanish,
            chapter_number: self.number_str.clone(),
            volume_number: None,
            scanlator: None,
            publication_date: self.publication_date,
        }
    }

    pub fn to_chapter_reader(&self) -> ChapterReader {
        ChapterReader {
            id: self.id.clone(),
            number: self.number_str.clone(),
            volume: "none".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LeercapituloMangaDetails {
    pub title: String,
    pub cover_img_url: String,
    pub description: String,
    pub status: MangaStatus,
    pub genres: Vec<Genres>,
    pub chapters: Vec<LeercapituloChapterItem>,
}

impl From<LeercapituloMangaDetails> for Manga {
    fn from(value: LeercapituloMangaDetails) -> Self {
        Self {
            id: String::new(),
            id_safe_for_download: String::new(),
            title: value.title,
            description: value.description,
            status: value.status,
            cover_img_url: value.cover_img_url,
            genres: value.genres,
            languages: vec![Languages::Spanish],
            rating: String::new(),
            artist: None,
            author: None,
        }
    }
}

/// Parses the reader page HTML to extract image page URLs from `data-src`
pub fn parse_reader_pages(html: &str) -> Result<Vec<ChapterPageUrl>, Box<dyn Error>> {
    let re = regex::Regex::new(r#"<img[^>]+data-src="([^"]+)"[^>]*>"#)?;
    let mut pages = Vec::new();

    for cap in re.captures_iter(html) {
        if let Some(src) = cap.get(1) {
            let img_url_str = src.as_str().trim();
            if img_url_str.is_empty() {
                continue;
            }

            let full_url = if img_url_str.starts_with("http://") || img_url_str.starts_with("https://") {
                img_url_str.to_string()
            } else if img_url_str.starts_with('/') {
                format!("{LEERCAPITULO_BASE_URL}{img_url_str}")
            } else {
                format!("{LEERCAPITULO_BASE_URL}/{img_url_str}")
            };

            if let Ok(url) = full_url.parse::<Url>() {
                let extension = url.path().rsplit('.').next().unwrap_or("jpg").to_string();

                pages.push(ChapterPageUrl { url, extension });
            }
        }
    }

    if pages.is_empty() {
        return Err("No chapter pages found on LeerCapitulo reader page".into());
    }

    Ok(pages)
}

/// Parses a manga page on LeerCapitulo
pub fn parse_manga_details(html: &str, manga_id: &str) -> LeercapituloMangaDetails {
    // Title from <h1 ...>
    let title = if let Some(m) = regex::Regex::new(r#"<h1[^>]*>([\s\S]*?)</h1>"#).unwrap().captures(html) {
        scraper_clean_text(m.get(1).map(|v| v.as_str()).unwrap_or(""))
    } else {
        manga_id.split('/').nth(1).unwrap_or(manga_id).replace('-', " ")
    };

    // Cover from og:image or first cover image
    let cover_img_url = extract_meta_tag(html, "og:image")
        .or_else(|| {
            regex::Regex::new(r#"/covers/[a-zA-Z0-9/_\-\.]+\.(?:jpg|png|webp)"#)
                .ok()?
                .find(html)
                .map(|m| format!("{LEERCAPITULO_BASE_URL}{}", m.as_str()))
        })
        .unwrap_or_default();

    // Description from meta
    let description = extract_meta_tag(html, "og:description")
        .or_else(|| extract_meta_tag(html, "description"))
        .unwrap_or_default();

    let status = if html.contains("Finalizado") || html.contains("Completed") {
        MangaStatus::Completed
    } else if html.contains("Pausa") || html.contains("Hiatus") {
        MangaStatus::Hiatus
    } else {
        MangaStatus::Ongoing
    };

    // Chapters from <div id="chapterList">
    let mut chapters = Vec::new();
    let row_re = regex::Regex::new(
        r#"<a[^>]+class="lc-chapter-row"[^>]+href="(/leer/[^"]+)"[^>]*>[\s\S]*?<span class="n">([^<]+)</span>[\s\S]*?<span class="d">([^<]+)</span>[\s\S]*?</a>"#
    ).unwrap();

    let mut seen = std::collections::HashSet::new();
    for cap in row_re.captures_iter(html) {
        let chapter_path = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let chapter_name = cap.get(2).map(|m| m.as_str().trim()).unwrap_or("");
        let chapter_date = cap.get(3).map(|m| m.as_str().trim()).unwrap_or("");

        if chapter_path.is_empty() || seen.contains(chapter_path) {
            continue;
        }
        seen.insert(chapter_path.to_string());

        // Extract chapter number from chapter_name (e.g. "Capitulo 1194" -> 1194.0)
        let num = if let Some(m) = regex::Regex::new(r#"[\d\.]+"#).unwrap().find(chapter_name) {
            m.as_str().parse::<f64>().unwrap_or(0.0)
        } else {
            0.0
        };

        let date_part = chapter_date.split(' ').next().unwrap_or(chapter_date);
        let publication_date = chrono::NaiveDate::parse_from_str(date_part, "%Y-%m-%d").ok();

        chapters.push(LeercapituloChapterItem {
            id: chapter_path.to_string(),
            number: num,
            number_str: num.to_string(),
            title: chapter_name.to_string(),
            publication_date,
        });
    }

    LeercapituloMangaDetails {
        title,
        cover_img_url,
        description,
        status,
        genres: Vec::new(),
        chapters,
    }
}

/// Parses Populares on homepage
pub fn parse_home_popular(html: &str) -> Vec<PopularManga> {
    let mut results = Vec::new();
    let re = regex::Regex::new(
        r#"<a class="lc-side-item" href="(/manga/[^"]+)">[\s\S]*?<img src="([^"]+)"[^>]*alt="([^"]*)"[\s\S]*?<span class="lc-side-name">([^<]+)</span>"#,
    );

    if let Ok(re) = re {
        let mut seen = std::collections::HashSet::new();
        for cap in re.captures_iter(html) {
            let uri = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let cover_uri = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let name = cap.get(4).map(|m| m.as_str().trim()).unwrap_or("");

            let manga_id = uri.trim_matches('/').strip_prefix("manga/").unwrap_or(uri).to_string();

            if manga_id.is_empty() || seen.contains(&manga_id) {
                continue;
            }
            seen.insert(manga_id.clone());

            let cover_img_url =
                if cover_uri.starts_with("http") { cover_uri.to_string() } else { format!("{LEERCAPITULO_BASE_URL}{cover_uri}") };

            results.push(PopularManga {
                id: manga_id,
                title: name.to_string(),
                genres: Vec::new(),
                description: String::new(),
                status: Some(MangaStatus::Ongoing),
                cover_img_url,
            });
        }
    }

    results
}

/// Parses Ultimos mangas on homepage
pub fn parse_home_recent(html: &str) -> Vec<RecentlyAddedManga> {
    let mut results = Vec::new();
    let re = regex::Regex::new(
        r#"<a class="lc-side-item" href="(/manga/[^"]+)">[\s\S]*?<img src="([^"]+)"[^>]*alt="([^"]*)"[\s\S]*?<span class="lc-side-name">([^<]+)</span>"#,
    );

    if let Ok(re) = re {
        let mut seen = std::collections::HashSet::new();
        for cap in re.captures_iter(html) {
            let uri = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            let cover_uri = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let name = cap.get(4).map(|m| m.as_str().trim()).unwrap_or("");

            let manga_id = uri.trim_matches('/').strip_prefix("manga/").unwrap_or(uri).to_string();

            if manga_id.is_empty() || seen.contains(&manga_id) {
                continue;
            }
            seen.insert(manga_id.clone());

            let cover_img_url =
                if cover_uri.starts_with("http") { cover_uri.to_string() } else { format!("{LEERCAPITULO_BASE_URL}{cover_uri}") };

            results.push(RecentlyAddedManga {
                id: manga_id,
                title: name.to_string(),
                description: String::new(),
                cover_img_url,
            });
        }
    }

    results
}

fn extract_meta_tag(html: &str, tag_name: &str) -> Option<String> {
    let p1 = format!(r#"<meta[^>]+(?:name|property)="[^"]*{tag_name}[^"]*"[^>]+content="([^"]+)""#);
    if let Ok(re) = regex::Regex::new(&p1) {
        if let Some(c) = re.captures(html) {
            return c.get(1).map(|m| m.as_str().to_string());
        }
    }
    let p2 = format!(r#"<meta[^>]+content="([^"]+)"[^>]+(?:name|property)="[^"]*{tag_name}[^"]*""#);
    if let Ok(re) = regex::Regex::new(&p2) {
        if let Some(c) = re.captures(html) {
            return c.get(1).map(|m| m.as_str().to_string());
        }
    }
    None
}

fn scraper_clean_text(html: &str) -> String {
    let tag_re = regex::Regex::new(r"<[^>]+>").unwrap();
    let stripped = tag_re.replace_all(html, "");
    stripped.split_whitespace().collect::<Vec<_>>().join(" ")
}
