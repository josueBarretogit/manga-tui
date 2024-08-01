use crate::backend::authors::AuthorsResponse;
use crate::backend::fetch::MangadexClient;
use crate::backend::filter::{
    Artist, Author, ContentRating, Filters, Languages, MagazineDemographic, PublicationStatus,
    SortBy,
};
use crate::backend::tags::TagsResponse;
use crate::backend::tui::Events;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::widgets::*;
use std::marker::PhantomData;
use strum::{Display, IntoEnumIterator};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

pub enum FilterEvents {
    LoadAuthors(Option<AuthorsResponse>),
    LoadArtists(Option<AuthorsResponse>),
    SearchTags,
    LoadTags(TagsResponse),
}

#[derive(Display, PartialEq, Eq)]
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

#[derive(Clone)]
pub struct FilterListItem {
    pub is_selected: bool,
    pub name: String,
}

impl FilterListItem {
    pub fn toggle(&mut self) {
        self.is_selected = !self.is_selected;
    }
}

pub struct ContentRatingState;
pub struct PublicationStatusState;
pub struct SortByState;
pub struct MagazineDemographicState;
pub struct LanguageState;

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
        if self
            .state
            .selected()
            .is_some_and(|index| index == self.items.len() - 1)
        {
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
                    is_selected: true,
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
        let items = Languages::iter()
            .filter(|lang| *lang != Languages::Unkown)
            .map(|lang| FilterListItem {
                name: format!("{} {}", lang.as_emoji(), lang.as_human_readable()),
                is_selected: false,
            });

        Self {
            items: items.collect(),
            state: ListState::default(),
            _state: PhantomData,
        }
    }
}

#[derive(Clone)]
pub struct ListItemId {
    pub id: String,
    pub name: String,
    pub is_selected: bool,
}

#[derive(Default)]
pub struct AuthorState;
#[derive(Default)]
pub struct ArtistState;
#[derive(Default)]
pub struct TagState;

// It's called dynamic because the items must be fetched
#[derive(Default)]
pub struct FilterListDynamic<T> {
    pub items: Option<Vec<ListItemId>>,
    pub state: ListState,
    pub search_bar: Input,
    pub _is_found: bool,
    _state: PhantomData<T>,
}

impl FilterListDynamic<TagState> {
    pub fn toggle_tags(&mut self) {
        if self.is_search_bar_empty() {
            self.toggle()
        } else if let Some(index) = self.state.selected() {
            if let Some(items) = self.items.as_mut() {
                if let Some(tag) = items
                    .iter_mut()
                    .filter(|tag| {
                        tag.name
                            .to_lowercase()
                            .contains(&self.search_bar.value().to_lowercase())
                    })
                    .collect::<Vec<&mut ListItemId>>()
                    .get_mut(index)
                {
                    tag.is_selected = !tag.is_selected;
                }
            }
        }
    }
}

impl FilterListDynamic<AuthorState> {
    fn search_authors(&mut self, tx: UnboundedSender<FilterEvents>) {
        let name = self.get_name();
        tokio::spawn(async move {
            let res = MangadexClient::global().get_authors(&name).await;
            tx.send(FilterEvents::LoadAuthors(res.ok())).ok();
        });
    }
}

impl FilterListDynamic<ArtistState> {
    fn search_artists(&mut self, tx: UnboundedSender<FilterEvents>) {
        let name = self.get_name();
        tokio::spawn(async move {
            let res = MangadexClient::global().get_authors(&name).await;
            tx.send(FilterEvents::LoadArtists(res.ok())).ok();
        });
    }
}

impl<T> FilterListDynamic<T> {
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

    pub fn get_name(&self) -> String {
        self.search_bar.value().trim().to_lowercase()
    }

    fn set_users_not_found(&mut self) {
        self.items = None;
    }

    pub fn is_search_bar_empty(&mut self) -> bool {
        self.search_bar.value().trim().is_empty()
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
            }
            None => {
                self.set_users_not_found();
            }
        }
    }
}

