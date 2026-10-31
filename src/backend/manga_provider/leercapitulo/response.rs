use std::error::Error;

use reqwest::Url;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};

use crate::backend::manga_provider::{
    Artist, Author, Chapter, ChapterPageUrl, ChapterReader, Genres, Languages, Manga, MangaStatus, PopularManga, Rating,
    RecentlyAddedManga,
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
    pub author: Option<Author>,
    pub artist: Option<Artist>,
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
            artist: value.artist,
            author: value.author,
            provider: Some(crate::backend::manga_provider::MangaProviders::Leercapitulo),
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
    let document = Html::parse_document(html);

    // Title from article.lc-panel h1 or generic h1
    let title = {
        let title_sel = Selector::parse("article.lc-panel h1, .lc-panel h1, h1").ok();
        title_sel
            .and_then(|sel| document.select(&sel).next())
            .map(|el| el.text().collect::<Vec<_>>().join(" ").trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| manga_id.split('/').nth(1).unwrap_or(manga_id).replace('-', " "))
    };

    // Cover from the specific manga cover container (.lc-cover-lg img)
    // Avoids picking up the first cover image on the page, which belongs to popular manga in the sidebar!
    let cover_img_url = {
        let cover_sel = Selector::parse(".lc-cover-lg img, article.lc-panel .lc-cover-lg img, div.lc-cover-lg img").ok();
        let from_selector = cover_sel.and_then(|sel| {
            document.select(&sel).next().and_then(|el| {
                el.value().attr("src").or_else(|| el.value().attr("data-src")).map(|src| {
                    let s = src.trim();
                    if s.starts_with("http://") || s.starts_with("https://") {
                        s.to_string()
                    } else if s.starts_with('/') {
                        format!("{LEERCAPITULO_BASE_URL}{s}")
                    } else {
                        format!("{LEERCAPITULO_BASE_URL}/{s}")
                    }
                })
            })
        });

        from_selector
            .or_else(|| {
                // Fallback: regex strictly inside the .lc-cover-lg container
                let re = regex::Regex::new(r#"class="[^"]*lc-cover-lg[^"]*"[\s\S]*?<img[^>]+(?:src|data-src)="([^"]+)""#).ok()?;
                let cap = re.captures(html)?;
                let src = cap.get(1)?.as_str().trim();
                if src.starts_with("http://") || src.starts_with("https://") {
                    Some(src.to_string())
                } else if src.starts_with('/') {
                    Some(format!("{LEERCAPITULO_BASE_URL}{src}"))
                } else {
                    Some(format!("{LEERCAPITULO_BASE_URL}/{src}"))
                }
            })
            .or_else(|| {
                // Fallback: search within article.lc-panel only (never touches sidebar aside)
                let panel_sel = Selector::parse("article.lc-panel img").ok()?;
                let el = document.select(&panel_sel).next()?;
                let src = el.value().attr("src").or_else(|| el.value().attr("data-src"))?.trim();
                if src.starts_with("http://") || src.starts_with("https://") {
                    Some(src.to_string())
                } else if src.starts_with('/') {
                    Some(format!("{LEERCAPITULO_BASE_URL}{src}"))
                } else {
                    Some(format!("{LEERCAPITULO_BASE_URL}/{src}"))
                }
            })
            .or_else(|| {
                // Secondary fallback: og:image meta tag
                extract_meta_tag(html, "og:image")
            })
            .unwrap_or_default()
    };

    // Description from synopsis section, fallback to meta tag
    let description = {
        let sinopsis_sel = Selector::parse("#sinopsis .p-3 p, #sinopsis p, section#sinopsis p").ok();
        let from_section = sinopsis_sel.and_then(|sel| {
            document
                .select(&sel)
                .next()
                .map(|el| el.text().collect::<Vec<_>>().join(" ").trim().to_string())
        });

        match from_section {
            Some(desc) if !desc.is_empty() && !desc.to_lowercase().contains("no tiene sinopsis") => desc,
            _ => extract_meta_tag(html, "og:description")
                .or_else(|| extract_meta_tag(html, "description"))
                .unwrap_or_default(),
        }
    };

    // Status from .lc-facts
    let status = {
        let facts_sel = Selector::parse(".lc-facts li").ok();
        let mut found_status = None;
        if let Some(sel) = facts_sel {
            for li in document.select(&sel) {
                let text = li.text().collect::<Vec<_>>().join(" ");
                if text.contains("Estado") {
                    if text.contains("Completed") || text.contains("Finalizado") {
                        found_status = Some(MangaStatus::Completed);
                    } else if text.contains("Hiatus") || text.contains("Pausa") || text.contains("Paused") {
                        found_status = Some(MangaStatus::Hiatus);
                    } else if text.contains("Cancelled") || text.contains("Cancelado") {
                        found_status = Some(MangaStatus::Cancelled);
                    } else {
                        found_status = Some(MangaStatus::Ongoing);
                    }
                    break;
                }
            }
        }
        found_status.unwrap_or_else(|| {
            if html.contains("Finalizado") || html.contains("Completed") {
                MangaStatus::Completed
            } else if html.contains("Pausa") || html.contains("Hiatus") {
                MangaStatus::Hiatus
            } else {
                MangaStatus::Ongoing
            }
        })
    };

    // Author and Artist from .lc-facts
    let mut author = None;
    let mut artist = None;
    if let Ok(facts_sel) = Selector::parse(".lc-facts li") {
        for li in document.select(&facts_sel) {
            let k_sel = Selector::parse(".k").ok();
            let key = k_sel
                .as_ref()
                .and_then(|k| li.select(k).next())
                .map(|el| el.text().collect::<String>().trim().to_lowercase());
            if let Some(key) = key {
                if key.contains("autor") {
                    let full = li.text().collect::<Vec<_>>().join(" ");
                    let val = full.replace("Autor", "").trim().to_string();
                    if !val.is_empty() {
                        author = Some(Author {
                            id: val.clone(),
                            name: val,
                        });
                    }
                } else if key.contains("dibujo") || key.contains("artista") {
                    let full = li.text().collect::<Vec<_>>().join(" ");
                    let val = full.replace("Dibujo", "").replace("Artista", "").trim().to_string();
                    if !val.is_empty() {
                        artist = Some(Artist {
                            id: val.clone(),
                            name: val,
                        });
                    }
                }
            }
        }
    }

    // Genres from badge links
    let mut genres = Vec::new();
    if let Ok(badge_sel) = Selector::parse("a.badge[href*='genre='], a.badge[href*='theme=']") {
        for badge in document.select(&badge_sel) {
            let name = badge.text().collect::<String>().trim().to_string();
            if !name.is_empty() {
                let rating = match name.to_lowercase().as_str() {
                    "ecchi" => Rating::Moderate,
                    "hentai" | "smut" | "gore" => Rating::Nsfw,
                    _ => Rating::Normal,
                };
                genres.push(Genres::new(name, rating));
            }
        }
    }

    // Chapters from <div id="chapterList">
    let mut chapters = Vec::new();
    let row_re = regex::Regex::new(
        r#"<a[^>]+class="lc-chapter-row"[^>]+href="(/leer/[^"]+)"[^>]*>[\s\S]*?<span class="n">([^<]+)</span>[\s\S]*?<span class="d">([^<]+)</span>[\s\S]*?</a>"#,
    )
    .unwrap();

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
        genres,
        author,
        artist,
        chapters,
    }
}

