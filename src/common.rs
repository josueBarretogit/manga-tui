use strum::{Display, EnumIter};
use crate::backend::filter::Languages;

#[derive(Default, Clone, Debug)]
pub struct Author {
    pub id: String,
    pub name: String,
}

#[derive(Default, Clone, Debug)]
pub struct Artist {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Default, Debug)]
pub struct Manga {
    pub id: String,
    pub title: String,
    pub description: String,
    pub content_rating: String,
    pub publication_demographic: String,
    pub tags: Vec<String>,
    pub status: String,
    pub img_url: Option<String>,
    pub author: Author,
    pub artist: Artist,
    pub available_languages: Vec<Languages>,
    pub created_at: String,
}

#[derive(Display, Clone, Copy, EnumIter, Default, Debug, Eq, PartialEq)]
pub enum PageType {
    #[strum(to_string = "data")]
    HighQuality,
    #[strum(to_string = "data-saver")]
    #[default]
    LowQuality,
}

impl PageType {
    pub fn toggle(self) -> Self {
        match self {
            Self::LowQuality => Self::HighQuality,
            Self::HighQuality => Self::LowQuality,
        }
    }

    pub fn as_human_readable(&self) -> &str {
        match self {
            Self::LowQuality => "Low quality",
            Self::HighQuality => "High quality",
        }
    }
}
