use std::fmt::Write;
use strum::{Display, EnumIter, IntoEnumIterator};

pub trait IntoParam {
    fn into_param(self) -> String;
}

#[derive(Display, Clone)]
pub enum ContentRating {
    #[strum(to_string = "safe")]
    Safe,
    #[strum(to_string = "suggestive")]
    Suggestive,
    #[strum(to_string = "erotica")]
    Erotic,
    #[strum(to_string = "pornographic")]
    Pornographic,
}

impl From<&str> for ContentRating {
    fn from(value: &str) -> Self {
        match value {
            "safe" => Self::Safe,
            "suggestive" => Self::Suggestive,
            "erotica" => Self::Erotic,
            "pornographic" => Self::Pornographic,
            _ => Self::Safe,
        }
    }
}

#[derive(Display, Clone, EnumIter, PartialEq, Eq, Default)]
pub enum SortBy {
    #[strum(to_string = "Best match")]
    BestMatch,
    #[strum(to_string = "Latest upload")]
    #[default]
    LatestUpload,
    #[strum(to_string = "Oldest upload")]
    OldestUpload,
    #[strum(to_string = "Highest rating")]
    HighestRating,
    #[strum(to_string = "Lowest rating")]
    LowestRating,
    #[strum(to_string = "Title ascending")]
    TitleAscending,
    #[strum(to_string = "Title descending")]
    TitleDescending,
    #[strum(to_string = "Oldest added")]
    OldestAdded,
    #[strum(to_string = "Recently added")]
    RecentlyAdded,
    #[strum(to_string = "Most follows")]
    MostFollows,
    #[strum(to_string = "Fewest follows")]
    FewestFollows,
    #[strum(to_string = "Year descending")]
    YearDescending,
    #[strum(to_string = "Year ascending")]
    YearAscending,
}

#[derive(Display, Clone, EnumIter, PartialEq, Eq)]
pub enum Language {
    English,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Tags(Vec<String>);

impl IntoParam for Tags {
    fn into_param(self) -> String {
        let mut param = String::new();

        if self.0.is_empty() {
            return param;
        }

        for id_tag in self.0 {
            param.push_str(format!("&includedTags[]={}", id_tag).as_str());
        }

        param
    }
}

impl IntoParam for Vec<ContentRating> {
    fn into_param(self) -> String {
        let mut result = String::new();

        if self.is_empty() {
            return format!("&contentRating[]={}", ContentRating::Safe);
        }

        for cont in self {
            result.push_str(format!("&contentRating[]={}", cont).as_str());
        }

        result
    }
}

impl From<&str> for SortBy {
    fn from(value: &str) -> Self {
        SortBy::iter()
            .find(|sort_by| sort_by.to_string() == value)
            .unwrap()
    }
}

impl IntoParam for SortBy {
    fn into_param(self) -> String {
        match self {
            Self::BestMatch => "&order[relevance]=desc".to_string(),
            Self::LatestUpload => "&order[latestUploadedChapter]=desc".to_string(),
            Self::OldestUpload => "&order[latestUploadedChapter]=asc".to_string(),
            Self::OldestAdded => "&order[createdAt]=asc".to_string(),
            Self::MostFollows => "&order[followedCount]=desc".to_string(),
            Self::LowestRating => "&order[rating]=asc".to_string(),
            Self::HighestRating => "&order[rating]=desc".to_string(),
            Self::RecentlyAdded => "&order[createdAt]=desc".to_string(),
            Self::FewestFollows => "&order[followedCount]=asc".to_string(),
            Self::TitleAscending => "&order[title]=asc".to_string(),
            Self::TitleDescending => "&order[title]=desc".to_string(),
            Self::YearAscending => "&order[year]=asc".to_string(),
            Self::YearDescending => "&order[year]=desc".to_string(),
        }
    }
}

#[derive(Display, Clone, EnumIter, PartialEq, Eq)]
pub enum MagazineDemographic {
    Shounen,
    Shoujo,
    Seinen,
    Josei,
}

impl From<&str> for MagazineDemographic {
    fn from(value: &str) -> Self {
        Self::iter()
            .find(|mag| mag.to_string().to_lowercase() == value.to_lowercase())
            .unwrap()
    }
}

impl IntoParam for Vec<MagazineDemographic> {
    fn into_param(self) -> String {
        let mut param = String::new();

        if self.is_empty() {
            return param;
        }

        for magazine in self {
            param.push_str(
                format!(
                    "&publicationDemographic[]={}",
                    magazine.to_string().to_lowercase()
                )
                .as_str(),
            );
        }

        param
    }
}

#[derive(Default, Clone)]
pub struct Author(String);

impl Author {
    pub fn new(id_author: String) -> Self {
        Author(id_author)
    }
}

#[derive(Default, Clone)]
pub struct Artist(String);

impl Artist {
    pub fn new(id_artist: String) -> Self {
        Artist(id_artist)
    }
}

#[derive(Default, Clone)]
pub struct User<T: Clone + Default>(pub Vec<T>);

impl IntoParam for User<Author> {
    fn into_param(self) -> String {
        self.0.into_iter().fold(String::new(), |mut ids, author| {
            let _ = write!(ids, "&authors[]={}", author.0);
            ids
        })
    }
}

impl IntoParam for User<Artist> {
    fn into_param(self) -> String {
        self.0.into_iter().fold(String::new(), |mut ids, artist| {
            let _ = write!(ids, "&artists[]={}", artist.0);
            ids
        })
    }
}

impl<T> User<T>
where
    T: Clone + Default,
{
    pub fn set_one_user(&mut self, user: T) {
        self.0.push(user);
    }
}

#[derive(Clone)]
pub struct Filters {
    pub content_rating: Vec<ContentRating>,
    pub sort_by: SortBy,
    pub tags: Tags,
    pub magazine_demographic: Vec<MagazineDemographic>,
    pub authors: User<Author>,
    pub artists: User<Artist>,
}

impl IntoParam for Filters {
    fn into_param(self) -> String {
        format!(
            "{}{}{}{}{}",
            self.authors.into_param(),
            self.tags.into_param(),
            self.magazine_demographic.into_param(),
            self.content_rating.into_param(),
            self.sort_by.into_param(),
        )
    }
}

impl Default for Filters {
    fn default() -> Self {
        Self {
            content_rating: vec![ContentRating::Safe, ContentRating::Suggestive],
            sort_by: SortBy::default(),
            tags: Tags(vec![]),
            magazine_demographic: vec![],
            authors: User::<Author>::default(),
            artists: User::<Artist>::default(),
        }
    }
}

impl Filters {
    pub fn set_content_rating(&mut self, ratings: Vec<ContentRating>) {
        self.content_rating = ratings;
    }

    pub fn set_sort_by(&mut self, sort_by: SortBy) {
        self.sort_by = sort_by;
    }
    pub fn set_tags(&mut self, tags: Vec<String>) {
        self.tags.0 = tags;
    }

    pub fn set_magazine_demographic(&mut self, magazine_demographics: Vec<MagazineDemographic>) {
        self.magazine_demographic = magazine_demographics;
    }

    pub fn set_authors(&mut self, author_ids: Vec<Author>) {
        self.authors.0 = author_ids;
    }

    pub fn set_artists(&mut self, artist_ids: Vec<Artist>) {
        self.artists.0 = artist_ids;
    }
}
