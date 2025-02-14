use std::fmt::{Debug, Write};
use std::marker::PhantomData;

use crossterm::event::{KeyCode, KeyEvent};
use manga_tui::SearchTerm;
use ratatui::widgets::*;
use strum::{Display, EnumIter, IntoEnumIterator};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

use super::{API_URL_BASE, COVER_IMG_URL_BASE};
use crate::backend::manga_provider::mangadex::api_responses::authors::AuthorsResponse;
use crate::backend::manga_provider::mangadex::api_responses::tags::TagsResponse;
use crate::backend::manga_provider::mangadex::MangadexClient;
use crate::backend::manga_provider::{Artist, Author, EventHandler as FiltersEventHandler, FiltersHandler, Languages};
use crate::backend::tui::Events;

pub trait IntoParam: Debug {
    fn into_param(self) -> String;
}

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

#[derive(Clone, Debug)]
pub struct FilterListItem {
    pub is_selected: bool,
    pub name: String,
}

impl FilterListItem {
    pub fn toggle(&mut self) {
        self.is_selected = !self.is_selected;
    }
}

#[derive(Debug)]
pub struct ContentRatingState;
#[derive(Debug)]
pub struct PublicationStatusState;
#[derive(Debug)]
pub struct SortByState;
#[derive(Debug)]
pub struct MagazineDemographicState;
#[derive(Debug)]
pub struct LanguageState;

#[derive(Debug)]
pub struct FilterList<T> {
    pub items: Vec<FilterListItem>,
    pub state: ListState,
    _state: PhantomData<T>,
}

impl<T> FilterList<T> {
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
            items: vec![
                FilterListItem {
                    is_selected: true,
                    name: ContentRating::Safe.to_string(),
                },
                FilterListItem {
                    is_selected: false,
                    name: ContentRating::Suggestive.to_string(),
                },
                FilterListItem {
                    is_selected: false,
                    name: ContentRating::Erotic.to_string(),
                },
                FilterListItem {
                    is_selected: false,
                    name: ContentRating::Pornographic.to_string(),
                },
            ],
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

impl TagsState {
    pub fn num_filters_active(&self) -> usize {
        match self.tags.as_ref() {
            Some(tags) => tags
                .iter()
                .filter(|tag| tag.state == TagListItemState::Included || tag.state == TagListItemState::Excluded)
                .count(),
            None => 0,
        }
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
    pub tags_state: TagsState,
    pub author_state: FilterListDynamic<AuthorState>,
    pub artist_state: FilterListDynamic<ArtistState>,
    pub lang_state: FilterList<LanguageState>,
    pub is_typing: bool,
    api_client: MangadexClient,
    tx: UnboundedSender<FilterEvents>,
    rx: UnboundedReceiver<FilterEvents>,
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
        self.is_open = !self.is_open;
    }

    fn is_typing(&self) -> bool {
        self.is_typing
    }

    fn is_open(&self) -> bool {
        self.is_open
    }

    fn get_state(&self) -> &Self::InnerState {
        &self.filters
    }
}

impl MangadexFilterProvider {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel::<FilterEvents>();
        tx.send(FilterEvents::SearchTags).ok();
        Self {
            is_open: false,
            id_filter: 0,
            filters: Filters::default(),
            content_rating: FilterList::<ContentRatingState>::default(),
            publication_status: FilterList::<PublicationStatusState>::default(),
            sort_by_state: FilterList::<SortByState>::default(),
            tags_state: TagsState::default(),
            magazine_demographic: FilterList::<MagazineDemographicState>::default(),
            author_state: FilterListDynamic::<AuthorState>::default(),
            artist_state: FilterListDynamic::<ArtistState>::default(),
            lang_state: FilterList::<LanguageState>::default(),
            api_client: MangadexClient::new(API_URL_BASE.parse().unwrap(), COVER_IMG_URL_BASE.parse().unwrap()),
            is_typing: false,
            tx,
            rx,
        }
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
                id: data.id,
                name: data.attributes.name.en,
                state: TagListItemState::default(),
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
                            return Some(AuthorFilterState::new(item.id.to_string()));
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
                            return Some(ArtistFilterState::new(item.id.to_string()));
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

    /// This function is called from manga page
    pub fn set_author(&mut self, author: Author) {
        self.filters.reset_author();
        self.filters.reset_artist();
        self.artist_state.items = None;
        self.author_state.items = Some(vec![ListItemId {
            id: author.id.clone(),
            is_selected: true,
            name: author.name,
        }]);
        self.filters.authors.set_one_user(AuthorFilterState::new(author.id))
    }

