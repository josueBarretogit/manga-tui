use std::fmt::Debug;
use std::marker::PhantomData;

use crossterm::event::{KeyCode, KeyEvent};
use manga_tui::SearchTerm;
use ratatui::style::Stylize;
use ratatui::text::Span;
use ratatui::widgets::*;
use strum::{Display, IntoEnumIterator};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

use super::super::{API_URL_BASE, COVER_IMG_URL_BASE};
use crate::backend::cache::in_memory::InMemoryCache;
use crate::backend::manga_provider::filters::FiltersCache;
use crate::backend::manga_provider::mangadex::api_responses::authors::AuthorsResponse;
use crate::backend::manga_provider::mangadex::api_responses::tags::TagsResponse;
use crate::backend::manga_provider::mangadex::filters::api_parameter::*;
use crate::backend::manga_provider::mangadex::{MANGADEX_CACHE_BASE_DIRECTORY, MANGADEX_CACHE_FILENAME, MangadexClient};
use crate::backend::manga_provider::{EventHandler as FiltersEventHandler, FiltersHandler, Languages};
use crate::backend::tui::Events;

#[derive(Debug, PartialEq)]
pub enum FilterEvents {
    LoadAuthors(Option<AuthorsResponse>),
    LoadArtists(Option<AuthorsResponse>),
    SearchTags,
    LoadTags(TagsResponse),
}

#[derive(Display, PartialEq, Eq, Debug)]
pub enum MangaFilters {
    #[strum(to_string = "Content rating")]
    ContentRating,
    Languages,
    #[strum(to_string = "Sort by")]
    SortBy,
    #[strum(to_string = "Publication status")]
    PublicationStatus,
    #[strum(to_string = "Magazine demographic")]
    MagazineDemographic,
    Tags,
    Authors,
    Artists,
}

pub const FILTERS: [MangaFilters; 8] = [
    MangaFilters::ContentRating,
    MangaFilters::Languages,
    MangaFilters::SortBy,
    MangaFilters::PublicationStatus,
    MangaFilters::Tags,
    MangaFilters::MagazineDemographic,
    MangaFilters::Authors,
    MangaFilters::Artists,
];

#[derive(Clone, Debug, PartialEq)]
pub struct FilterListItem {
    pub is_selected: bool,
    pub name: String,
}

impl FilterListItem {
    pub fn toggle(&mut self) {
        self.is_selected = !self.is_selected;
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ContentRatingState;

#[derive(Debug, PartialEq, Eq)]
pub struct PublicationStatusState;

#[derive(Debug, PartialEq, Eq)]
pub struct SortByState;

#[derive(Debug, PartialEq, Eq)]
pub struct MagazineDemographicState;

#[derive(Debug, PartialEq, Eq)]
pub struct LanguageState;

#[derive(Debug, PartialEq)]
pub struct FilterList<T> {
    pub items: Vec<FilterListItem>,
    pub state: ListState,
    _state: PhantomData<T>,
}

struct FilterListIter<'a> {
    items: &'a [FilterListItem],
    index: usize,
}

impl<'a> FilterListIter<'a> {
    fn new<T>(filter_list: &'a FilterList<T>) -> Self {
        Self {
            items: &filter_list.items,
            index: 0,
        }
    }
}

impl<'a> Iterator for FilterListIter<'a> {
    type Item = &'a FilterListItem;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.items.get(self.index);

        self.index += 1;

        next
    }
}

impl<T> FilterList<T> {
    fn iter(&self) -> FilterListIter<'_> {
        FilterListIter::new(self)
    }

    pub fn toggle(&mut self) {
        if let Some(index) = self.state.selected() {
            if let Some(content_rating) = self.items.get_mut(index) {
                content_rating.toggle();
            }
        }
    }

    pub fn scroll_down(&mut self) {
        if self.state.selected().is_some_and(|index| index == self.items.len() - 1) {
            self.state.select_first();
        } else {
            self.state.select_next()
        }
    }

    pub fn scroll_up(&mut self) {
        if self.state.selected().is_some_and(|index| index == 0) {
            self.state.select_last();
        } else {
            self.state.select_previous()
        }
    }

    pub fn num_filters_active(&self) -> usize {
        self.items.iter().filter(|item| item.is_selected).count()
    }
}

impl Default for FilterList<ContentRatingState> {
    fn default() -> Self {
        Self {
            items: ContentRating::iter()
                .map(|rating| FilterListItem {
                    is_selected: rating == ContentRating::default(),
                    name: rating.to_string(),
                })
                .collect(),
            state: ListState::default(),
            _state: PhantomData::<ContentRatingState>,
        }
    }
}

impl FilterList<ContentRatingState> {
    fn from_content_ratings(content_ratings: &[ContentRating]) -> Self {
        Self {
            items: ContentRating::iter()
                .map(|rating| FilterListItem {
                    is_selected: content_ratings.contains(&rating),
                    name: rating.to_string(),
                })
                .collect(),
            state: ListState::default(),
            _state: PhantomData::<ContentRatingState>,
        }
    }
}

