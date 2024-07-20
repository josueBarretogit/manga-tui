use strum::Display;

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

#[derive(Display, Clone)]
pub enum SortBy {
    #[strum(to_string = "")]
    BestMatch,
    #[strum(to_string = "")]
    LatestUpload,
    #[strum(to_string = "")]
    OldestUpload,
    #[strum(to_string = "")]
    HighestRating,
    #[strum(to_string = "")]
    LowestRating,
}

impl IntoParam for Vec<ContentRating> {
    fn into_param(self) -> String {
        let mut result = String::new();

        if self.is_empty() {
            return format!("contentRating[]={}", ContentRating::Safe);
        }

        for cont in self {
            result.push_str(format!("contentRating[]={}&", cont).as_str());
        }

        result.pop();

        result
    }
}

impl IntoParam for SortBy {
    fn into_param(self) -> String {
        format!("{}", self)
    }
}

#[derive(Clone)]
pub struct Filters {
    pub content_rating: Vec<ContentRating>,
    pub sort_by: SortBy,
}

impl IntoParam for Filters {
    fn into_param(self) -> String {
        format!(
            "{}&{}",
            self.content_rating.into_param(),
            self.sort_by.into_param()
        )
    }
}

impl Default for Filters {
    fn default() -> Self {
        Self {
            content_rating: vec![ContentRating::Safe, ContentRating::Suggestive],
            sort_by: SortBy::BestMatch,
        }
    }
}

impl Filters {
    pub fn set_content_rating(&mut self, ratings: Vec<ContentRating>) {
        self.content_rating = ratings;
    }
}
