pub mod fetch;
pub mod tui;

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
    pub relationships: Vec<CoverImgMetadata>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attributes {
    pub title: Title,
    pub description: Option<Description>,
    pub status: String,
    pub tags: Vec<Tag>,
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
pub struct CoverImgMetadata {
    pub id: String,
    pub attributes: Option<CoverImgAttributes>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverImgAttributes {
    #[serde(rename = "fileName")]
    pub file_name: String,
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

// manga's chapter structs
//
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
