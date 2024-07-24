use manga_tui::exists;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{create_dir, create_dir_all, File};
use std::path::{Path, PathBuf};
use strum::Display;

pub mod database;
pub mod download;
pub mod error_log;
pub mod fetch;
pub mod tui;
pub mod filter;

#[derive(Display)]
pub enum AppDirectories {
    #[strum(to_string = "mangaDownloads")]
    MangaDownloads,
    #[strum(to_string = "errorLogs")]
    ErrorLogs,
    #[strum(to_string = "history")]
    History,
}

pub static APP_DATA_DIR: Lazy<Option<PathBuf>> = Lazy::new(|| {
    directories::ProjectDirs::from(
        env!("CARGO_PKG_NAME"),
        env!("CARGO_CRATE_NAME"),
        "manga-tui",
    )
    .map(|dirs| match std::env::var("MANGA_TUI_DATA_DIR") {
        Ok(data_dir) => PathBuf::from(data_dir),
        Err(_) => dirs.data_dir().to_path_buf(),
    })
});

pub fn build_data_dir() -> Result<(), std::io::Error> {
    let data_dir = APP_DATA_DIR.as_ref();

    match data_dir {
        Some(dir) => {
            if !exists!(dir) {
                create_dir_all(dir)?;
            }

            if !exists!(&dir.join(AppDirectories::MangaDownloads.to_string())) {
                create_dir(dir.join(AppDirectories::MangaDownloads.to_string()))?;
            }

            if !exists!(&dir.join(AppDirectories::ErrorLogs.to_string())) {
                create_dir(dir.join(AppDirectories::ErrorLogs.to_string()))?;
            }

            if !exists!(&dir.join(AppDirectories::History.to_string())) {
                create_dir(dir.join(AppDirectories::History.to_string()))?;
            }

            if !exists!(&dir
                .join(AppDirectories::ErrorLogs.to_string())
                .join("manga-tui-error-logs.txt"))
            {
                File::create(
                    dir.join(AppDirectories::ErrorLogs.to_string())
                        .join("manga-tui-error-logs.txt"),
                )?;
            }

            Ok(())
        }
        None => Err(std::io::Error::other("data dir could not be found")),
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchMangaResponse {
    pub result: String,
    pub response: String,
    pub data: Vec<Data>,
    pub limit: i32,
    pub offset: i32,
    pub total: i32,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub id: String,
    pub attributes: Attributes,
    pub relationships: Vec<MangaSearchRelationship>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attributes {
    pub title: Title,
    pub description: Option<Description>,
    pub status: String,
    pub tags: Vec<Tag>,
    pub content_rating: String,
    pub state: String,
    pub publication_demographic: Option<String>,
    pub available_translated_languages : Vec<Option<String>>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Title {
    pub en: Option<String>,
    pub ja: Option<String>,
    #[serde(rename = "ja-ro")]
    pub ja_ro: Option<String>,
    pub jp: Option<String>,
    pub zh: Option<String>,
    pub ko: Option<String>,
    #[serde(rename = "zh-ro")]
    pub zh_ro: Option<String>,
    #[serde(rename = "zh-ro")]
    pub ko_ro: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Description {
    pub en: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaSearchRelationship {
    pub id: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub attributes: Option<MangaSearchAttributes>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MangaSearchAttributes {
    #[serde(rename = "fileName")]
    pub file_name: Option<String>,
    pub name: Option<String>,
    pub locale: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: String,
    pub attributes: TagAtributtes,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagAtributtes {
    pub name: Name,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Name {
    pub en: String,
}

// manga chapter structs
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterResponse {
    pub result: String,
    pub response: String,
    pub data: Vec<ChapterData>,
    pub limit: i64,
    pub offset: i64,
    pub total: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterData {
    pub id: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub attributes: ChapterAttribute,
    pub relationships: Vec<Relationship>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterAttribute {
    pub volume: Option<String>,
    pub chapter: Option<String>,
    pub title: Option<String>,
    pub translated_language: String,
    pub external_url: Option<String>,
    pub publish_at: String,
    pub readable_at: String,
    pub created_at: String,
    pub updated_at: String,
    pub pages: i64,
    pub version: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Relationship {
    pub id: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub attributes: Option<ChapterRelationshipAttribute>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterRelationshipAttribute {
    pub name: String,
}

// Translations


#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterPagesResponse {
    pub result: String,
    pub base_url: String,
    pub chapter: ChapterPages,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterPages {
    pub hash: String,
    pub data: Vec<String>,
    pub data_saver: Vec<String>,
}

// manga statistics
//

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MangaStatisticsResponse {
    pub result: String,
    pub statistics: HashMap<String, Statistics>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Statistics {
    pub rating: Rating,
    pub follows: Option<u64>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rating {
    pub average: Option<f64>,
}

pub mod feed {
    use serde::{Deserialize, Serialize};

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct OneMangaResponse {
        pub result: String,
        pub response: String,
        pub data: super::Data,
    }
}

pub mod tags {
    use serde::{Deserialize, Serialize};

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TagsResponse {
        pub result: String,
        pub response: String,
        pub data: Vec<TagsData>,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TagsData {
        pub id: String,
        #[serde(rename = "type")]
        pub type_field: String,
        pub attributes: Attributes,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Attributes {
        pub name: Name,
        pub group: String,
        pub version: i64,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Name {
        pub en: String,
    }
}

pub mod authors {
    use serde::{Deserialize, Serialize};

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct AuthorsResponse {
        pub result: String,
        pub response: String,
        pub data: Vec<Data>,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Data {
        pub id: String,
        #[serde(rename = "type")]
        pub type_field: String,
        pub attributes: Attributes,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Attributes {
        pub name: String,
        pub created_at: String,
        pub updated_at: String,
        pub version: i64,
    }
}
