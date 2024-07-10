use strum::{Display, EnumCount, EnumIter, FromRepr, IntoEnumIterator};

pub mod home;
pub mod manga;
pub mod reader;
pub mod search;

#[derive(
    Clone, Copy, Default, FromRepr, Display, EnumIter, EnumCount, PartialEq, Eq, PartialOrd, Ord,
)]
pub enum SelectedTabs {
    // #[default]
    // Home,
    ///In these page the user will be able to search for a manga by title
    ///Reference: https://mangadex.org/search?q=death+note
    ReaderTab,
    MangaTab,
    #[default]
    Home,
    Search,
}

impl SelectedTabs {
    pub fn next(self) -> Self {
        let current_index = self as usize;
        let next_index = current_index.saturating_add(1);
        Self::from_repr(next_index).unwrap_or(Self::from_repr(0).unwrap())
    }

    pub fn previous(self) -> Self {
        let current_index = self as usize;
        let previous_index = current_index.saturating_sub(1);
        if current_index == 0 {
            return Self::from_repr(Self::iter().len() - 1).unwrap();
        }

        Self::from_repr(previous_index).unwrap()
    }
}