impl Default for FilterList<SortByState> {
    fn default() -> Self {
        let sort_by_items = SortBy::iter().map(|sort_by_elem| FilterListItem {
            is_selected: sort_by_elem == SortBy::default(),
            name: sort_by_elem.to_string(),
        });

        Self {
            items: sort_by_items.collect(),
            state: ListState::default(),
            _state: PhantomData::<SortByState>,
        }
    }
}

impl FilterList<SortByState> {
    fn from_sort_by(cached_sort_by: &SortBy) -> Self {
        let sort_by_items = SortBy::iter().map(|sort_by_elem| FilterListItem {
            is_selected: sort_by_elem == *cached_sort_by,
            name: sort_by_elem.to_string(),
        });

        Self {
            items: sort_by_items.collect(),
            state: ListState::default(),
            _state: PhantomData::<SortByState>,
        }
    }
}

impl Default for FilterList<MagazineDemographicState> {
    fn default() -> Self {
        let items = MagazineDemographic::iter().map(|mag| FilterListItem {
            name: mag.to_string(),
            is_selected: false,
        });
        Self {
            items: items.collect(),
            state: ListState::default(),
            _state: PhantomData,
        }
    }
}

impl FilterList<MagazineDemographicState> {
    fn from_magazine_demographic(magazine_demographic: &[MagazineDemographic]) -> Self {
        let items = MagazineDemographic::iter().map(|mag| FilterListItem {
            name: mag.to_string(),
            is_selected: magazine_demographic.contains(&mag),
        });
        Self {
            items: items.collect(),
            state: ListState::default(),
            _state: PhantomData,
        }
    }
}

impl Default for FilterList<PublicationStatusState> {
    fn default() -> Self {
        let items = PublicationStatus::iter().map(|status| FilterListItem {
            is_selected: false,
            name: status.to_string(),
        });
        Self {
            items: items.collect(),
            state: ListState::default(),
            _state: PhantomData,
        }
    }
}

impl FilterList<PublicationStatusState> {
    fn from_publication_status(publication_statuses: &[PublicationStatus]) -> Self {
        let items = PublicationStatus::iter().map(|status| FilterListItem {
            is_selected: publication_statuses.contains(&status),
            name: status.to_string(),
        });
        Self {
            items: items.collect(),
            state: ListState::default(),
            _state: PhantomData,
        }
    }
}

impl FilterList<SortByState> {
    pub fn toggle_sort_by(&mut self) {
        for item in self.items.iter_mut() {
            item.is_selected = false;
        }

        if let Some(index) = self.state.selected() {
            if let Some(sort_by) = self.items.get_mut(index) {
                sort_by.toggle();
            }
        }
    }
}

impl Default for FilterList<LanguageState> {
    fn default() -> Self {
        let items = Languages::iter().filter(|lang| *lang != Languages::Unkown).map(|lang| FilterListItem {
            name: format!("{} {}", lang.as_emoji(), lang.as_human_readable()),
            is_selected: lang == *Languages::get_preferred_lang(),
        });

        Self {
            items: items.collect(),
            state: ListState::default(),
            _state: PhantomData,
        }
    }
}

