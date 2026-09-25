use std::error::Error;

use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::backend::manga_provider::{
    Chapter, ChapterPageUrl, ChapterReader, Genres, Languages, Manga, MangaStatus, PopularManga, RecentlyAddedManga,
};

pub const MANGAONI_BASE_URL: &str = "https://manga-oni.com";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MangaoniSearchResponse {
    #[serde(default)]
    pub mangas: Vec<MangaoniSearchItem>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MangaoniSearchItem {
    pub nombre: String,
    pub slug: String,
    pub autor: Option<String>,
    pub url: Option<String>,
    pub img: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MangaoniChapterItem {
    pub id: String,
    pub number: f64,
    pub number_str: String,
    pub title: String,
    pub publication_date: Option<chrono::NaiveDate>,
}

impl MangaoniChapterItem {
    pub fn to_chapter(&self, manga_id: &str) -> Chapter {
        Chapter {
            id: self.id.clone(),
            id_safe_for_download: self.id.replace('|', "_").replace('/', "_"),
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

/// Simple, robust pure-Rust Base64 decoder
pub fn decode_base64(input: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut map = [0xffu8; 256];
    for (i, &b) in TABLE.iter().enumerate() {
        map[b as usize] = i as u8;
    }

    let trimmed = input.trim();
    let mut buf = 0u32;
    let mut bits = 0u32;
    let mut output = Vec::with_capacity((trimmed.len() * 3) / 4);

    for &b in trimmed.as_bytes() {
        if b == b'=' {
            break;
        }
        let val = map[b as usize];
        if val == 0xff {
            continue;
        }
        buf = (buf << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push(((buf >> bits) & 0xff) as u8);
        }
    }

    Some(output)
}

/// Parses the `var unicap = '...';` variable in reader HTML into chapter page URLs.
pub fn parse_unicap(html: &str) -> Result<Vec<ChapterPageUrl>, Box<dyn Error>> {
    // Regex or simple string search for `var unicap = '...'`
    let start_needle = "var unicap = '";
    let b64_str = if let Some(start_idx) = html.find(start_needle) {
        let rest = &html[start_idx + start_needle.len()..];
        if let Some(end_idx) = rest.find('\'') {
            &rest[..end_idx]
        } else {
            return Err("Unterminated unicap string".into());
        }
    } else {
        return Err("Could not find unicap in reader HTML".into());
    };

    let decoded_bytes = decode_base64(b64_str).ok_or("Failed to decode base64 unicap")?;
    let decoded_str = String::from_utf8(decoded_bytes)?;

    let parts: Vec<&str> = decoded_str.split("||").collect();
    if parts.is_empty() {
        return Err("Invalid unicap format".into());
    }

    let base_url = parts[0];
    let pages_json = parts.get(1).copied().unwrap_or("[]");
    let filenames: Vec<String> = serde_json::from_str(pages_json)?;

    let mut pages = Vec::with_capacity(filenames.len());
    for file in filenames {
        let full_url = format!("{base_url}{file}");
        if let Ok(parsed_url) = full_url.parse::<Url>() {
            let extension = parsed_url.path().rsplit('.').next().unwrap_or("webp").to_string();

            pages.push(ChapterPageUrl {
                url: parsed_url,
                extension,
            });
        }
    }

    Ok(pages)
}

/// Details parsed from a manga page
#[derive(Debug, Clone)]
pub struct MangaoniMangaDetails {
    pub title: String,
    pub cover_img_url: String,
    pub description: String,
    pub status: MangaStatus,
    pub genres: Vec<Genres>,
    pub chapters: Vec<MangaoniChapterItem>,
}

impl From<MangaoniMangaDetails> for Manga {
    fn from(value: MangaoniMangaDetails) -> Self {
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

/// Parses a MangaOni manga details page
pub fn parse_manga_details(html: &str, slug: &str) -> MangaoniMangaDetails {
    // Title from <h1 ...> or og:title
    let title = if let Some(idx) = html.find("<h1") {
        let rest = &html[idx..];
        if let Some(end) = rest.find("</h1>") {
            let h1_content = &rest[..end];
            scraper_clean_text(h1_content)
        } else {
            slug.replace('-', " ")
        }
    } else {
        slug.replace('-', " ")
    };

    // Cover from og:image or <img ... archivos/mangas/...>
    let cover_img_url = extract_meta_tag(html, "og:image")
        .or_else(|| {
            regex::Regex::new(r#"https?://oni\.ntr-files\.online/public/archivos/mangas/[^"'\s]+"#)
                .ok()?
                .find(html)
                .map(|m| m.as_str().to_string())
        })
        .unwrap_or_default();

    // Description from og:description or description meta
    let description = extract_meta_tag(html, "og:description")
        .or_else(|| extract_meta_tag(html, "description"))
        .unwrap_or_default();

    // Status: defaults to Ongoing
    let status = if html.contains("Finalizado") || html.contains("completed") {
        MangaStatus::Completed
    } else if html.contains("Pausa") || html.contains("hiatus") {
        MangaStatus::Hiatus
    } else {
        MangaStatus::Ongoing
    };

    // Chapters from <div id="c_list"> or links with `/lector/<slug>/<chapter_id>/`
    let mut chapters = Vec::new();
    let chap_pattern = format!(r#"href="https://manga-oni\.com/lector/{slug}/([^"/]+)/"#);
    if let Ok(re) = regex::Regex::new(&chap_pattern) {
        let mut seen = std::collections::HashSet::new();

        // Search each <a> block for data-num and title
        let entry_re =
            regex::Regex::new(r#"<a\s+href="https://manga-oni\.com/lector/[^/]+/([^"/]+)/"[^>]*>([\s\S]*?)</a>"#).unwrap();
        for cap in entry_re.captures_iter(html) {
            let chapter_id = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            if chapter_id.is_empty() || seen.contains(chapter_id) {
                continue;
            }
            seen.insert(chapter_id.to_string());

            let inner = cap.get(2).map(|m| m.as_str()).unwrap_or("");

            // Extract data-num="1193"
            let num = if let Some(m) = regex::Regex::new(r#"data-num="([^"]+)""#).unwrap().captures(inner) {
                m.get(1).map(|v| v.as_str().parse::<f64>().unwrap_or(0.0)).unwrap_or(0.0)
            } else if let Some(m) = regex::Regex::new(r#"(?i)cap[íi]tulo\s*([\d\.]+)"#).unwrap().captures(inner) {
                m.get(1).map(|v| v.as_str().parse::<f64>().unwrap_or(0.0)).unwrap_or(0.0)
            } else {
                0.0
            };

            // Extract title from <h3 class="entry-title-h2">...</h3>
            let title = if let Some(m) = regex::Regex::new(r#"<h3[^>]*>([\s\S]*?)</h3>"#).unwrap().captures(inner) {
                scraper_clean_text(m.get(1).map(|v| v.as_str()).unwrap_or(""))
            } else {
                format!("Capítulo {num}")
            };

            // Extract date from datetime="2026-09-10 20:28:04"
            let publication_date = if let Some(m) = regex::Regex::new(r#"datetime="([^"]+)""#).unwrap().captures(inner) {
                let raw_date = m.get(1).map(|v| v.as_str()).unwrap_or("");
                let date_part = raw_date.split(' ').next().unwrap_or(raw_date);
                chrono::NaiveDate::parse_from_str(date_part, "%Y-%m-%d").ok()
            } else {
                None
            };

            chapters.push(MangaoniChapterItem {
                id: format!("{slug}|{chapter_id}"),
                number: num,
                number_str: num.to_string(),
                title,
                publication_date,
            });
        }
    }

    MangaoniMangaDetails {
        title,
        cover_img_url,
        description,
        status,
        genres: Vec::new(),
        chapters,
    }
}

/// Parses Top manga from homepage into PopularManga
pub fn parse_home_popular(html: &str) -> Vec<PopularManga> {
    let mut results = Vec::new();
    let re = regex::Regex::new(
        r#"<div class="media-left">[\s\S]*?<img[^>]+src="([^"]+)"[\s\S]*?alt="([^"]+)"[\s\S]*?<h2 class="media-heading"><a href="https://manga-oni\.com/(?:manga|manhua|manhwa)/([^"/]+)/""#,
    );

    if let Ok(re) = re {
        let mut seen = std::collections::HashSet::new();
        for cap in re.captures_iter(html) {
            let cover_img_url = cap.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
            let title = cap.get(2).map(|m| m.as_str()).unwrap_or("").to_string();
            let slug = cap.get(3).map(|m| m.as_str()).unwrap_or("").to_string();

            if slug.is_empty() || seen.contains(&slug) {
                continue;
            }
            seen.insert(slug.clone());

            results.push(PopularManga {
                id: slug,
                title,
                genres: Vec::new(),
                description: String::new(),
                status: Some(MangaStatus::Ongoing),
                cover_img_url,
            });
        }
    }
    results
}

/// Parses Actualizado from homepage into RecentlyAddedManga
pub fn parse_home_recent(html: &str) -> Vec<RecentlyAddedManga> {
    let mut results = Vec::new();
    let re = regex::Regex::new(
        r#"<a href="https://manga-oni\.com/(?:manga|manhua|manhwa)/([^"/]+)/"[\s\S]*?<img[^>]+data-src="([^"]+)"[\s\S]*?data-test="latest-update-name"[^>]*>([^<]+)</a>"#,
    );

    if let Ok(re) = re {
        let mut seen = std::collections::HashSet::new();
        for cap in re.captures_iter(html) {
            let slug = cap.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
            let cover_img_url = cap.get(2).map(|m| m.as_str()).unwrap_or("").to_string();
            let title = cap.get(3).map(|m| m.as_str().trim()).unwrap_or("").to_string();

            if slug.is_empty() || seen.contains(&slug) {
                continue;
            }
            seen.insert(slug.clone());

            results.push(RecentlyAddedManga {
                id: slug,
                title,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_base64() {
        let encoded = "aHR0cHM6Ly9vbmkubnRyLWZpbGVzLm9ubGluZS9wdWJsaWMvfHxbIjAwMS53ZWJwIl0=";
        let decoded = String::from_utf8(decode_base64(encoded).unwrap()).unwrap();
        assert_eq!(decoded, "https://oni.ntr-files.online/public/||[\"001.webp\"]");
    }

    #[test]
    fn test_parse_unicap() {
        let html = r#"<script>var unicap = 'aHR0cHM6Ly9vbmkubnRyLWZpbGVzLm9ubGluZS9wdWJsaWMvfHxbIjAwMS53ZWJwIl0=';</script>"#;
        let pages = parse_unicap(html).unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].url.as_str(), "https://oni.ntr-files.online/public/001.webp");
        assert_eq!(pages[0].extension, "webp");
    }
}