    /// This function is called from manga page
    pub fn set_artist(&mut self, artist: Artist) {
        self.filters.reset_author();
        self.filters.reset_artist();

        self.author_state.items = None;
        self.artist_state.items = Some(vec![ListItemId {
            id: artist.id.clone(),
            is_selected: true,
            name: artist.name,
        }]);
        self.filters.artists.set_one_user(ArtistFilterState::new(artist.id))
    }
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
pub struct AuthorFilterState(String);

impl AuthorFilterState {
    pub fn new(id_author: String) -> Self {
        AuthorFilterState(id_author)
    }
}

#[derive(Default, Clone, Debug)]
pub struct ArtistFilterState(String);

impl ArtistFilterState {
    pub fn new(id_artist: String) -> Self {
        ArtistFilterState(id_artist)
    }
}

#[derive(Default, Clone, Debug)]
pub struct User<T: Clone + Default>(pub Vec<T>);

impl IntoParam for User<AuthorFilterState> {
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

impl IntoParam for User<ArtistFilterState> {
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
    pub authors: User<AuthorFilterState>,
    pub artists: User<ArtistFilterState>,
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
            content_rating: vec![ContentRating::Safe],
            publication_status: vec![],
            sort_by: SortBy::default(),
            tags: Tags(vec![]),
            magazine_demographic: vec![],
            authors: User::<AuthorFilterState>::default(),
            artists: User::<ArtistFilterState>::default(),
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

    pub fn set_authors(&mut self, author_ids: Vec<AuthorFilterState>) {
        self.authors.0 = author_ids;
    }

    pub fn set_artists(&mut self, artist_ids: Vec<ArtistFilterState>) {
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
        let sample_artists: Vec<ArtistFilterState> =
            vec![ArtistFilterState::new("id_artist1".to_string()), ArtistFilterState::new("id_artist2".to_string())];
        let filter_artist = User::<ArtistFilterState>::new(sample_artists);
        assert_eq!("&artists[]=id_artist1&artists[]=id_artist2", filter_artist.into_param());
    }

    #[test]
    fn filter_by_author_works() {
        let sample_authors: Vec<AuthorFilterState> =
            vec![AuthorFilterState::new("id_author1".to_string()), AuthorFilterState::new("id_author2".to_string())];
        let filter_artist = User::<AuthorFilterState>::new(sample_authors);
        assert_eq!("&authors[]=id_author1&authors[]=id_author2", filter_artist.into_param());
    }

    #[test]
    fn filter_by_language_works() {
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
            "&availableTranslatedLanguage[]=en&contentRating[]=safe&order[latestUploadedChapter]=desc",
            filters.into_param()
        );

        let mut filters = Filters::default();

        filters.set_tags(vec![TagData::new("id_1".to_string(), TagSelection::Included)]);

        filters.set_authors(vec![AuthorFilterState::new("id_1".to_string()), AuthorFilterState::new("id_2".to_string())]);

        filters.set_languages(vec![Languages::French, Languages::Spanish]);

        assert_eq!(
            "&authors[]=id_1&authors[]=id_2&availableTranslatedLanguage[]=fr&availableTranslatedLanguage[]=es&includedTags[]=id_1&contentRating[]=safe&order[latestUploadedChapter]=desc",
            filters.into_param()
        );
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

        assert!(!language_items.iter().any(|lang| *lang == Languages::Unkown));
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
        let mut filter_state = MangadexFilterProvider::new();

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

    //#[tokio::test]
    //async fn search_authors_sends_expected_event() {
    //    let (tx, mut rx) = unbounded_channel::<FilterEvents>();
    //    let mut filter_state_author: FilterListDynamic<AuthorState> = FilterListDynamic::default();
    //
    //    let client = MockMangadexClient::new();
    //    filter_state_author.set_search_term("some thing");
    //
    //    filter_state_author.search_items(tx, client);
    //
    //    let event_sent = rx.recv().await.expect("no event was sent");
    //
    //    let expected = FilterEvents::LoadAuthors(Some(AuthorsResponse::default()));
    //
    //    assert_eq!(event_sent, expected);
    //}
    //
    //#[tokio::test]
    //async fn search_artist_sends_expected_event() {
    //    let (tx, mut rx) = unbounded_channel::<FilterEvents>();
    //    let mut filter_state_artist: FilterListDynamic<ArtistState> = FilterListDynamic::default();
    //
    //    let client = MockMangadexClient::new();
    //    filter_state_artist.set_search_term("some thing");
    //
    //    filter_state_artist.search_items(tx, client);
    //
    //    let event_sent = rx.recv().await.expect("no event was sent");
    //
    //    let expected = FilterEvents::LoadArtists(Some(AuthorsResponse::default()));
    //
    //    assert_eq!(event_sent, expected);
    //}
}
