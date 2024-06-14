use strum::{Display, EnumCount, EnumIter, FromRepr};

pub mod search;

#[derive(
    Clone, Default, FromRepr, Display, EnumIter, EnumCount, PartialEq, Eq, PartialOrd, Ord,
)]
pub enum SelectedTabs {
    #[default]
    Home,
    ///In these page the user will be able to search for a manga by title
    ///Reference: https://mangadex.org/search?q=death+note
    Search,
}