impl FilterList<LanguageState> {
    fn from_languages(from_languages: &[Languages]) -> Self {
        let items = Languages::iterate().map(|lang| FilterListItem {
            name: format!("{} {}", lang.as_emoji(), lang.as_human_readable()),
            is_selected: from_languages.contains(&lang),
        });

        Self {
            items: items.collect(),
            state: ListState::default(),
            _state: PhantomData,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ListItemId {
    pub id: String,
    pub name: String,
    pub is_selected: bool,
}

#[derive(Default, Debug)]
pub struct AuthorState;

#[derive(Default, Debug)]
pub struct ArtistState;

// It's called dynamic because the items must be fetched
#[derive(Default, Debug)]
pub struct FilterListDynamic<T> {
    pub items: Option<Vec<ListItemId>>,
    pub state: ListState,
    pub search_bar: Input,
    pub _is_found: bool,
    _state: PhantomData<T>,
}

pub trait SendEventOnSuccess {
    fn send(data: Option<AuthorsResponse>) -> FilterEvents;
}

impl SendEventOnSuccess for ArtistState {
    fn send(data: Option<AuthorsResponse>) -> FilterEvents {
        FilterEvents::LoadArtists(data)
    }
}

impl FilterListDynamic<AuthorState> {
    fn from_authors(authors: &User<AuthorFilterState>) -> Self {
        Self {
            items: if authors.is_empty() {
                None
            } else {
                Some(
                    authors
                        .iter()
                        .map(|author| ListItemId {
                            is_selected: true,
                            id: author.id.to_string(),
                            name: author.name.to_string(),
                        })
                        .collect(),
                )
            },
            state: ListState::default(),
            search_bar: Input::default(),
            _is_found: true,
            _state: PhantomData,
        }
    }
}

impl FilterListDynamic<ArtistState> {
    fn from_artist(artists: &User<ArtistFilterState>) -> Self {
        Self {
            items: if artists.is_empty() {
                None
            } else {
                Some(
                    artists
                        .iter()
                        .map(|artist| ListItemId {
                            is_selected: true,
                            id: artist.id.to_string(),
                            name: artist.name.to_string(),
                        })
                        .collect(),
                )
            },
            state: ListState::default(),
            search_bar: Input::default(),
            _is_found: true,
            _state: PhantomData,
        }
    }
}

impl SendEventOnSuccess for AuthorState {
    fn send(data: Option<AuthorsResponse>) -> FilterEvents {
        FilterEvents::LoadAuthors(data)
    }
}

impl<T: SendEventOnSuccess> FilterListDynamic<T> {
    pub fn search_items(&self, tx: UnboundedSender<FilterEvents>, client: MangadexClient) {
        let name_to_search = SearchTerm::trimmed_lowercased(self.search_bar.value());
        if let Some(search_term) = name_to_search {
            tokio::spawn(async move {
                let response = client.get_authors(search_term).await;
                if let Ok(res) = response {
                    tx.send(T::send(res.json().await.ok())).ok();
                }
            });
        }
    }

    pub fn set_users_found(&mut self, response: AuthorsResponse) {
        self.items = Some(
            response
                .data
                .into_iter()
                .map(|data| ListItemId {
                    is_selected: false,
                    id: data.id,
                    name: data.attributes.name,
                })
                .collect(),
        );
    }

    fn set_users_not_found(&mut self) {
        self.items = None;
    }

    fn toggle(&mut self) {
        if let Some(items) = self.items.as_mut() {
            if let Some(index) = self.state.selected() {
                if let Some(user_selected) = items.get_mut(index) {
                    user_selected.is_selected = !user_selected.is_selected;
                }
            }
        }
    }

    pub fn num_filters_active(&self) -> usize {
        match &self.items {
            Some(tags) => tags.iter().filter(|item| item.is_selected).count(),
            None => 0,
        }
    }

    fn load_users(&mut self, maybe_user: Option<AuthorsResponse>) {
        match maybe_user {
            Some(user) => {
                if user.data.is_empty() {
                    self.set_users_not_found();
                } else {
                    self.set_users_found(user);
                }
            },
            None => {
                self.set_users_not_found();
            },
        }
    }
}

#[derive(Default, PartialEq, Eq, Clone, Debug)]
pub enum TagListItemState {
    Included,
    Excluded,
    #[default]
    NotSelected,
}

impl From<TagSelection> for TagListItemState {
    fn from(value: TagSelection) -> Self {
        match value {
            TagSelection::Included => Self::Included,
            TagSelection::Excluded => Self::Excluded,
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct TagListItem {
    pub id: String,
    pub name: String,
    pub state: TagListItemState,
}

impl TagListItem {
    pub fn toggle_include(&mut self) {
        match self.state {
            TagListItemState::NotSelected | TagListItemState::Excluded => {
                self.state = TagListItemState::Included;
            },
            TagListItemState::Included => {
                self.state = TagListItemState::NotSelected;
            },
        }
    }

    pub fn set_filter_tags_style(&self) -> Span<'_> {
        match self.state {
            TagListItemState::Included => format!(" {} ", self.name).black().on_green(),
            TagListItemState::Excluded => format!(" {} ", self.name).black().on_red(),
            TagListItemState::NotSelected => Span::from(self.name.clone()),
        }
    }

    pub fn toggle_exclude(&mut self) {
        match self.state {
            TagListItemState::NotSelected | TagListItemState::Included => {
                self.state = TagListItemState::Excluded;
            },
            TagListItemState::Excluded => {
                self.state = TagListItemState::NotSelected;
            },
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct TagsState {
    pub tags: Option<Vec<TagListItem>>,
    pub state: ListState,
    pub filter_input: Input,
}

pub struct TagsStateIter<'a> {
    tags: Option<&'a [TagListItem]>,
    current: usize,
}

impl<'a> TagsStateIter<'a> {
    pub fn new(tags: Option<&'a [TagListItem]>) -> Self {
        Self { tags, current: 0 }
    }
}

impl<'a> Iterator for TagsStateIter<'a> {
    type Item = &'a TagListItem;

    fn next(&mut self) -> Option<Self::Item> {
        self.tags.as_ref().and_then(|tags| {
            let next = tags.get(self.current);
            self.current += 1;
            next
        })
    }
}

impl From<&Tags> for TagsState {
    fn from(value: &Tags) -> Self {
        Self {
            tags: if value.is_empty() { None } else { Some(value.iter().map(TagListItem::from).collect()) },
            ..Default::default()
        }
    }
}

impl TagsState {
    pub fn iter(&self) -> TagsStateIter<'_> {
        TagsStateIter::new(self.tags.as_deref())
    }

    pub fn num_filters_active(&self) -> usize {
        self.tags.as_ref().map_or(0, |tags| {
            tags.iter()
                .filter(|tag| tag.state == TagListItemState::Included || tag.state == TagListItemState::Excluded)
                .count()
        })
    }

    pub fn is_filter_empty(&mut self) -> bool {
        self.filter_input.value().trim().is_empty()
    }

    pub fn get_selected_tag(&mut self) -> Option<&mut TagListItem> {
        if let Some(tags) = self.tags.as_mut() {
            if let Some(index) = self.state.selected() {
                return tags.get_mut(index);
            }
            None
        } else {
            None
        }
    }

    pub fn get_filtered_tags(&mut self) -> Vec<&mut TagListItem> {
        self.tags
            .as_mut()
            .unwrap()
            .iter_mut()
            .filter(|tag| tag.name.to_lowercase().contains(&self.filter_input.value().to_lowercase()))
            .collect()
    }

    pub fn include_tag(&mut self) {
        if self.is_filter_empty() {
            if let Some(tag) = self.get_selected_tag() {
                tag.toggle_include();
            }
        } else if self.tags.is_some() {
            if let Some(index) = self.state.selected() {
                if let Some(tag) = self.get_filtered_tags().get_mut(index) {
                    tag.toggle_include();
                }
            }
        }
    }

    pub fn exclude_tag(&mut self) {
        if self.is_filter_empty() {
            if let Some(tag) = self.get_selected_tag() {
                tag.toggle_exclude();
            }
        } else if self.tags.is_some() {
            if let Some(index) = self.state.selected() {
                if let Some(tag) = self.get_filtered_tags().get_mut(index) {
                    tag.toggle_exclude();
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct MangadexFilterProvider {
    pub is_open: bool,
    pub id_filter: usize,
    pub filters: Filters,
    pub content_rating: FilterList<ContentRatingState>,
    pub publication_status: FilterList<PublicationStatusState>,
    pub sort_by_state: FilterList<SortByState>,
    pub magazine_demographic: FilterList<MagazineDemographicState>,
    already_existings_tags: Option<Tags>,
    pub tags_state: TagsState,
    pub author_state: FilterListDynamic<AuthorState>,
    pub artist_state: FilterListDynamic<ArtistState>,
    pub lang_state: FilterList<LanguageState>,
    pub is_typing: bool,
    api_client: MangadexClient,
    tx: UnboundedSender<FilterEvents>,
    rx: UnboundedReceiver<FilterEvents>,
}

impl From<Filters> for MangadexFilterProvider {
    fn from(filters: Filters) -> Self {
        let (tx, rx) = mpsc::unbounded_channel::<FilterEvents>();
        tx.send(FilterEvents::SearchTags).ok();

        let already_existings_tags = if filters.tags.is_empty() { None } else { Some(filters.tags.clone()) };

        Self {
            is_open: false,
            id_filter: 0,
            content_rating: FilterList::<ContentRatingState>::from_content_ratings(&filters.content_rating),
            sort_by_state: FilterList::<SortByState>::from_sort_by(&filters.sort_by),
            publication_status: FilterList::<PublicationStatusState>::from_publication_status(&filters.publication_status),
            tags_state: TagsState::from(&filters.tags),
            magazine_demographic: FilterList::<MagazineDemographicState>::from_magazine_demographic(&filters.magazine_demographic),
            author_state: FilterListDynamic::<AuthorState>::from_authors(&filters.authors),
            artist_state: FilterListDynamic::<ArtistState>::from_artist(&filters.artists),
            lang_state: FilterList::<LanguageState>::from_languages(filters.languages.as_ref()),
            already_existings_tags,
            api_client: MangadexClient::new(
                API_URL_BASE.parse().unwrap(),
                COVER_IMG_URL_BASE.parse().unwrap(),
                InMemoryCache::init(2),
            ),
            is_typing: false,
            tx,
            rx,
            filters,
        }
    }
}

impl FiltersEventHandler for MangadexFilterProvider {
    fn handle_events(&mut self, events: Events) {
        match events {
            Events::Key(key_event) => self.handle_key_events(key_event),
            Events::Tick => self.tick(),
            _ => {},
        }
    }
}

impl FiltersHandler for MangadexFilterProvider {
    type InnerState = Filters;

    fn toggle(&mut self) {
        if self.is_open {
            self.save_filters_on_close();
        }

        self.is_open = !self.is_open;
    }

    #[inline]
    fn is_typing(&self) -> bool {
        self.is_typing
    }

    #[inline]
    fn is_open(&self) -> bool {
        self.is_open
    }

    #[inline]
    fn get_state(&self) -> &Self::InnerState {
        &self.filters
    }
}

impl MangadexFilterProvider {
    fn save_filters_on_close(&self) {
        let filters_cache_writer = FiltersCache::new(&*MANGADEX_CACHE_BASE_DIRECTORY, MANGADEX_CACHE_FILENAME);

        filters_cache_writer
            .write_to_cache(&self.filters)
            .inspect_err(|e| {
                #[cfg(not(test))]
                {
                    use crate::backend::error_log::{ErrorType, write_to_error_log};

                    write_to_error_log(ErrorType::String(&e.to_string()));
                }
            })
            .ok();
    }

    pub fn reset(&mut self) {
        if self.tags_state.tags.is_some() {
            self.tags_state
                .tags
                .as_mut()
                .unwrap()
                .iter_mut()
                .for_each(|tag| tag.state = TagListItemState::NotSelected);
            self.tags_state.filter_input.reset();
        }

        self.filters = Filters::default();
        self.content_rating = FilterList::<ContentRatingState>::default();
        self.publication_status = FilterList::<PublicationStatusState>::default();
        self.magazine_demographic = FilterList::<MagazineDemographicState>::default();
        self.sort_by_state = FilterList::<SortByState>::default();
        self.lang_state = FilterList::<LanguageState>::default();
        self.author_state = FilterListDynamic::<AuthorState>::default();
        self.artist_state = FilterListDynamic::<ArtistState>::default();
    }

    fn tick(&mut self) {
        if let Ok(event) = self.rx.try_recv() {
            match event {
                FilterEvents::SearchTags => self.search_tags(),
                FilterEvents::LoadTags(res) => self.load_tags(res),
                FilterEvents::LoadAuthors(res) => self.author_state.load_users(res),
                FilterEvents::LoadArtists(res) => self.artist_state.load_users(res),
            }
        }
    }

    fn search_tags(&mut self) {
        let tx = self.tx.clone();
        let client = self.api_client.clone();
        tokio::spawn(async move {
            let response = client.get_tags().await;
            if let Ok(res) = response {
                if let Ok(tags) = res.json().await {
                    tx.send(FilterEvents::LoadTags(tags)).ok();
                }
            }
        });
    }

    fn load_tags(&mut self, response: TagsResponse) {
        self.set_tags_from_response(response);
    }

    fn search(&mut self) {
        if let Some(filter) = FILTERS.get(self.id_filter) {
            let tx = self.tx.clone();
            let client = self.api_client.clone();
            if *filter == MangaFilters::Authors {
                self.author_state.search_items(tx, client);
            } else if *filter == MangaFilters::Artists {
                self.artist_state.search_items(tx, client);
            }
        }
    }

    pub fn handle_key_events(&mut self, key_event: KeyEvent) {
        if self.is_typing {
            match key_event.code {
                KeyCode::Esc | KeyCode::Left => self.toggle_focus_input(),
                KeyCode::Enter => self.search(),

                _ => self.handle_key_events_for_input(key_event),
            }
        } else {
            match key_event.code {
                KeyCode::Char('f') => self.toggle(),
                KeyCode::Esc => self.toggle(),
                KeyCode::Char('j') | KeyCode::Down => self.scroll_down_filter_list(),
                KeyCode::Char('k') | KeyCode::Up => self.scroll_up_filter_list(),
                KeyCode::Tab => self.next_filter(),
                KeyCode::BackTab => self.previous_filter(),
                KeyCode::Char('s') => self.toggle_filter_list(),
                KeyCode::Char('d') => {
                    if *FILTERS.get(self.id_filter).unwrap() == MangaFilters::Tags {
                        self.exclude_tag_selected();
                    }
                },
                KeyCode::Char('r') => self.reset(),
                KeyCode::Char('l') | KeyCode::Right => self.toggle_focus_input(),
                _ => {},
            }
        }
    }

    fn handle_key_events_for_input(&mut self, key_event: KeyEvent) {
        if let Some(filter) = FILTERS.get(self.id_filter) {
            match filter {
                MangaFilters::Tags => {
                    self.tags_state.filter_input.handle_event(&crossterm::event::Event::Key(key_event));
                },
                MangaFilters::Authors => {
                    self.author_state.search_bar.handle_event(&crossterm::event::Event::Key(key_event));
                },
                MangaFilters::Artists => {
                    self.artist_state.search_bar.handle_event(&crossterm::event::Event::Key(key_event));
                },
                _ => {},
            }
        }
    }

    fn toggle_focus_input(&mut self) {
        match FILTERS.get(self.id_filter).unwrap() {
            MangaFilters::Tags | MangaFilters::Authors | MangaFilters::Artists => {
                self.is_typing = !self.is_typing;
            },
            _ => {},
        }
    }

    fn next_filter(&mut self) {
        if self.id_filter + 1 < FILTERS.len() {
            self.id_filter += 1;
        } else {
            self.id_filter = 0;
        }
    }

    fn previous_filter(&mut self) {
        if self.id_filter == 0 {
            self.id_filter = FILTERS.len() - 1;
        } else {
            self.id_filter = self.id_filter.saturating_sub(1);
        }
    }

    fn scroll_down_filter_list(&mut self) {
        if let Some(filter) = FILTERS.get(self.id_filter) {
            match filter {
                MangaFilters::ContentRating => {
                    self.content_rating.scroll_down();
                },
                MangaFilters::SortBy => {
                    self.sort_by_state.scroll_down();
                },
                MangaFilters::Tags => {
                    if self.tags_state.tags.is_some() {
                        self.tags_state.state.select_next();
                    }
                },
                MangaFilters::MagazineDemographic => {
                    self.magazine_demographic.scroll_down();
                },
                MangaFilters::Authors => {
                    if self.author_state.items.is_some() {
                        self.author_state.state.select_next();
                    }
                },
                MangaFilters::Artists => {
                    if self.artist_state.items.is_some() {
                        self.artist_state.state.select_next();
                    }
                },
                MangaFilters::Languages => {
                    self.lang_state.scroll_down();
                },
                MangaFilters::PublicationStatus => {
                    self.publication_status.scroll_down();
                },
            }
        }
    }

    fn scroll_up_filter_list(&mut self) {
        if let Some(filter) = FILTERS.get(self.id_filter) {
            match filter {
                MangaFilters::ContentRating => {
                    self.content_rating.scroll_up();
                },
                MangaFilters::SortBy => {
                    self.sort_by_state.scroll_up();
                },
                MangaFilters::Tags => {
                    if self.tags_state.tags.is_some() {
                        self.tags_state.state.select_previous();
                    }
                },
                MangaFilters::MagazineDemographic => {
                    self.magazine_demographic.scroll_up();
                },
                MangaFilters::Authors => {
                    if self.author_state.items.is_some() {
                        self.author_state.state.select_previous();
                    }
                },
                MangaFilters::Artists => {
                    if self.artist_state.items.is_some() {
                        self.artist_state.state.select_previous();
                    }
                },

                MangaFilters::Languages => {
                    self.lang_state.scroll_up();
                },
                MangaFilters::PublicationStatus => {
                    self.publication_status.scroll_up();
                },
            }
        }
    }

    fn toggle_filter_list(&mut self) {
        if let Some(filter) = FILTERS.get(self.id_filter) {
            match filter {
                MangaFilters::ContentRating => {
                    self.content_rating.toggle();
                    self.set_content_rating();
                },
                MangaFilters::SortBy => {
                    self.sort_by_state.toggle_sort_by();
                    self.set_sort_by();
                },
                MangaFilters::Tags => {
                    self.include_tag_selected();
                },
                MangaFilters::MagazineDemographic => {
                    self.magazine_demographic.toggle();
                    self.set_magazine_demographic();
                },
                MangaFilters::Authors => {
                    self.author_state.toggle();
                    self.set_authors();
                },
                MangaFilters::Artists => {
                    self.artist_state.toggle();
                    self.set_artists();
                },

                MangaFilters::Languages => {
                    self.lang_state.toggle();
                    self.set_languages();
                },
                MangaFilters::PublicationStatus => {
                    self.publication_status.toggle();
                    self.set_publication_status();
                },
            }
        }
    }

    fn include_tag_selected(&mut self) {
        self.tags_state.include_tag();
        self.set_tags();
    }

    fn exclude_tag_selected(&mut self) {
        self.tags_state.exclude_tag();
        self.set_tags();
    }

    fn set_publication_status(&mut self) {
        self.filters.set_publication_status(
            self.publication_status
                .items
                .iter()
                .filter_map(|item| {
                    if item.is_selected {
                        return Some(item.name.as_str().into());
                    }
                    None
                })
                .collect(),
        )
    }

    pub fn set_tags_from_response(&mut self, tags_response: TagsResponse) {
        let tags: Vec<TagListItem> = tags_response
            .data
            .into_iter()
            .map(|data| TagListItem {
                id: data.id.to_string(),
                name: data.attributes.name.en,
                state: self
                    .already_existings_tags
                    .as_ref()
                    .and_then(|tags| {
                        let found_tag = tags.iter().find(|tag| tag.id == data.id);

                        found_tag.map(|existing_tag| TagListItemState::from(existing_tag.state))
                    })
                    .unwrap_or_default(),
            })
            .collect();

        self.tags_state.tags = Some(tags);
    }

    fn set_tags(&mut self) {
        if let Some(tags) = self.tags_state.tags.as_ref() {
            let tag_ids: Vec<TagData> = tags
                .iter()
                .filter_map(|tag| {
                    if tag.state != TagListItemState::NotSelected {
                        return Some(TagData::from(tag));
                    }
                    None
                })
                .collect();

            self.filters.set_tags(tag_ids);
        }
    }

    fn set_sort_by(&mut self) {
        let sort_by_selected = self.sort_by_state.items.iter().find(|item| item.is_selected);

        if let Some(sort_by) = sort_by_selected {
            self.filters.set_sort_by(sort_by.name.as_str().into());
        }
    }

    fn set_content_rating(&mut self) {
        self.filters.set_content_rating(
            self.content_rating
                .items
                .iter()
                .filter_map(|item| {
                    if item.is_selected {
                        return Some(item.name.as_str().into());
                    }
                    None
                })
                .collect(),
        )
    }

    fn set_magazine_demographic(&mut self) {
        self.filters.set_magazine_demographic(
            self.magazine_demographic
                .items
                .iter()
                .filter_map(|item| {
                    if item.is_selected {
                        return Some(item.name.as_str().into());
                    }
                    None
                })
                .collect(),
        )
    }

    fn set_authors(&mut self) {
        if let Some(authors) = self.author_state.items.as_ref() {
            self.filters.set_authors(
                authors
                    .iter()
                    .filter_map(|item| {
                        if item.is_selected {
                            return Some(AuthorFilterState::new(item.id.to_string(), item.name.clone()));
                        }
                        None
                    })
                    .collect(),
            )
        }
    }

    fn set_artists(&mut self) {
        if let Some(artists) = self.artist_state.items.as_ref() {
            self.filters.set_artists(
                artists
                    .iter()
                    .filter_map(|item| {
                        if item.is_selected {
                            return Some(ArtistFilterState::new(item.id.to_string()).with_name(&item.name));
                        }
                        None
                    })
                    .collect(),
            )
        }
    }

    fn set_languages(&mut self) {
        self.filters.set_languages(
            self.lang_state
                .items
                .iter()
                .filter_map(|item| {
                    if item.is_selected {
                        return Some(item.clone().into());
                    }
                    None
                })
                .collect(),
        )
    }

    // This function is called from manga page
    // Deprecated functionality but maybe re-implemented in the future
    //pub fn set_author(&mut self, author: Author) {
    //    self.filters.reset_author();
    //    self.filters.reset_artist();
    //    self.artist_state.items = None;
    //    self.author_state.items = Some(vec![ListItemId {
    //        id: author.id.clone(),
    //        is_selected: true,
    //        name: author.name,
    //    }]);
    //    self.filters.authors.set_one_user(AuthorFilterState::new(author.id))
    //}
    //
    ///// This function is called from manga page
    //pub fn set_artist(&mut self, artist: Artist) {
    //    self.filters.reset_author();
    //    self.filters.reset_artist();
    //
    //    self.author_state.items = None;
    //    self.artist_state.items = Some(vec![ListItemId {
    //        id: artist.id.clone(),
    //        is_selected: true,
    //        name: artist.name,
    //    }]);
    //    self.filters.artists.set_one_user(ArtistFilterState::new(artist.id))
    //}
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::backend::manga_provider::mangadex::authors::Data;
    use crate::backend::manga_provider::mangadex::tags::TagsData;

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
    fn filter_list_works() {
        let mut filter_list: FilterList<MagazineDemographicState> = FilterList::default();

        filter_list.scroll_down();

        filter_list.toggle();

        assert_eq!(Some(0), filter_list.state.selected());

        assert!(filter_list.items.iter().any(|item| item.is_selected));

        assert_eq!(1, filter_list.num_filters_active());

        filter_list.scroll_down();

        filter_list.toggle();

        assert_eq!(Some(1), filter_list.state.selected());

        assert_eq!(2, filter_list.num_filters_active());

        filter_list.scroll_up();

        assert_eq!(Some(0), filter_list.state.selected());
    }

    #[test]
    fn language_filter_list_works() {
        let filter_list: FilterList<LanguageState> = FilterList::default();

        assert_eq!(
            Languages::default(),
            filter_list
                .items
                .iter()
                .find(|filter_list_item| filter_list_item.is_selected)
                .cloned()
                .unwrap()
                .into()
        );

        let language_items: Vec<Languages> =
            filter_list.items.into_iter().map(|filter_list_item| filter_list_item.into()).collect();

        assert!(!language_items.contains(&Languages::Unkown));
    }

    #[test]
    fn sort_by_state_works() {
        let mut filter_list: FilterList<SortByState> = FilterList::default();

        filter_list.scroll_down();
        filter_list.toggle_sort_by();
        filter_list.scroll_down();
        filter_list.toggle_sort_by();

        // for the sort_by filter only one can be selected at a time
        assert_eq!(1, filter_list.num_filters_active());
    }

    #[test]
    fn filter_list_dynamic_works() {
        let mut filter_list: FilterListDynamic<AuthorState> = FilterListDynamic::default();

        let mock_response = AuthorsResponse {
            data: vec![Data::default()],
            ..Default::default()
        };

        assert_eq!(0, filter_list.num_filters_active());

        filter_list.toggle();

        filter_list.load_users(Some(mock_response));

        assert!(filter_list.items.is_some());

        filter_list.state.select_next();

        filter_list.toggle();

        assert!(filter_list.items.as_ref().is_some_and(|items| items.iter().any(|item| item.is_selected)));

        filter_list.load_users(Some(AuthorsResponse::default()));

        assert!(filter_list.items.is_none());
    }

    #[test]
    fn tag_state_works() {
        let mut tag_state = TagsState {
            tags: Some(vec![TagListItem::default(), TagListItem::default()]),
            ..Default::default()
        };

        tag_state.state.select_next();

        tag_state.include_tag();

        assert!(
            tag_state
                .tags
                .as_ref()
                .is_some_and(|tags| tags.iter().any(|tag| tag.state == TagListItemState::Included))
        );

        tag_state.exclude_tag();

        assert!(
            tag_state
                .tags
                .as_ref()
                .is_some_and(|tags| tags.iter().any(|tag| tag.state == TagListItemState::Excluded))
        );
    }

    // simulate what the user can do
    fn next_tab(filter_state: &mut MangadexFilterProvider) {
        filter_state.handle_events(Events::Key(KeyCode::Tab.into()));
    }

    fn previous_tab(filter_state: &mut MangadexFilterProvider) {
        filter_state.handle_events(Events::Key(KeyCode::BackTab.into()));
    }

    fn scroll_down(filter_state: &mut MangadexFilterProvider) {
        filter_state.handle_events(Events::Key(KeyCode::Char('j').into()));
    }

    fn press_s(filter_state: &mut MangadexFilterProvider) {
        filter_state.handle_events(Events::Key(KeyCode::Char('s').into()));
    }

    fn start_typing(filter_state: &mut MangadexFilterProvider) {
        filter_state.handle_events(Events::Key(KeyCode::Char('l').into()));
    }

    fn type_a_letter(filter_state: &mut MangadexFilterProvider, character: char) {
        filter_state.handle_events(Events::Key(KeyCode::Char(character).into()));
    }

    // this action both sets fillter_state.is_open = false and unfocus search_bar input
    fn close_filter(filter_state: &mut MangadexFilterProvider) {
        filter_state.handle_events(Events::Key(KeyCode::Esc.into()));
    }

    #[test]
    fn filter_state() {
        let mut filter_state = MangadexFilterProvider::from(Filters::default());

        filter_state.is_open = true;

        let mock_response = TagsResponse {
            data: vec![TagsData::default(), TagsData::default()],
            ..Default::default()
        };

        filter_state.set_tags_from_response(mock_response);

        assert!(filter_state.tags_state.tags.is_some());

        // Go to magazine demographic
        previous_tab(&mut filter_state);
        previous_tab(&mut filter_state);
        previous_tab(&mut filter_state);

        scroll_down(&mut filter_state);
        press_s(&mut filter_state);

        assert!(filter_state.magazine_demographic.state.selected().is_some());

        assert!(filter_state.magazine_demographic.items.iter().any(|item| item.is_selected));

        // Go to tags
        previous_tab(&mut filter_state);

        scroll_down(&mut filter_state);
        press_s(&mut filter_state);

        assert!(
            filter_state
                .tags_state
                .tags
                .as_ref()
                .is_some_and(|tags| tags.iter().any(|tag| tag.state == TagListItemState::Included))
        );

        assert!(!filter_state.filters.tags.is_empty());

        // Go to Publication status
        previous_tab(&mut filter_state);
        scroll_down(&mut filter_state);
        press_s(&mut filter_state);

        assert!(filter_state.publication_status.state.selected().is_some());
        assert!(filter_state.publication_status.items.iter().any(|item| item.is_selected));

        // Go to tags
        next_tab(&mut filter_state);
        start_typing(&mut filter_state);

        assert!(filter_state.is_typing);

        type_a_letter(&mut filter_state, 't');
        type_a_letter(&mut filter_state, 'e');
        type_a_letter(&mut filter_state, 's');
        type_a_letter(&mut filter_state, 't');
        assert_eq!("test", filter_state.tags_state.filter_input.value());

        // First unfocus the filter bar
        close_filter(&mut filter_state);

        // then "close" the filter
        close_filter(&mut filter_state);

        assert!(!filter_state.is_open);
    }

    #[test]
    fn filter_provider_is_initialized_from_filters() {
        let filters: Filters = Filters {
            content_rating: vec![ContentRating::Suggestive, ContentRating::Erotic],
            publication_status: vec![PublicationStatus::Completed, PublicationStatus::Ongoing],
            sort_by: SortBy::HighestRating,
            tags: Tags::new(vec![TagData::new("id_tag".to_string(), TagSelection::Included, "fantasy".to_string())]),
            magazine_demographic: vec![MagazineDemographic::Shoujo, MagazineDemographic::Seinen],
            authors: User::new(vec![AuthorFilterState::new("author_id".to_string(), "name_author".to_string())]),
            artists: User::new(vec![ArtistFilterState::new("artist_id".to_string()).with_name("artist_name")]),
            languages: vec![Languages::English, Languages::Spanish],
        };

        let filters_provider = MangadexFilterProvider::from(filters);

        let expected_content_rating: FilterList<ContentRatingState> = FilterList {
            items: vec![
                FilterListItem {
                    is_selected: false,
                    name: ContentRating::Safe.to_string(),
                },
                FilterListItem {
                    is_selected: true,
                    name: ContentRating::Suggestive.to_string(),
                },
                FilterListItem {
                    is_selected: true,
                    name: ContentRating::Erotic.to_string(),
                },
                FilterListItem {
                    is_selected: false,
                    name: ContentRating::Pornographic.to_string(),
                },
            ],
            state: ListState::default(),
            _state: PhantomData,
        };

        assert_eq!(expected_content_rating, filters_provider.content_rating);

        filters_provider
            .sort_by_state
            .iter()
            .find(|item| item.is_selected && item.name == SortBy::HighestRating.to_string())
            .expect("sort_by state is not the one that should be selected");

        let num_publication_status_expected = filters_provider
            .publication_status
            .iter()
            .filter_map(|item| {
                if item.is_selected
                    && (item.name == PublicationStatus::Ongoing.to_string()
                        || item.name == PublicationStatus::Completed.to_string())
                {
                    Some(item)
                } else {
                    None
                }
            })
            .count();

        assert_eq!(num_publication_status_expected, 2);

        let num_languages_expected = filters_provider
            .lang_state
            .iter()
            .filter_map(|lan| lan.is_selected.then_some(lan))
            .count();

        assert_eq!(num_languages_expected, 2);

        filters_provider
            .tags_state
            .iter()
            .find(|tag| tag.id == "id_tag")
            .expect("tag state was not initialized correctly");

        let num_magazine_demographic_expected = filters_provider
            .magazine_demographic
            .iter()
            .filter_map(|magazine| magazine.is_selected.then_some(magazine))
            .count();

        assert_eq!(num_magazine_demographic_expected, 2);

        filters_provider
            .artist_state
            .items
            .as_ref()
            .unwrap()
            .iter()
            .find(|artist| artist.id == "artist_id" && artist.name == "artist_name")
            .expect("Expected artist was not found");

        filters_provider
            .author_state
            .items
            .as_ref()
            .unwrap()
            .iter()
            .find(|author| author.id == "author_id" && author.name == "name_author")
            .expect("Expected author was not found");
    }
}
