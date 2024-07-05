use ratatui::widgets::ListItem;

pub enum PageItemState {
    Loading,
    Display,
    NotFound,
}

pub struct PagesItem {
    pub number: usize,
    pub state: PageItemState,
}
pub struct PagesList {
    pub pages: Vec<PagesItem>,
}

impl From<PagesItem> for ListItem<'_> {
    fn from(value: PagesItem) -> Self {
        ListItem::new(value.number.to_string())
    }
}
