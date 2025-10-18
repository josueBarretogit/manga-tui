use strum_macros::{Display, EnumCount, EnumIter, FromRepr};

pub mod feed;
pub mod home;
pub mod manga;
pub mod reader;
pub mod search;

#[derive(Debug, Clone, Copy, Default, FromRepr, Display, EnumIter, EnumCount, PartialEq, Eq, PartialOrd, Ord)]
pub enum SelectedPage {
    ReaderTab,
    MangaTab,
    #[default]
    Home,
    Search,
    Feed,
}
