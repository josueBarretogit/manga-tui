pub mod fetch;
pub mod tui;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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
}

// Translations

#[derive(strum_macros::Display, Default, Clone, Copy)]
pub enum Languages {
    #[strum(to_string = "🇫🇷")]
    French,
    #[default]
    #[strum(to_string = "🇬🇧")]
    English,
    #[strum(to_string = "🇪🇸")]
    Spanish,
    #[strum(to_string = "🇲🇽")]
    SpanishLa,
    #[strum(to_string = "🇯🇵")]
    Japanese,
    #[strum(to_string = "🇪🇸")]
    Korean,
    #[strum(to_string = "🇧🇷")]
    BrazilianPortuguese,
    #[strum(to_string = "🇨🇳")]
    TraditionalChinese,
    #[strum(to_string = "🇷🇺")]
    Russian,
    #[strum(to_string = "🇩🇪")]
    German,
}

impl From<&str> for Languages {
    fn from(value: &str) -> Self {
        match value {
            "fr" => Self::French,
            "en" => Self::English,
            "es" => Self::Spanish,
            "es-la" => Self::SpanishLa,
            "ko" => Self::Korean,
            "de" => Self::German,
            "pt-br" => Self::BrazilianPortuguese,
            "ru" => Self::Russian,
            "zh-hk" => Self::TraditionalChinese,
            "ja" | "ja-ro" => Self::Japanese,
            _ => Self::default(),
        }
    }
}

impl From<Languages> for &str {
    fn from(value: Languages) -> Self {
        match value {
            Languages::Spanish => "es",
            Languages::French => "fr",
            Languages::English => "en",
            Languages::Japanese => "ja",
            _ => "",
        }
    }
}

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
    pub follows: u64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rating {
    pub average: f64,
}
