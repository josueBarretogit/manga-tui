use std::fmt::{Debug, Write};

use strum::{Display, EnumIter, IntoEnumIterator};

use crate::global::PREFERRED_LANGUAGE;
use crate::view::widgets::filter_widget::state::{FilterListItem, TagListItem, TagListItemState};

pub trait IntoParam: Debug {
    fn into_param(self) -> String;
}

#[derive(Display, Clone, Debug)]
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

#[derive(Display, Clone, EnumIter, PartialEq, Eq, Default, Debug)]
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

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TagSelection {
    Included,
    Excluded,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TagData {
    id: String,
    state: TagSelection,
}

impl TagData {
    pub fn new(id: String, state: TagSelection) -> Self {
        Self { id, state }
    }
}

impl From<&TagListItem> for TagData {
    fn from(value: &TagListItem) -> Self {
        Self {
            id: value.id.clone(),
            state: if value.state == TagListItemState::Included { TagSelection::Included } else { TagSelection::Excluded },
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Tags(Vec<TagData>);

impl Tags {
    pub fn new(tags: Vec<TagData>) -> Self {
        Self(tags)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl IntoParam for Tags {
    fn into_param(self) -> String {
        let mut param = String::new();

        if self.0.is_empty() {
            return param;
        }

        for tag in self.0 {
            let parameter = match tag.state {
                TagSelection::Included => "&includedTags[]=",
                TagSelection::Excluded => "&excludedTags[]=",
            };
            param.push_str(format!("{}{}", parameter, tag.id).as_str());
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
        SortBy::iter().find(|sort_by| sort_by.to_string() == value).unwrap()
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

#[derive(Display, Clone, EnumIter, PartialEq, Eq, Debug)]
pub enum MagazineDemographic {
    Shounen,
    Shoujo,
    Seinen,
    Josei,
}

impl From<&str> for MagazineDemographic {
    fn from(value: &str) -> Self {
        Self::iter().find(|mag| mag.to_string().to_lowercase() == value.to_lowercase()).unwrap()
    }
}

impl IntoParam for Vec<MagazineDemographic> {
    fn into_param(self) -> String {
        let mut param = String::new();

        if self.is_empty() {
            return param;
        }

        for magazine in self {
            param.push_str(format!("&publicationDemographic[]={}", magazine.to_string().to_lowercase()).as_str());
        }

        param
    }
}

#[derive(Default, Clone, Debug)]
pub struct Author(String);

impl Author {
    pub fn new(id_author: String) -> Self {
        Author(id_author)
    }
}

#[derive(Default, Clone, Debug)]
pub struct Artist(String);

impl Artist {
    pub fn new(id_artist: String) -> Self {
        Artist(id_artist)
    }
}

#[derive(Default, Clone, Debug)]
pub struct User<T: Clone + Default>(pub Vec<T>);

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
    pub fn new(users: Vec<T>) -> Self {
        Self(users)
    }

    pub fn set_one_user(&mut self, user: T) {
        self.0.push(user);
    }
}

#[derive(Debug, Display, EnumIter, Default, Clone, Copy, PartialEq, Eq)]
pub enum Languages {
    French,
    #[default]
    English,
    Spanish,
    #[strum(to_string = "Spanish (latam)")]
    SpanishLa,
    Italian,
    Japanese,
    Korean,
    #[strum(to_string = "Portuguese (brazil)")]
    BrazilianPortuguese,
    #[strum(to_string = "Portuguese")]
    Portuguese,
    #[strum(to_string = "Chinese (traditional)")]
    TraditionalChinese,
    #[strum(to_string = "Chinese (simplified)")]
    SimplifiedChinese,
    Russian,
    German,
    Burmese,
    Arabic,
    Bulgarian,
    Vietnamese,
    Croatian,
    Hungarian,
    Dutch,
    Mongolian,
    Turkish,
    Ukrainian,
    Thai,
    Catalan,
    Indonesian,
    Filipino,
    Hindi,
    Romanian,
    Hebrew,
    Polish,
    Persian,
    // Some language that is missing from this `list`
    Unkown,
}

impl From<FilterListItem> for Languages {
    fn from(value: FilterListItem) -> Self {
        Self::iter()
            .find(|lang| value.name == format!("{} {}", lang.as_emoji(), lang.as_human_readable()))
            .unwrap_or_default()
    }
}

impl Languages {
    pub fn as_emoji(self) -> &'static str {
        match self {
            Self::Mongolian => "🇲🇳",
            Self::Polish => "🇵🇱",
            Self::Persian => "🇮🇷",
            Self::Romanian => "🇷🇴",
            Self::Hungarian => "🇭🇺",
            Self::Hebrew => "🇮🇱",
            Self::Filipino => "🇵🇭",
            Self::Catalan => "",
            Self::Hindi => "🇮🇳",
            Self::Indonesian => "🇮🇩",
            Self::Thai => "🇹🇭",
            Self::Turkish => "🇹🇷",
            Self::SimplifiedChinese => "🇨🇳",
            Self::TraditionalChinese => "🇨🇳",
            Self::Italian => "🇮🇹",
            Self::Vietnamese => "🇻🇳",
            Self::English => "🇺🇸",
            Self::Dutch => "🇳🇱",
            Self::French => "🇫🇷",
            Self::Korean => "🇰🇷",
            Self::German => "🇩🇪",
            Self::Arabic => "🇸🇦",
            Self::Spanish => "🇪🇸",
            Self::Russian => "🇷🇺",
            Self::Japanese => "🇯🇵",
            Self::Burmese => "🇲🇲",
            Self::Croatian => "🇭🇷",
            Self::SpanishLa => "🇲🇽",
            Self::Bulgarian => "🇧🇬",
            Self::Ukrainian => "🇺🇦",
            Self::BrazilianPortuguese => "🇧🇷",
            Self::Portuguese => "🇵🇹",
            Self::Unkown => unreachable!(),
        }
    }

    pub fn get_preferred_lang() -> &'static Languages {
        PREFERRED_LANGUAGE.get().expect("an error ocurred when setting preferred language")
    }

    pub fn as_human_readable(self) -> String {
        self.to_string()
    }

    pub fn as_iso_code(self) -> &'static str {
        match self {
            Self::Mongolian => "mn",
            Self::Persian => "fa",
            Self::Polish => "pl",
            Self::Romanian => "ro",
            Self::Hungarian => "hu",
            Self::Hebrew => "he",
            Self::Filipino => "fi",
            Self::Catalan => "ca",
            Self::Hindi => "hi",
            Self::Indonesian => "id",
            Self::Turkish => "tr",
            Self::Spanish => "es",
            Self::French => "fr",
            Self::English => "en",
            Self::Japanese => "ja",
            Self::Dutch => "nl",
            Self::Korean => "ko",
            Self::German => "de",
            Self::Arabic => "ar",
            Self::BrazilianPortuguese => "pt-br",
            Self::Portuguese => "pt",
            Self::Russian => "ru",
            Self::Burmese => "my",
            Self::Croatian => "hr",
            Self::SpanishLa => "es-la",
            Self::Bulgarian => "bg",
            Self::Ukrainian => "uk",
            Self::Vietnamese => "vi",
            Self::TraditionalChinese => "zh-hk",
            Self::Italian => "it",
            Self::SimplifiedChinese => "zh",
            Self::Thai => "th",
            Languages::Unkown => "",
        }
    }

    pub fn try_from_iso_code(code: &str) -> Option<Self> {
        Languages::iter().find(|lang| lang.as_iso_code() == code)
    }
}

impl IntoParam for Vec<Languages> {
    fn into_param(self) -> String {
        if self.is_empty() {
            return format!("&availableTranslatedLanguage[]={}", Languages::get_preferred_lang().as_iso_code());
        }
        self.into_iter()
            .filter(|lang| *lang != Languages::Unkown)
            .fold(String::new(), |mut languages, language| {
                let _ = write!(languages, "&availableTranslatedLanguage[]={}", language.as_iso_code());
                languages
            })
    }
}

#[derive(Clone, Display, EnumIter, Debug)]
pub enum PublicationStatus {
    #[strum(to_string = "ongoing")]
    Ongoing,
    #[strum(to_string = "completed")]
    Completed,
    #[strum(to_string = "hiatus")]
    Hiatus,
    #[strum(to_string = "cancelled")]
    Cancelled,
}

impl From<&str> for PublicationStatus {
    fn from(value: &str) -> Self {
        PublicationStatus::iter().find(|status| status.to_string() == value).unwrap()
    }
}

impl IntoParam for Vec<PublicationStatus> {
    fn into_param(self) -> String {
        let param = String::new();
        if self.is_empty() {
            return param;
        }
        self.into_iter().fold(String::new(), |mut name, current_status| {
            let _ = write!(name, "&status[]={}", current_status);
            name
        })
    }
}

#[derive(Clone, Debug)]
pub struct Filters {
    pub content_rating: Vec<ContentRating>,
    pub publication_status: Vec<PublicationStatus>,
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
            "{}{}{}{}{}{}{}{}",
            self.authors.into_param(),
            self.artists.into_param(),
            self.publication_status.into_param(),
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
            publication_status: vec![],
            sort_by: SortBy::default(),
            tags: Tags(vec![]),
            magazine_demographic: vec![],
            authors: User::<Author>::default(),
            artists: User::<Artist>::default(),
            languages: vec![*Languages::get_preferred_lang()],
        }
    }
}

impl Filters {
    pub fn set_content_rating(&mut self, ratings: Vec<ContentRating>) {
        self.content_rating = ratings;
    }

    pub fn set_publication_status(&mut self, status: Vec<PublicationStatus>) {
        self.publication_status = status;
    }

    pub fn set_sort_by(&mut self, sort_by: SortBy) {
        self.sort_by = sort_by;
    }

    pub fn set_tags(&mut self, tags: Vec<TagData>) {
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

/// This test may be changed depending on the Mangadex Api
#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn language_from_filter_list_item() {
        let language_formatted = FilterListItem {
            name: format!("{} {}", Languages::Spanish.as_emoji(), Languages::Spanish.as_human_readable()),
            is_selected: false,
        };

        let conversion: Languages = language_formatted.into();

        assert_eq!(conversion, Languages::Spanish);
    }

    #[test]
    fn filter_by_content_rating_works() {
        let content_rating =
            vec![ContentRating::Safe, ContentRating::Erotic, ContentRating::Pornographic, ContentRating::Suggestive];

        assert_eq!(
            "&contentRating[]=safe&contentRating[]=erotica&contentRating[]=pornographic&contentRating[]=suggestive",
            content_rating.into_param()
        );
    }

    #[test]
    fn sort_by_works() {
        assert_eq!("&order[relevance]=desc", SortBy::BestMatch.into_param());

        assert_eq!("&order[createdAt]=asc", SortBy::OldestAdded.into_param());

        assert_eq!("&order[followedCount]=desc", SortBy::MostFollows.into_param());

        assert_eq!("&order[followedCount]=asc", SortBy::FewestFollows.into_param());

        assert_eq!("&order[latestUploadedChapter]=desc", SortBy::LatestUpload.into_param());

        assert_eq!("&order[latestUploadedChapter]=asc", SortBy::OldestUpload.into_param());

        assert_eq!("&order[rating]=desc", SortBy::HighestRating.into_param());

        assert_eq!("&order[rating]=asc", SortBy::LowestRating.into_param());

        assert_eq!("&order[createdAt]=desc", SortBy::RecentlyAdded.into_param());

        assert_eq!("&order[year]=asc", SortBy::YearAscending.into_param());

        assert_eq!("&order[year]=desc", SortBy::YearDescending.into_param());

        assert_eq!("&order[title]=asc", SortBy::TitleAscending.into_param());

        assert_eq!("&order[title]=desc", SortBy::TitleDescending.into_param());
    }

    #[test]
    fn filter_by_magazine_demographic_works() {
        let magazine_demographic = vec![
            MagazineDemographic::Shounen,
            MagazineDemographic::Shoujo,
            MagazineDemographic::Josei,
            MagazineDemographic::Seinen,
        ];

        assert_eq!(
            "&publicationDemographic[]=shounen&publicationDemographic[]=shoujo&publicationDemographic[]=josei&publicationDemographic[]=seinen",
            magazine_demographic.into_param()
        );
    }

    #[test]
    fn filter_by_artist_works() {
        let sample_artists: Vec<Artist> = vec![Artist::new("id_artist1".to_string()), Artist::new("id_artist2".to_string())];
        let filter_artist = User::<Artist>::new(sample_artists);
        assert_eq!("&artists[]=id_artist1&artists[]=id_artist2", filter_artist.into_param());
    }

    #[test]
    fn filter_by_author_works() {
        let sample_authors: Vec<Author> = vec![Author::new("id_author1".to_string()), Author::new("id_author2".to_string())];
        let filter_artist = User::<Author>::new(sample_authors);
        assert_eq!("&authors[]=id_author1&authors[]=id_author2", filter_artist.into_param());
    }

    #[test]
    fn filter_by_language_works() {
        let _ = PREFERRED_LANGUAGE.set(Languages::default());

        let default_language: Vec<Languages> = vec![];
        assert_eq!("&availableTranslatedLanguage[]=en", default_language.into_param());

        let languages: Vec<Languages> =
            vec![Languages::English, Languages::Spanish, Languages::SpanishLa, Languages::BrazilianPortuguese];

        assert_eq!(
            "&availableTranslatedLanguage[]=en&availableTranslatedLanguage[]=es&availableTranslatedLanguage[]=es-la&availableTranslatedLanguage[]=pt-br",
            languages.into_param()
        );
    }

    #[test]
    fn filter_by_publication_status_works() {
        let publication_status: Vec<PublicationStatus> =
            vec![PublicationStatus::Ongoing, PublicationStatus::Hiatus, PublicationStatus::Completed, PublicationStatus::Cancelled];

        assert_eq!("&status[]=ongoing&status[]=hiatus&status[]=completed&status[]=cancelled", publication_status.into_param());
    }

    #[test]
    fn filter_by_tags_works() {
        let tags = Tags::new(vec![
            TagData {
                id: "id_tag_included".to_string(),
                state: TagSelection::Included,
            },
            TagData {
                id: "id_tag_excluded".to_string(),
                state: TagSelection::Excluded,
            },
        ]);

        assert_eq!("&includedTags[]=id_tag_included&excludedTags[]=id_tag_excluded", tags.into_param());
    }

    #[test]
    fn filters_combined_work() {
        let filters = Filters::default();

        assert_eq!(
            "&availableTranslatedLanguage[]=en&contentRating[]=safe&contentRating[]=suggestive&order[latestUploadedChapter]=desc",
            filters.into_param()
        );

        let mut filters = Filters::default();

        filters.set_tags(vec![TagData::new("id_1".to_string(), TagSelection::Included)]);

        filters.set_authors(vec![Author::new("id_1".to_string()), Author::new("id_2".to_string())]);

        filters.set_languages(vec![Languages::French, Languages::Spanish]);

        assert_eq!(
            "&authors[]=id_1&authors[]=id_2&availableTranslatedLanguage[]=fr&availableTranslatedLanguage[]=es&includedTags[]=id_1&contentRating[]=safe&contentRating[]=suggestive&order[latestUploadedChapter]=desc",
            filters.into_param()
        );
    }
}
