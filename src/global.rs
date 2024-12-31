use once_cell::sync::{Lazy, OnceCell};
use ratatui::style::{Style, Stylize};

use crate::backend::filter::Languages;

pub static PREFERRED_LANGUAGE: OnceCell<Languages> = OnceCell::new();

pub static INSTRUCTIONS_STYLE: Lazy<Style> = Lazy::new(|| Style::default().bold().underlined().yellow());

pub static ERROR_STYLE: Lazy<Style> = Lazy::new(|| Style::default().bold().underlined().red().on_black());

pub static CURRENT_LIST_ITEM_STYLE: Lazy<Style> = Lazy::new(|| Style::default().on_blue());

#[cfg(test)]
pub mod test_utils {
    use std::error::Error;

    use crate::backend::tracker::{MangaTracker, PlanToReadArgs};

    #[derive(Debug, Clone)]
    pub struct TrackerTest {
        pub should_fail: bool,
        pub title_manga_tracked: Option<String>,
        pub error_message: Option<String>,
    }

    impl TrackerTest {
        pub fn new() -> Self {
            Self {
                title_manga_tracked: None,
                should_fail: false,
                error_message: None,
            }
        }

        pub fn failing() -> Self {
            Self {
                should_fail: true,
                title_manga_tracked: None,
                error_message: None,
            }
        }

        pub fn failing_with_error_message(error_message: &str) -> Self {
            Self {
                should_fail: true,
                title_manga_tracked: None,
                error_message: Some(error_message.to_string()),
            }
        }
    }

    impl MangaTracker for TrackerTest {
        async fn search_manga_by_title(
            &self,
            _title: manga_tui::SearchTerm,
        ) -> Result<Option<crate::backend::tracker::MangaToTrack>, Box<dyn std::error::Error>> {
            if self.should_fail {
                return Err(self.error_message.clone().unwrap_or("".to_string()).into());
            }
            Ok(None)
        }

        async fn mark_manga_as_read_with_chapter_count(
            &self,
            _manga: crate::backend::tracker::MarkAsRead<'_>,
        ) -> Result<(), Box<dyn Error>> {
            if self.should_fail {
                return Err(self.error_message.clone().unwrap_or("".to_string()).into());
            }
            Ok(())
        }

        async fn mark_manga_as_plan_to_read(&self, _manga_to_plan_to_read: PlanToReadArgs<'_>) -> Result<(), Box<dyn Error>> {
            if self.should_fail {
                return Err(self.error_message.clone().unwrap_or("".to_string()).into());
            }
            Ok(())
        }
    }
}
