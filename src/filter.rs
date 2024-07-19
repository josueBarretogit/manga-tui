use strum::Display;

pub trait IntoParam {
    fn into_param(self) -> String;
}

#[derive(Display, Clone)]
pub enum ContentRating {
    #[strum(to_string = "contentRating[]=safe")]
    Safe,
    #[strum(to_string = "contentRating[]=suggestive")]
    Suggestive,
    #[strum(to_string = "contentRating[]=erotica")]
    Erotic,
    #[strum(to_string = "contentRating[]=pornographic")]
    Pornographic,
}

impl IntoParam for Vec<ContentRating> {
    fn into_param(self) -> String {
        let mut result = String::new();

        for cont in self {
            result.push_str(format!("{}&", cont).as_str());
        }

        result.pop();

        result
    }
}

#[derive(Clone)]
pub struct Filters {
    content_rating: Vec<ContentRating>,
}

impl IntoParam for Filters {
    fn into_param(self) -> String {
        self.content_rating.into_param()
    }
}

impl Default for Filters {
    fn default() -> Self {
        Self {
            content_rating: vec![
                ContentRating::Safe,
                ContentRating::Suggestive,
            ],
        }
    }
}

impl Filters {
    pub fn set_content_rating(&mut self, ratings: Vec<ContentRating>) {
        self.content_rating = ratings;
    }
}
