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
pub struct User<T: Clone + Default + Sized>(pub Vec<T>);

impl IntoParam for User<Author> {
    fn into_param(self) -> String {
        if self.0.is_empty() {
            return String::new();
        }
        self.0.into_iter().fold(String::new(), |mut ids, author| {
            let _ = write!(ids, "&authors[]={}", author.0);
            ids
        })
    }
}

impl IntoParam for User<Artist> {
    fn into_param(self) -> String {
        if self.0.is_empty() {
            return String::new();
        }
        self.0.into_iter().fold(String::new(), |mut ids, artist| {
            let _ = write!(ids, "&artists[]={}", artist.0);
            ids
        })
    }
}

impl<T> User<T>
where
    T: Clone + Default + Sized,
{
    pub fn set_one_user(&mut self, user: T) {
        self.0.push(user);
    }
}

#[derive(Debug, Display, EnumIter, Default, Clone, Copy, PartialEq, Eq)]
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
    #[strum(to_string = "🇰🇷")]
    Korean,
    #[strum(to_string = "🇧🇷")]
    BrazilianPortuguese,
    #[strum(to_string = "🇵🇹")]
    Portuguese,
    #[strum(to_string = "🇨🇳")]
    TraditionalChinese,
    #[strum(to_string = "🇷🇺")]
    Russian,
    #[strum(to_string = "🇩🇪")]
    German,
    #[strum(to_string = "🇦🇱")]
    Albanian,
    #[strum(to_string = "🇸🇦")]
    Arabic,
    #[strum(to_string = "🇧🇬")]
    Bulgarian,
    #[strum(to_string = "🇻🇳")]
    Vietnamese,
    #[strum(to_string = "🇭🇷")]
    Croatian,
    #[strum(to_string = "🇩🇰")]
    Danish,
    #[strum(to_string = "🇳🇱")]
    Dutch,
    #[strum(to_string = "🇺🇦")]
    Ukrainian,
    // needs to be implemented
    Unkown,
}

impl From<&str> for Languages {
    fn from(value: &str) -> Self {
        match value {
            "es" => Languages::Spanish,
            "fr" => Languages::French,
            "en" => Languages::English,
            "ja" => Languages::Japanese,
            "nl" => Languages::Dutch,
            "ko" => Languages::Korean,
            "de" => Languages::German,
            "ar" => Languages::Arabic,
            "pt-br" => Languages::BrazilianPortuguese,
            "br" => Languages::Portuguese,
            "da" => Languages::Danish,
            "ru" => Languages::Russian,
            "sq" => Languages::Albanian,
            "hr" => Languages::Croatian,
            "es-la" => Languages::SpanishLa,
            "bg" => Languages::Bulgarian,
            "uk" => Languages::Ukrainian,
            "vi" => Languages::Vietnamese,
            "zh-hk" => Languages::TraditionalChinese,
            _ => Languages::Unkown,
        }
    }
}

// Todo! there has to be a better way of doing this conversion
impl From<String> for Languages {
    fn from(value: String) -> Self {
        Self::iter()
            .find(|lang| value == format!("{} {}", lang.as_emoji(), lang.as_human_readable()))
            .unwrap_or_default()
    }
}

impl Languages {
    pub fn as_human_readable(self) -> &'static str {
        match self {
            Languages::TraditionalChinese => "Chinese (traditional)",
            Languages::Vietnamese => "Vietnamese",
            Languages::English => "English",
            Languages::Dutch => "Dutch",
            Languages::French => "French",
            Languages::Korean => "Korean",
            Languages::German => "German",
            Languages::Arabic => "Arabic",
            Languages::Danish => "Danish",
            Languages::Spanish => "Spanish (traditional)",
            Languages::Russian => "Russian",
            Languages::Japanese => "Japanese",
            Languages::Albanian => "Albanian",
            Languages::Croatian => "Croatian",
            Languages::SpanishLa => "Spanish (mx)",
            Languages::Bulgarian => "Bulgarian",
            Languages::Ukrainian => "Ukrainian",
            Languages::BrazilianPortuguese => "Portuguese",
            Languages::Portuguese => "Portuguese (traditional)",
            Languages::Unkown => "",
        }
    }

    pub fn as_emoji(self) -> String {
        self.to_string()
    }

    pub fn as_param(self) -> &'static str {
        match self {
            Languages::Spanish => "es",
            Languages::French => "fr",
            Languages::English => "en",
            Languages::Japanese => "ja",
            Languages::Dutch => "nl",
            Languages::Korean => "ko",
            Languages::German => "de",
            Languages::Arabic => "ar",
            Languages::BrazilianPortuguese => "pt-br",
            Languages::Portuguese => "br",
            Languages::Danish => "da",
            Languages::Russian => "ru",
            Languages::Albanian => "sq",
            Languages::Croatian => "hr",
            Languages::SpanishLa => "es-la",
            Languages::Bulgarian => "bg",
            Languages::Ukrainian => "uk",
            Languages::Vietnamese => "vi",
            Languages::TraditionalChinese => "zh-hk",
            Languages::Unkown => unreachable!(),
        }
    }
}

impl IntoParam for Vec<Languages> {
    fn into_param(self) -> String {
        if self.is_empty() {
            return format!(
                "&availableTranslatedLanguage[]={}",
                Languages::default().as_param()
            );
        }
        self.into_iter()
            .fold(String::new(), |mut languages, language| {
                let _ = write!(
                    languages,
                    "&availableTranslatedLanguage[]={}",
                    language.as_param()
                );
                languages
            })
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
    pub languages: Vec<Languages>,
}

impl IntoParam for Filters {
    fn into_param(self) -> String {
        format!(
            "{}{}{}{}{}{}{}",
            self.authors.into_param(),
            self.artists.into_param(),
            self.languages.into_param(),
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
            languages: vec![Languages::English],
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

    pub fn set_languages(&mut self, languages: Vec<Languages>) {
        self.languages = languages;
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

    pub fn reset_author(&mut self) {
        self.authors.0 = vec![];
    }

    pub fn reset_artist(&mut self) {
        self.artists.0 = vec![];
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn language_conversion_works() {
        let language_formatted = format!(
            "{} {}",
            Languages::Spanish.as_emoji(),
            Languages::Spanish.as_human_readable()
        );

        let conversion: Languages = language_formatted.into();

        assert_eq!(conversion, Languages::Spanish);
    }
}