pub struct FilterState {
    pub is_open: bool,
    pub id_filter: usize,
    pub filters: Filters,
    pub content_rating: FilterList<ContentRatingState>,
    pub publication_status: FilterList<PublicationStatusState>,
    pub sort_by_state: FilterList<SortByState>,
    pub magazine_demographic: FilterList<MagazineDemographicState>,
    pub tags: FilterListDynamic<TagState>,
    pub author_state: FilterListDynamic<AuthorState>,
    pub artist_state: FilterListDynamic<ArtistState>,
    pub lang_state: FilterList<LanguageState>,
    pub is_typing: bool,
    tx: UnboundedSender<FilterEvents>,
    rx: UnboundedReceiver<FilterEvents>,
}

impl FilterState {
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
            tags: FilterListDynamic::<TagState>::default(),
            magazine_demographic: FilterList::<MagazineDemographicState>::default(),
            author_state: FilterListDynamic::<AuthorState>::default(),
            artist_state: FilterListDynamic::<ArtistState>::default(),
            lang_state: FilterList::<LanguageState>::default(),
            is_typing: false,
            tx,
            rx,
        }
    }

    pub fn reset(&mut self) {
        if self.tags.items.is_some() {
            self.tags
                .items
                .as_mut()
                .unwrap()
                .iter_mut()
                .for_each(|tag| tag.is_selected = false);
            self.tags.search_bar.reset();
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

    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
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
        tokio::spawn(async move {
            let response = MangadexClient::global().get_tags().await;
            if let Ok(res) = response {
                tx.send(FilterEvents::LoadTags(res)).ok();
            }
        });
    }

    fn load_tags(&mut self, response: TagsResponse) {
        self.set_tags_from_response(response);
    }

    fn search(&mut self) {
        if let Some(filter) = FILTERS.get(self.id_filter) {
            let tx = self.tx.clone();
            if *filter == MangaFilters::Authors {
                self.author_state.search_authors(tx);
            } else if *filter == MangaFilters::Artists {
                self.artist_state.search_artists(tx);
            }
        }
    }

    pub fn handle_events(&mut self, events: Events) {
        match events {
            Events::Key(key_event) => self.handle_key_events(key_event),
            Events::Tick => self.tick(),
            _ => {}
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
                KeyCode::Char('r') => self.reset(),
                KeyCode::Char('l') | KeyCode::Right => self.toggle_focus_input(),
                _ => {}
            }
        }
    }

    fn handle_key_events_for_input(&mut self, key_event: KeyEvent) {
        if let Some(filter) = FILTERS.get(self.id_filter) {
            match filter {
                MangaFilters::Tags => {
                    self.tags
                        .search_bar
                        .handle_event(&crossterm::event::Event::Key(key_event));
                }
                MangaFilters::Authors => {
                    self.author_state
                        .search_bar
                        .handle_event(&crossterm::event::Event::Key(key_event));
                }
                MangaFilters::Artists => {
                    self.artist_state
                        .search_bar
                        .handle_event(&crossterm::event::Event::Key(key_event));
                }
                _ => {}
            }
        }
    }

    fn toggle_focus_input(&mut self) {
        self.is_typing = !self.is_typing;
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
                }
                MangaFilters::SortBy => {
                    self.sort_by_state.scroll_down();
                }
                MangaFilters::Tags => {
                    if self.tags.items.is_some() {
                        self.tags.state.select_next();
                    }
                }
                MangaFilters::MagazineDemographic => {
                    self.magazine_demographic.scroll_down();
                }
                MangaFilters::Authors => {
                    if self.author_state.items.is_some() {
                        self.author_state.state.select_next();
                    }
                }
                MangaFilters::Artists => {
                    if self.artist_state.items.is_some() {
                        self.artist_state.state.select_next();
                    }
                }
                MangaFilters::Languages => {
                    self.lang_state.scroll_down();
                }
                MangaFilters::PublicationStatus => {
                    self.publication_status.scroll_down();
                }
            }
        }
    }

    fn scroll_up_filter_list(&mut self) {
        if let Some(filter) = FILTERS.get(self.id_filter) {
            match filter {
                MangaFilters::ContentRating => {
                    self.content_rating.scroll_up();
                }
                MangaFilters::SortBy => {
                    self.sort_by_state.scroll_up();
                }
                MangaFilters::Tags => {
                    if self.tags.items.is_some() {
                        self.tags.state.select_previous();
                    }
                }
                MangaFilters::MagazineDemographic => {
                    self.magazine_demographic.scroll_up();
                }
                MangaFilters::Authors => {
                    if self.author_state.items.is_some() {
                        self.author_state.state.select_previous();
                    }
                }
                MangaFilters::Artists => {
                    if self.artist_state.items.is_some() {
                        self.artist_state.state.select_previous();
                    }
                }

                MangaFilters::Languages => {
                    self.lang_state.scroll_up();
                }
                MangaFilters::PublicationStatus => {
                    self.publication_status.scroll_up();
                }
            }
        }
    }

    fn toggle_filter_list(&mut self) {
        if let Some(filter) = FILTERS.get(self.id_filter) {
            match filter {
                MangaFilters::ContentRating => {
                    self.content_rating.toggle();
                    self.set_content_rating();
                }
                MangaFilters::SortBy => {
                    self.sort_by_state.toggle_sort_by();
                    self.set_sort_by();
                }
                MangaFilters::Tags => {
                    self.tags.toggle_tags();
                    self.set_tags();
                }

                MangaFilters::MagazineDemographic => {
                    self.magazine_demographic.toggle();
                    self.set_magazine_demographic();
                }
                MangaFilters::Authors => {
                    self.author_state.toggle();
                    self.set_authors();
                }
                MangaFilters::Artists => {
                    self.artist_state.toggle();
                    self.set_artists();
                }

                MangaFilters::Languages => {
                    self.lang_state.toggle();
                    self.set_languages();
                }
                MangaFilters::PublicationStatus => {
                    self.publication_status.toggle();
                    self.set_publication_status();
                }
            }
        }
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
        let tags: Vec<ListItemId> = tags_response
            .data
            .into_iter()
            .map(|data| ListItemId {
                is_selected: false,
                id: data.id,
                name: data.attributes.name.en,
            })
            .collect();

        self.tags.items = Some(tags);
    }

    fn set_tags(&mut self) {
        if let Some(items) = self.tags.items.as_ref() {
            let tag_ids: Vec<String> = items
                .iter()
                .filter_map(|tag| {
                    if tag.is_selected {
                        return Some(tag.id.to_string());
                    }
                    None
                })
                .collect();

            self.filters.set_tags(tag_ids);
        }
    }

    fn set_sort_by(&mut self) {
        let sort_by_selected = self
            .sort_by_state
            .items
            .iter()
            .find(|item| item.is_selected);

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
                            return Some(Author::new(item.id.to_string()));
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
                            return Some(Artist::new(item.id.to_string()));
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
    pub fn set_author(&mut self, author: crate::common::Author) {
        self.filters.reset_author();
        self.filters.reset_artist();
        self.artist_state.items = None;
        self.author_state.items = Some(vec![ListItemId {
            id: author.id.clone(),
            is_selected: true,
            name: author.name,
        }]);
        self.filters.authors.set_one_user(Author::new(author.id))
    }

    /// This function is called from manga page
    pub fn set_artist(&mut self, artist: crate::common::Artist) {
        self.filters.reset_author();
        self.filters.reset_artist();

        self.author_state.items = None;
        self.artist_state.items = Some(vec![ListItemId {
            id: artist.id.clone(),
            is_selected: true,
            name: artist.name,
        }]);
        self.filters.artists.set_one_user(Artist::new(artist.id))
    }
}