/// Parses Populares on homepage
pub fn parse_home_popular(html: &str) -> Vec<PopularManga> {
    let document = Html::parse_document(html);
    let mut results = Vec::new();
    let section_sel = Selector::parse("section.lc-panel").unwrap();
    let h2_sel = Selector::parse("h2.lc-panel-title").unwrap();
    let item_sel = Selector::parse("a.lc-side-item").unwrap();
    let img_sel = Selector::parse("img").unwrap();
    let name_sel = Selector::parse(".lc-side-name").unwrap();

    let mut seen = std::collections::HashSet::new();

    for section in document.select(&section_sel) {
        let is_popular = section
            .select(&h2_sel)
            .next()
            .map(|h| h.text().collect::<String>().contains("Populares"))
            .unwrap_or(false);
        if !is_popular {
            continue;
        }

        for item in section.select(&item_sel) {
            let uri = item.value().attr("href").unwrap_or("");
            let manga_id = uri.trim_matches('/').strip_prefix("manga/").unwrap_or(uri).to_string();
            if manga_id.is_empty() || seen.contains(&manga_id) {
                continue;
            }
            seen.insert(manga_id.clone());

            let cover_uri = item.select(&img_sel).next().and_then(|img| img.value().attr("src")).unwrap_or("");
            let name = item
                .select(&name_sel)
                .next()
                .map(|n| n.text().collect::<String>().trim().to_string())
                .unwrap_or_default();

            let cover_img_url = if cover_uri.starts_with("http") {
                cover_uri.to_string()
            } else if cover_uri.starts_with('/') {
                format!("{LEERCAPITULO_BASE_URL}{cover_uri}")
            } else {
                format!("{LEERCAPITULO_BASE_URL}/{cover_uri}")
            };

            results.push(PopularManga {
                id: manga_id,
                title: name,
                genres: Vec::new(),
                description: String::new(),
                status: Some(MangaStatus::Ongoing),
                cover_img_url,
            });
        }
    }

    // Fallback if section selector didn't match (e.g. site layout changes)
    if results.is_empty() {
        let re = regex::Regex::new(
            r#"<a class="lc-side-item" href="(/manga/[^"]+)">[\s\S]*?<img src="([^"]+)"[^>]*alt="([^"]*)"[\s\S]*?<span class="lc-side-name">([^<]+)</span>"#,
        )
        .unwrap();

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
    let document = Html::parse_document(html);
    let mut results = Vec::new();
    let section_sel = Selector::parse("section.lc-panel").unwrap();
    let h2_sel = Selector::parse("h2.lc-panel-title").unwrap();
    let item_sel = Selector::parse("a.lc-side-item").unwrap();
    let img_sel = Selector::parse("img").unwrap();
    let name_sel = Selector::parse(".lc-side-name").unwrap();

    let mut seen = std::collections::HashSet::new();

    for section in document.select(&section_sel) {
        let is_recent = section
            .select(&h2_sel)
            .next()
            .map(|h| {
                let t = h.text().collect::<String>();
                t.contains("Ultimos") || t.contains("Últimos") || t.contains("Novedades")
            })
            .unwrap_or(false);
        if !is_recent {
            continue;
        }

        for item in section.select(&item_sel) {
            let uri = item.value().attr("href").unwrap_or("");
            let manga_id = uri.trim_matches('/').strip_prefix("manga/").unwrap_or(uri).to_string();
            if manga_id.is_empty() || seen.contains(&manga_id) {
                continue;
            }
            seen.insert(manga_id.clone());

            let cover_uri = item.select(&img_sel).next().and_then(|img| img.value().attr("src")).unwrap_or("");
            let name = item
                .select(&name_sel)
                .next()
                .map(|n| n.text().collect::<String>().trim().to_string())
                .unwrap_or_default();

            let cover_img_url = if cover_uri.starts_with("http") {
                cover_uri.to_string()
            } else if cover_uri.starts_with('/') {
                format!("{LEERCAPITULO_BASE_URL}{cover_uri}")
            } else {
                format!("{LEERCAPITULO_BASE_URL}/{cover_uri}")
            };

            results.push(RecentlyAddedManga {
                id: manga_id,
                title: name,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_manga_details_extracts_correct_cover_not_sidebar() {
        let sample_html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Yashahime - LeerCapitulo</title>
            <meta name="description" content="Lee Yashahime online y gratis.">
        </head>
        <body>
            <aside class="col-12 col-lg-3">
                <section class="lc-panel mb-3">
                    <h2 class="lc-panel-title">Populares</h2>
                    <div>
                        <a class="lc-side-item" href="/manga/psvkfbmjgo/one-piece/">
                            <img src="/covers/4f/1067e3fadaf8ec9edd6b71368445a7.jpg?v1792726711873" alt="Portada de One Piece">
                            <span class="lc-side-name">One Piece</span>
                        </a>
                    </div>
                </section>
            </aside>
            <div class="col-12 col-lg-9">
                <article class="lc-panel p-3 p-md-4 mb-4">
                    <div class="row g-4">
                        <div class="col-5 col-md-3">
                            <div class="lc-cover-lg">
                                <img src="/covers/44/9954b3d95fd11eb1be764d40c286d9.jpg?v1791774235446" alt="Portada de Yashahime" width="248" height="352">
                            </div>
                        </div>
                        <div class="col-7 col-md-9">
                            <h1 class="h3 mb-1">Yashahime</h1>
                            <div class="d-flex flex-wrap gap-1 mb-3">
                                <a class="badge text-bg-secondary text-decoration-none" href="/manga/?genre=action">Action</a>
                                <a class="badge text-bg-secondary text-decoration-none" href="/manga/?genre=adventure">Adventure</a>
                            </div>
                            <ul class="lc-facts mb-3">
                                <li><span class="k">Estado</span><a href="/manga/?status=ongoing">Ongoing</a></li>
                                <li><span class="k">Autor</span><span>Shiina Takashi, Takahashi Rumiko</span></li>
                                <li><span class="k">Dibujo</span><span>Shiina Takashi</span></li>
                            </ul>
                        </div>
                    </div>
                </article>
                <section class="lc-panel mb-4" id="sinopsis">
                    <h2 class="lc-panel-title">Sinopsis</h2>
                    <div class="p-3 p-md-4">
                        <p class="mb-0">Las hijas de Sesshomaru e Inuyasha emprenden un viaje a traves del tiempo.</p>
                    </div>
                </section>
                <div id="chapterList">
                    <a class="lc-chapter-row" href="/leer/9u90quvc7v/yashahime/37/">
                        <span class="n">Capitulo 37</span>
                        <span class="d">2026-10-04</span>
                    </a>
                </div>
            </div>
        </body>
        </html>
        "#;

        let details = parse_manga_details(sample_html, "9u90quvc7v/yashahime");

        // MUST be Yashahime's cover, NOT One Piece from the sidebar!
        assert_eq!(
            details.cover_img_url,
            "https://www.leercapitulo.co/covers/44/9954b3d95fd11eb1be764d40c286d9.jpg?v1791774235446"
        );
        assert_ne!(
            details.cover_img_url,
            "https://www.leercapitulo.co/covers/4f/1067e3fadaf8ec9edd6b71368445a7.jpg?v1792726711873"
        );
        assert_eq!(details.title, "Yashahime");
        assert_eq!(details.status, MangaStatus::Ongoing);
        assert_eq!(details.description, "Las hijas de Sesshomaru e Inuyasha emprenden un viaje a traves del tiempo.");
        assert_eq!(details.author.as_ref().map(|a| a.name.as_str()), Some("Shiina Takashi, Takahashi Rumiko"));
        assert_eq!(details.artist.as_ref().map(|a| a.name.as_str()), Some("Shiina Takashi"));
        assert_eq!(details.genres.len(), 2);
        assert_eq!(details.genres[0].title, "Action");
        assert_eq!(details.chapters.len(), 1);
        assert_eq!(details.chapters[0].id, "/leer/9u90quvc7v/yashahime/37/");
    }

    #[test]
    fn test_parse_reader_pages() {
        let sample_html = r#"
        <div id="imagesContainer">
            <img data-src="https://cdn.example.com/ch1/01.webp" alt="pagina 1">
            <img data-src="/images/ch1/02.webp" alt="pagina 2">
        </div>
        "#;

        let pages = parse_reader_pages(sample_html).unwrap();
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].url.as_str(), "https://cdn.example.com/ch1/01.webp");
        assert_eq!(pages[1].url.as_str(), "https://www.leercapitulo.co/images/ch1/02.webp");
    }

    #[test]
    fn test_parse_home_popular_and_recent_separation() {
        let sample_html = r#"
        <section class="lc-panel mb-3">
            <h2 class="lc-panel-title">Populares</h2>
            <div>
                <a class="lc-side-item" href="/manga/psvkfbmjgo/one-piece/">
                    <img src="/covers/onepiece.jpg" alt="Portada de One Piece">
                    <span class="lc-side-name">One Piece</span>
                </a>
            </div>
        </section>
        <section class="lc-panel mb-3">
            <h2 class="lc-panel-title">Ultimos mangas</h2>
            <div>
                <a class="lc-side-item" href="/manga/d3vq66wsfq/arrogant-slave/">
                    <img src="/covers/arrogant.jpg" alt="Portada de Arrogant Slave">
                    <span class="lc-side-name">Arrogant Slave</span>
                </a>
            </div>
        </section>
        "#;

        let popular = parse_home_popular(sample_html);
        let recent = parse_home_recent(sample_html);

        assert_eq!(popular.len(), 1);
        assert_eq!(popular[0].title, "One Piece");

        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].title, "Arrogant Slave");
    }
}
