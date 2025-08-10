//! Feed page module for displaying and managing manga reading history and plan-to-read lists.
//!
//! This module provides the main feed interface where users can:
//! - View their reading history and plan-to-read manga
//! - Search and filter manga by title
//! - Navigate between different manga providers
//! - Delete individual manga or clear entire collections
//! - Navigate to detailed manga pages
//! - View latest chapters for each manga
//!
//! ## Key Features
//!
//! ### Dual Tab Interface
//! - **Reading History**: Shows manga the user has actually read
//! - **Plan to Read**: Shows manga the user wants to read later
//!
//! ### Search and Navigation
//! - Real-time search filtering by manga title
//! - Pagination support for large collections
//! - Keyboard navigation (j/k for up/down, w/b for next/previous page)
//! - Mouse wheel scrolling support
//!
//! ### Manga Management
//! - Delete individual manga from history (`d` key)
//! - Bulk delete all manga from a provider (`D` key with confirmation)
//! - Navigate to detailed manga pages (`r` key)
//!
//! ### Async Data Loading
//! - Background loading of latest chapters for each manga
//! - Loading states with visual feedback
//! - Error handling for failed API requests
//!
//! ## State Management
//!
//! The feed uses a state machine to manage different UI states:
//! - `DisplayingHistory`: Normal display mode
//! - `SearchingHistory`: Loading history from database
//! - `SearchingMangaPage`: Loading detailed manga data
//! - `AskingDeleteAllConfirmation`: Confirmation dialog for bulk deletion
//! - Various error states for failed operations
//!
//! ## Event System
//!
//! Uses a dual-channel event system:
//! - **Actions**: User interactions (key presses, mouse events)
//! - **Events**: Internal state changes and async results
//!
//! This separation allows for responsive UI while handling async operations.
use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use manga_tui::SearchTerm;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, ToSpan};
use ratatui::widgets::{Block, Paragraph, StatefulWidget, Tabs, Widget};
use throbber_widgets_tui::{Throbber, ThrobberState};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinSet;
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

use crate::backend::database::{Database, GetHistoryArgs, MangaHistoryResponse, MangaHistoryType};
use crate::backend::error_log::{ErrorType, write_to_error_log};
use crate::backend::manga_provider::{FeedPageProvider, LatestChapter, MangaProviders};
use crate::backend::tui::Events;
use crate::global::{ERROR_STYLE, INSTRUCTIONS_STYLE};
use crate::utils::render_search_bar;
use crate::view::widgets::Component;
use crate::view::widgets::feed::{AskConfirmationDeleteAllModal, FeedTabs, HistoryWidget, MangasRead};

/// Represents the current state of the feed page UI.
///
/// The feed uses a state machine to manage different UI modes and loading states.
/// This ensures proper user feedback and prevents invalid interactions during async operations.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FeedState {
    /// Currently searching/filtering the reading history
    SearchingHistory,
    /// An error occurred while searching history
    ErrorSearchingHistory,
    /// No manga found in the current history tab
    HistoryNotFound,
    /// Loading detailed manga page data
    SearchingMangaPage,
    /// Failed to load manga page data
    MangaPageNotFound,
    /// Normal display mode showing manga history
    DisplayingHistory,
    /// Showing confirmation dialog for bulk deletion
    AskingDeleteAllConfirmation,
}

/// User actions that can be triggered by keyboard or mouse input.
///
/// These actions are sent through the local action channel and processed
/// by the `update()` method to modify the feed state.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FeedActions {
    /// Scroll up in the manga list (k key or mouse wheel up)
    ScrollHistoryUp,
    /// Scroll down in the manga list (j key or mouse wheel down)
    ScrollHistoryDown,
    /// Toggle search bar focus (s key)
    ToggleSearchBar,
    /// Go to next page of results (w key)
    NextPage,
    /// Go to previous page of results (b key)
    PreviousPage,
    /// Switch between History and Plan to Read tabs (Tab key)
    SwitchTab,
    /// Navigate to detailed manga page (r key)
    GoToMangaPage,
}

/// Internal events for managing async operations and state changes.
///
/// These events are sent through the local event channel and processed
/// by the `tick()` method to handle async results and state transitions.
#[derive(Debug, PartialEq)]
pub enum FeedEvents {
    /// Trigger a search of the reading history
    SearchHistory,
    /// Start loading latest chapters for all displayed manga
    SearchRecentChapters,
    /// Load latest chapters for a specific manga
    LoadLatestChapters(String, Option<Vec<LatestChapter>>),
    /// Handle error when searching manga data
    ErrorSearchingMangaData,
    /// Load history results from database
    LoadHistory(Option<MangaHistoryResponse>),
}

/// Main feed page component for displaying and managing manga collections.
///
/// The `Feed` struct manages the complete feed page interface, including:
/// - UI state management and rendering
/// - User input handling (keyboard and mouse)
/// - Async data loading and caching
/// - Navigation between different views
/// - Search and filtering functionality
///
/// ## Generic Parameter
///
/// `T: FeedPageProvider` - The manga provider implementation that supplies
/// manga data and latest chapters. This allows the feed to work with different
/// manga sources (MangaDx, Weebcentral, etc.).
///
/// ## Key Components
///
/// - **Tabs**: Switch between Reading History and Plan to Read
/// - **Search Bar**: Filter manga by title
/// - **History Widget**: Display manga list with navigation
/// - **Loading States**: Visual feedback during async operations
/// - **Confirmation Dialogs**: For destructive actions
///
/// ## Event Handling
///
/// Uses a dual-channel system for responsive UI:
/// - `local_action_tx/rx`: Handle immediate user interactions
/// - `local_event_tx/rx`: Process async results and state changes
/// - `global_event_tx`: Communicate with the main application
///
/// ## Example Usage
///
/// ```rust
/// # use crate::view::pages::feed::Feed;
/// # use crate::backend::manga_provider::mock::MockMangaPageProvider;
/// let mut feed = Feed::new()
///     .with_api_client(Arc::new(MockMangaPageProvider::new()))
///     .with_global_sender(global_event_tx);
///
/// // Initialize with search
/// feed.init_search();
///
/// // Handle user input
/// feed.handle_events(Events::Key(key_event));
///
/// // Process async results
/// feed.tick();
///
/// // Render the UI
/// feed.render(area, frame);
/// ```
pub struct Feed<T>
where
    T: FeedPageProvider,
{
    /// Current active tab (History or Plan to Read)
    pub tabs: FeedTabs,
    /// Current UI state
    state: FeedState,
    /// Widget for displaying manga history
    pub history: Option<HistoryWidget>,
    /// Loading animation state
    pub loading_state: Option<ThrobberState>,
    /// Channel for sending events to the main application
    pub global_event_tx: Option<UnboundedSender<Events>>,
    /// Channel for sending user actions
    pub local_action_tx: UnboundedSender<FeedActions>,
    /// Channel for receiving user actions
    pub local_action_rx: UnboundedReceiver<FeedActions>,
    /// Channel for sending internal events
    pub local_event_tx: UnboundedSender<FeedEvents>,
    /// Channel for receiving internal events
    pub local_event_rx: UnboundedReceiver<FeedEvents>,
    /// Search input field
    search_bar: Input,
    /// Whether the search bar is currently focused
    is_typing: bool,
    /// Number of manga to display per page
    items_per_page: u32,
    /// Background tasks for async operations
    tasks: JoinSet<()>,
    /// Manga provider for fetching data
    manga_provider: Option<Arc<T>>,
}

impl<T> Feed<T>
where
    T: FeedPageProvider,
{
    /// Creates a new feed page with default settings.
    ///
    /// Initializes with:
    /// - History tab selected
    /// - DisplayingHistory state
    /// - 5 items per page
    /// - Empty search bar
    /// - No manga provider (must be set with `with_api_client()`)
    ///
    /// # Example
    ///
    /// ```rust
    /// # use crate::view::pages::feed::Feed;
    /// # use crate::backend::manga_provider::mock::MockMangaPageProvider;
    /// let feed = Feed::new();
    /// ```
    pub fn new() -> Self {
        let (local_action_tx, local_action_rx) = mpsc::unbounded_channel::<FeedActions>();
        let (local_event_tx, local_event_rx) = mpsc::unbounded_channel::<FeedEvents>();
        Self {
            tabs: FeedTabs::History,
            loading_state: None,
            history: None,
            state: FeedState::DisplayingHistory,
            global_event_tx: None,
            local_action_tx,
            local_action_rx,
            local_event_tx,
            local_event_rx,
            tasks: JoinSet::new(),
            search_bar: Input::default(),
            items_per_page: 5,
            is_typing: false,
            manga_provider: None,
        }
    }

    /// Returns whether the search bar is currently focused for text input.
    ///
    /// Used to determine if keyboard events should be handled as text input
    /// or as navigation commands.
    #[inline]
    pub fn is_typing(&self) -> bool {
        self.is_typing
    }

    /// Sets the global event sender for communicating with the main application.
    ///
    /// This channel is used to send events like navigation requests and errors
    /// to the main application loop.
    ///
    /// # Arguments
    ///
    /// * `sender` - Channel sender for global events
    ///
    /// # Example
    ///
    /// ```rust
    /// # use tokio::sync::mpsc;
    /// # use crate::backend::tui::Events;
    /// # let (tx, _) = mpsc::unbounded_channel::<Events>();
    /// let feed = Feed::new().with_global_sender(tx);
    /// ```
    pub fn with_global_sender(mut self, sender: UnboundedSender<Events>) -> Self {
        self.global_event_tx = Some(sender);
        self
    }

    /// Sets the manga provider for fetching data.
    ///
    /// The provider is responsible for fetching manga details and latest chapters
    /// from the manga source (MangaDx, Weebcentral, etc.).
    ///
    /// # Arguments
    ///
    /// * `api_client` - Arc-wrapped manga provider implementation
    ///
    /// # Example
    ///
    /// ```rust
    /// # use std::sync::Arc;
    /// # use crate::backend::manga_provider::mock::MockMangaPageProvider;
    /// let provider = Arc::new(MockMangaPageProvider::new());
    /// let feed = Feed::new().with_api_client(provider);
    /// ```
    pub fn with_api_client(mut self, api_client: Arc<T>) -> Self {
        self.manga_provider = Some(api_client);
        self
    }

    fn render_history(&mut self, area: Rect, buf: &mut Buffer) {
        if self.state == FeedState::ErrorSearchingHistory {
            Paragraph::new(
                "Cannot get your reading history due to some issues, please check error logs"
                    .to_span()
                    .style(*ERROR_STYLE),
            )
            .render(area, buf);
            return;
        }
        match self.history.as_mut() {
            Some(history) => {
                if self.state == FeedState::HistoryNotFound {
                    Paragraph::new("It seems you have no mangas stored here, try reading some").render(area, buf);
                } else {
                    StatefulWidget::render(history.clone(), area, buf, &mut history.state);
                }
            },
            None => {
                Paragraph::new("It seems you have no mangas stored here, try reading some").render(area, buf);
            },
        }
    }

    fn render_tabs_and_search_bar(&mut self, area: Rect, frame: &mut Frame) {
        let [tabs_area, search_bar_area] = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(area);

        let selected_tab = match self.tabs {
            FeedTabs::History => 0,
            FeedTabs::PlantToRead => 1,
        };

        let tabs_instructions = Line::from(vec!["Switch tab: ".into(), Span::raw("<tab>").style(*INSTRUCTIONS_STYLE)]);

        Tabs::new(vec!["Reading history", "Plan to Read"])
            .select(selected_tab)
            .block(Block::bordered().title(tabs_instructions))
            .highlight_style(Style::default().fg(Color::Yellow))
            .render(tabs_area, frame.buffer_mut());

        let input_help: Vec<Span<'_>> = if self.is_typing {
            vec!["Press ".into(), Span::raw("<Enter>").style(*INSTRUCTIONS_STYLE), " to search".into()]
        } else {
            vec!["Press ".into(), Span::raw("<s>").style(*INSTRUCTIONS_STYLE), " to filter mangas".into()]
        };

        render_search_bar(self.is_typing, input_help.into(), &self.search_bar, frame, search_bar_area);
    }

    fn render_searching_status(&mut self, area: Rect, buf: &mut Buffer) {
        if let Some(state) = self.loading_state.as_mut() {
            let loader = Throbber::default()
                .label("Searching manga data, please wait ")
                .style(Style::default().fg(Color::Yellow))
                .throbber_set(throbber_widgets_tui::BRAILLE_SIX)
                .use_type(throbber_widgets_tui::WhichUse::Spin);

            StatefulWidget::render(
                loader,
                area.inner(Margin {
                    horizontal: 1,
                    vertical: 1,
                }),
                buf,
                state,
            );
        }
        if self.state == FeedState::MangaPageNotFound {
            Paragraph::new(
                "Error, could not get manga data, please try again another time"
                    .to_span()
                    .style(*ERROR_STYLE),
            )
            .render(area, buf);
        }
    }

    fn render_top_area(&mut self, area: Rect, frame: &mut Frame) {
        let [tabs_and_search_bar_area, searching_area] = Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)]).areas(area);

        self.render_tabs_and_search_bar(tabs_and_search_bar_area, frame);

        self.render_searching_status(searching_area, frame.buffer_mut());
    }

    fn render_ask_modal_confirmation_delete_all_mangas(&mut self, area: Rect, buf: &mut Buffer) {
        if self.state == FeedState::AskingDeleteAllConfirmation {
            AskConfirmationDeleteAllModal::new()
                .with_manga_provider(self.manga_provider.as_ref().unwrap().name())
                .render(area, buf);
        }
    }

    #[inline]
    fn get_currently_selected_manga(&self) -> Option<&MangasRead> {
        self.history.as_ref().and_then(|widg| widg.get_current_manga_selected())
    }

    /// Initiates a search of the reading history.
    ///
    /// Triggers an async search operation that will load manga from the database
    /// based on the current tab, search term, and pagination settings.
    ///
    /// This method is typically called:
    /// - On initial page load
    /// - When switching tabs
    /// - When changing search terms
    /// - When navigating pages
    ///
    /// # Example
    ///
    /// ```rust
    /// # let mut feed = setup_test_feed();
    /// feed.init_search(); // Triggers FeedEvents::SearchHistory
    /// ```
    pub fn init_search(&mut self) {
        self.local_event_tx.send(FeedEvents::SearchHistory).ok();
    }

    /// Removes the currently selected manga from the reading history.
    ///
    /// This method:
    /// 1. Gets the currently selected manga
    /// 2. Removes it from the database using `Database::remove_from_history()`
    /// 3. Triggers a new search to refresh the display
    /// 4. Sends an error event if the removal fails
    ///
    /// Called when the user presses the `d` key.
    fn remove_currently_selected_manga(&mut self) {
        if let Some(manga) = self.get_currently_selected_manga() {
            let connection = Database::get_connection().unwrap();
            let database = Database::new(&connection);

            if let Err(err) = database.remove_from_history(&manga.id) {
                self.global_event_tx.as_ref().unwrap().send(Events::Error(err.to_string()));
            } else {
                self.search_history();
            }
        }
    }

    /// Removes all manga from the current provider and history type.
    ///
    /// This is a bulk deletion operation that:
    /// 1. Removes all manga for the current provider from the selected history type
    /// 2. Triggers a new search to refresh the display
    /// 3. Sends an error event if the operation fails
    ///
    /// Called when the user confirms bulk deletion (presses `w` in confirmation dialog).
    fn remove_all_mangas(&mut self) {
        let connection = Database::get_connection().unwrap();
        let database = Database::new(&connection);
        if let Err(e) = database.remove_all_from_history(self.tabs.into(), self.manga_provider.as_ref().unwrap().name()) {
            self.global_event_tx.as_ref().unwrap().send(Events::Error(e.to_string())).ok();
        };
        self.search_history();
    }

    /// Handles keyboard input based on the current state and focus.
    ///
    /// This method routes keyboard events to appropriate handlers:
    /// - If typing: handles text input and Enter/Esc for search
    /// - If in confirmation dialog: handles w/q for confirm/cancel
    /// - Otherwise: handles navigation keys (j/k, w/b, Tab, r, s, d, D)
    ///
    /// # Arguments
    ///
    /// * `key_event` - The keyboard event to handle
    fn handle_key_events(&mut self, key_event: KeyEvent) {
        if self.is_typing {
            match key_event.code {
                KeyCode::Enter => {
                    self.local_event_tx.send(FeedEvents::SearchHistory).ok();
                },
                KeyCode::Esc => {
                    self.local_action_tx.send(FeedActions::ToggleSearchBar).ok();
                },
                _ => {
                    self.search_bar.handle_event(&crossterm::event::Event::Key(key_event));
                },
            };
        } else if self.state == FeedState::AskingDeleteAllConfirmation {
            match key_event.code {
                KeyCode::Char('w') => self.remove_all_mangas(),
                KeyCode::Char('q') => self.state = FeedState::DisplayingHistory,
                _ => {},
            }
        } else {
            match key_event.code {
                KeyCode::Tab => {
                    self.local_action_tx.send(FeedActions::SwitchTab).ok();
                },
                KeyCode::Char('j') | KeyCode::Down => {
                    self.local_action_tx.send(FeedActions::ScrollHistoryDown).ok();
                },
                KeyCode::Char('k') | KeyCode::Up => {
                    self.local_action_tx.send(FeedActions::ScrollHistoryUp).ok();
                },
                KeyCode::Char('w') => {
                    self.local_action_tx.send(FeedActions::NextPage).ok();
                },

                KeyCode::Char('b') => {
                    self.local_action_tx.send(FeedActions::PreviousPage).ok();
                },
                KeyCode::Char('r') => {
                    self.local_action_tx.send(FeedActions::GoToMangaPage).ok();
                },
                KeyCode::Char('s') => {
                    self.local_action_tx.send(FeedActions::ToggleSearchBar).ok();
                },
                KeyCode::Char('d') => {
                    self.remove_currently_selected_manga();
                },
                KeyCode::Char('D') => {
                    self.state = FeedState::AskingDeleteAllConfirmation;
                },
                _ => {},
            }
        }
    }

    /// Processes async events and updates the feed state.
    ///
    /// This method should be called regularly (typically on each tick) to:
    /// - Update loading animations
    /// - Process async results from background tasks
    /// - Handle state transitions
    /// - Update the UI based on new data
    ///
    /// # Example
    ///
    /// ```rust
    /// # let mut feed = setup_test_feed();
    /// // In the main loop:
    /// feed.tick(); // Process any pending events
    /// ```
    pub fn tick(&mut self) {
        if let Some(loader_state) = self.loading_state.as_mut() {
            loader_state.calc_next();
        }
        if let Ok(local_event) = self.local_event_rx.try_recv() {
            match local_event {
                FeedEvents::ErrorSearchingMangaData => self.display_error_searching_manga(),
                FeedEvents::SearchHistory => self.search_history(),
                FeedEvents::LoadHistory(maybe_history) => self.load_history(maybe_history),
                FeedEvents::SearchRecentChapters => self.search_latest_chapters(),
                FeedEvents::LoadLatestChapters(manga_id, maybe_chapters) => {
                    self.load_recent_chapters(manga_id, maybe_chapters);
                },
            }
        }
    }

    fn load_recent_chapters(&mut self, manga_id: String, maybe_history: Option<Vec<LatestChapter>>) {
        if let Some(chapters_response) = maybe_history {
            if let Some(history) = self.history.as_mut() {
                history.set_chapter(manga_id, chapters_response);
            }
        }
    }

    /// Loads latest chapters for all displayed manga asynchronously.
    ///
    /// This method spawns background tasks to fetch the latest chapters for each
    /// manga in the current display. The results are processed asynchronously
    /// and update the manga entries with their latest chapter information.
    ///
    /// Called automatically after loading history to provide up-to-date chapter info.
    fn search_latest_chapters(&mut self) {
        if let Some(history) = self.history.as_mut() {
            for manga in history.mangas.clone() {
                let manga_id = manga.id;
                let sender = self.local_event_tx.clone();
                let client = self.manga_provider.as_ref().cloned().unwrap();
                self.tasks.spawn(async move {
                    let response = client.get_latest_chapters(&manga_id).await;
                    match response {
                        Ok(res) => {
                            sender.send(FeedEvents::LoadLatestChapters(manga_id, Some(res))).ok();
                        },
                        Err(e) => {
                            write_to_error_log(e.into());
                            sender.send(FeedEvents::LoadLatestChapters(manga_id, None)).ok();
                        },
                    }
                });
            }
        }
    }

    fn display_error_searching_manga(&mut self) {
        self.loading_state = None;
        self.state = FeedState::MangaPageNotFound;
    }

    /// Initiates a search of the reading history from the database.
    ///
    /// This method:
    /// 1. Sets the state to `SearchingHistory`
    /// 2. Aborts any existing background tasks
    /// 3. Spawns a new async task to query the database
    /// 4. Uses current search term, page, and tab settings
    ///
    /// The search results are sent back via the event channel and processed by `load_history()`.
    fn search_history(&mut self) {
        self.state = FeedState::SearchingHistory;
        let tx = self.local_event_tx.clone();
        self.tasks.abort_all();
        let search_term = self.search_bar.value().to_string();

        let page = match &self.history {
            Some(history) => history.page,
            None => 1,
        };

        let items_per_page = self.items_per_page;

        let history_type: MangaHistoryType = self.tabs.into();
        let provider = match &self.manga_provider {
            Some(provider) => provider.name(),
            None => MangaProviders::default(),
        };

        self.tasks.spawn(async move {
            let connection = Database::get_connection().unwrap();
            let database = Database::new(&connection);

            let maybe_reading_history = database.get_history(GetHistoryArgs {
                hist_type: history_type,
                page,
                search: SearchTerm::trimmed_lowercased(&search_term),
                items_per_page,
                provider,
            });

            match maybe_reading_history {
                Ok(history) => {
                    tx.send(FeedEvents::LoadHistory(Some(history))).ok();
                },
                Err(e) => {
                    write_to_error_log(ErrorType::Error(Box::new(e)));
                    tx.send(FeedEvents::LoadHistory(None)).ok();
                },
            }
        });
    }

    /// Navigates to the next page of results if available.
    ///
    /// Checks if there are more pages available and, if so, increments the page
    /// number and triggers a new search to load the next page of manga.
    fn search_next_page(&mut self) {
        if let Some(history) = self.history.as_mut() {
            if history.can_search_next_page(self.items_per_page as f64) {
                history.next_page();
                self.search_history();
            }
        }
    }

    /// Navigates to the previous page of results if available.
    ///
    /// Checks if there's a previous page and, if so, decrements the page number
    /// and triggers a new search to load the previous page of manga.
    fn search_previous_page(&mut self) {
        if let Some(history) = self.history.as_mut() {
            if history.can_search_previous_page() {
                history.previous_page();
                self.search_history();
            }
        }
    }

    /// Loads and displays the search results from the database.
    ///
    /// This method processes the results from `search_history()` and:
    /// 1. Creates a new `HistoryWidget` from the database response
    /// 2. Sets the state to `DisplayingHistory` if manga were found
    /// 3. Sets the state to `HistoryNotFound` if no manga were found
    /// 4. Triggers loading of latest chapters for the displayed manga
    ///
    /// # Arguments
    ///
    /// * `maybe_history` - Optional search results from the database
    fn load_history(&mut self, maybe_history: Option<MangaHistoryResponse>) {
        match maybe_history.filter(|history| !history.mangas.is_empty()) {
            Some(history) => {
                self.history = Some(HistoryWidget::from_database_response(history));
                self.state = FeedState::DisplayingHistory;
                self.local_event_tx.send(FeedEvents::SearchRecentChapters).ok();
            },
            None => {
                self.state = FeedState::HistoryNotFound;
                self.history = None;
            },
        }
    }

    fn select_next_manga(&mut self) {
        if let Some(mangas) = self.history.as_mut() {
            mangas.select_next();
        }
    }

    fn select_previous_manga(&mut self) {
        if let Some(mangas) = self.history.as_mut() {
            mangas.select_previous();
        }
    }

    /// Navigates to the detailed manga page for the currently selected manga.
    ///
    /// This method:
    /// 1. Sets the state to `SearchingMangaPage`
    /// 2. Shows a loading indicator
    /// 3. Fetches detailed manga data asynchronously
    /// 4. Sends a `GoToMangaPage` event to the main application
    ///
    /// If no manga is selected or the provider is not set, this method does nothing.
    ///
    /// # Example
    ///
    /// ```rust
    /// # let mut feed = setup_test_feed_with_selection();
    /// feed.go_to_manga_page(); // Will trigger Events::GoToMangaPage
    /// ```
    pub fn go_to_manga_page(&mut self) {
        self.state = FeedState::SearchingMangaPage;
        if let Some(currently_selected_manga) = self.get_currently_selected_manga() {
            let tx = self.global_event_tx.as_ref().cloned().unwrap();
            let local_tx = self.local_event_tx.clone();
            let manga_id = currently_selected_manga.id.clone();

            self.loading_state = Some(ThrobberState::default());

            let client = self.manga_provider.as_ref().cloned().unwrap();

            self.tasks.spawn(async move {
                let response = client.get_manga_by_id(&manga_id).await;
                match response {
                    Ok(res) => {
                        tx.send(Events::GoToMangaPage(res)).ok();
                    },
                    Err(e) => {
                        write_to_error_log(e.into());
                        local_tx.send(FeedEvents::ErrorSearchingMangaData).ok();
                    },
                }
            });
        } else {
            self.state = FeedState::DisplayingHistory;
        }
    }

    fn toggle_focus_search_bar(&mut self) {
        self.is_typing = !self.is_typing;
    }

    /// Switches between the History and Plan to Read tabs.
    ///
    /// This method:
    /// 1. Cycles to the next tab
    /// 2. Cleans up the current state (clears history, search bar, etc.)
    /// 3. Triggers a new search for the new tab
    ///
    /// Called when the user presses the Tab key.
    fn switch_tabs(&mut self) {
        self.tabs = self.tabs.cycle();
        self.clean_up();
        self.search_history();
    }

    fn handle_mouse_event(&mut self, mouse_event: MouseEvent) {
        match mouse_event.kind {
            MouseEventKind::ScrollUp => {
                self.local_action_tx.send(FeedActions::ScrollHistoryUp).ok();
            },
            MouseEventKind::ScrollDown => {
                self.local_action_tx.send(FeedActions::ScrollHistoryDown).ok();
            },
            _ => {},
        }
    }

    #[cfg(test)]
    fn get_history(&self) -> HistoryWidget {
        self.history.as_ref().cloned().unwrap()
    }
}

impl<T> Component for Feed<T>
where
    T: FeedPageProvider,
{
    type Actions = FeedActions;

    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) {
        let layout = Layout::vertical([Constraint::Percentage(20), Constraint::Percentage(80)]);

        let [tabs_area, history_area] = layout.areas(area);

        self.render_top_area(tabs_area, frame);

        self.render_history(history_area, frame.buffer_mut());

        self.render_ask_modal_confirmation_delete_all_mangas(area, frame.buffer_mut());
    }

    fn update(&mut self, action: Self::Actions) {
        match action {
            FeedActions::ToggleSearchBar => self.toggle_focus_search_bar(),
            FeedActions::NextPage => self.search_next_page(),
            FeedActions::PreviousPage => self.search_previous_page(),
            FeedActions::GoToMangaPage => self.go_to_manga_page(),
            FeedActions::ScrollHistoryUp => self.select_previous_manga(),
            FeedActions::ScrollHistoryDown => self.select_next_manga(),
            FeedActions::SwitchTab => self.switch_tabs(),
        }
    }

    fn clean_up(&mut self) {
        self.search_bar.reset();
        self.history = None;
        self.loading_state = None;
    }

    fn handle_events(&mut self, events: crate::backend::tui::Events) {
        match events {
            Events::Key(key_event) => {
                if self.state != FeedState::SearchingMangaPage {
                    self.handle_key_events(key_event);
                }
            },
            Events::Mouse(mouse_event) => {
                if self.state != FeedState::SearchingMangaPage {
                    self.handle_mouse_event(mouse_event);
                }
            },
            Events::Tick => self.tick(),
            _ => {},
        }
    }
}

#[cfg(test)]
mod tests {
    use core::panic;

    use pretty_assertions::{assert_eq, assert_ne};

    use self::mpsc::unbounded_channel;
    use super::*;
    use crate::backend::database::MangaHistory;
    use crate::backend::manga_provider::mock::MockMangaPageProvider;
    use crate::view::widgets::press_key;

    fn manga_history_response() -> MangaHistoryResponse {
        let mangas_in_history = vec![MangaHistory::default(), MangaHistory::default()];
        let total_items = mangas_in_history.len();

        MangaHistoryResponse {
            mangas: mangas_in_history,
            page: 1,
            total_items: total_items as u32,
        }
    }

    fn render_history_and_select(feed_page: &mut Feed<MockMangaPageProvider>) {
        feed_page.load_history(Some(manga_history_response()));

        let area = Rect::new(0, 0, 20, 20);
        let mut buf = Buffer::empty(area);

        feed_page.render_history(area, &mut buf);

        feed_page.select_next_manga();
    }

    #[test]
    fn search_for_history_when_instantiated() {
        let mut feed_page: Feed<MockMangaPageProvider> = Feed::new();

        let expected_event = FeedEvents::SearchHistory;

        feed_page.init_search();

        let event = feed_page.local_event_rx.blocking_recv().expect("the event was not sent");

        assert_eq!(expected_event, event);
    }

    #[test]
    fn history_is_loaded() {
        let mut feed_page: Feed<MockMangaPageProvider> = Feed::new();
        let response_from_database = manga_history_response();

        let expected_widget = HistoryWidget::from_database_response(response_from_database.clone());

        feed_page.load_history(Some(response_from_database));

        assert!(feed_page.history.is_some());

        assert_eq!(FeedState::DisplayingHistory, feed_page.state);

        let history = feed_page.get_history();

        assert_eq!(expected_widget.mangas, history.mangas);
        assert_eq!(expected_widget.total_results, history.total_results);
        assert_eq!(expected_widget.page, history.page);
    }

    #[test]
    fn send_events_after_history_is_loaded() {
        let mut feed_page: Feed<MockMangaPageProvider> = Feed::new();
        let response_from_database = manga_history_response();

        feed_page.load_history(Some(response_from_database));

        let expected_event = FeedEvents::SearchRecentChapters;

        let event_sent = feed_page.local_event_rx.blocking_recv().expect("no event was sent");

        assert_eq!(expected_event, event_sent);
    }

    #[test]
    fn load_no_mangas_found_from_database() {
        let mut feed_page: Feed<MockMangaPageProvider> = Feed::new();

        let mut some_empty_response = manga_history_response();

        some_empty_response.mangas = vec![];

        feed_page.load_history(Some(some_empty_response));

        assert_eq!(feed_page.state, FeedState::HistoryNotFound);

        feed_page.load_history(None);

        assert_eq!(feed_page.state, FeedState::HistoryNotFound);
    }

    #[test]
    fn load_chapters_of_manga() {
        let mut feed_page: Feed<MockMangaPageProvider> = Feed::new();

        let mut history = manga_history_response();

        let manga_id = "some_manga_id";
        let chapter_id = "some_chapter_id";

        history.mangas.push(MangaHistory {
            id: "some_manga_id".to_string(),
            ..Default::default()
        });

        feed_page.load_history(Some(history));

        let chapter_response = vec![
            LatestChapter {
                id: chapter_id.to_string(),
                ..Default::default()
            },
            LatestChapter::default(),
        ];

        feed_page.load_recent_chapters(manga_id.to_string(), Some(chapter_response));

        let expected_result = feed_page.get_history();
        let expected_result = expected_result
            .mangas
            .iter()
            .find(|manga| manga.id == manga_id)
            .expect("manga was not loaded");

        assert!(!expected_result.recent_chapters.is_empty());

        let chapter_loaded = expected_result.recent_chapters.iter().find(|chap| chap.id == chapter_id);

        assert!(chapter_loaded.is_some())
    }

    #[tokio::test]
    async fn load_chapters_of_manga_with_event() {
        let manga_id = "some_manga_id".to_string();

        let api_client = MockMangaPageProvider::with_latest_chapter_response(vec![LatestChapter {
            id: manga_id.clone(),
            ..Default::default()
        }]);

        let mut feed_page: Feed<MockMangaPageProvider> = Feed::new().with_api_client(api_client.into());

        let mut history = manga_history_response();

        history.mangas.push(MangaHistory {
            id: manga_id.clone(),
            ..Default::default()
        });

        feed_page.load_history(Some(history));

        let max_amounts_ticks = 10;
        let mut count = 0;

        loop {
            if count > max_amounts_ticks {
                break;
            }
            feed_page.tasks.join_next().await;
            feed_page.tick();
            count += 1;
        }
        let history = feed_page.get_history();

        let manga_with_chapters = history
            .mangas
            .iter()
            .find(|manga| manga.id == manga_id)
            .expect("the manga was not loaded");

        assert!(!manga_with_chapters.recent_chapters.is_empty())
    }

    #[tokio::test]
    async fn goes_to_next_page() {
        let mut feed_page: Feed<MockMangaPageProvider> = Feed::new();

        let mut history = manga_history_response();

        history.mangas = vec![MangaHistory::default(), MangaHistory::default(), MangaHistory::default(), MangaHistory::default()];
        history.total_items = history.mangas.len() as u32;
        history.page = 1;

        feed_page.items_per_page = 3;

        feed_page.load_history(Some(history));

        feed_page.search_next_page();

        assert_eq!(feed_page.get_history().page, 2);
    }

    #[tokio::test]
    async fn goes_to_previous_history_page() {
        let mut feed_page: Feed<MockMangaPageProvider> = Feed::new();

        let mut history = manga_history_response();

        history.mangas = vec![MangaHistory::default(), MangaHistory::default(), MangaHistory::default(), MangaHistory::default()];
        history.total_items = history.mangas.len() as u32;
        history.page = 2;

        feed_page.items_per_page = 3;

        feed_page.load_history(Some(history));

        feed_page.search_previous_page();

        assert_eq!(feed_page.get_history().page, 1);
    }

    #[tokio::test]
    async fn switch_between_tabs() {
        let mut feed_page: Feed<MockMangaPageProvider> = Feed::new();

        assert_eq!(feed_page.tabs, FeedTabs::History);

        feed_page.switch_tabs();

        assert_eq!(feed_page.tabs, FeedTabs::PlantToRead);

        feed_page.switch_tabs();

        assert_eq!(feed_page.tabs, FeedTabs::History);
    }

    #[tokio::test]
    async fn search_history_in_database() {
        let mut feed_page: Feed<MockMangaPageProvider> = Feed::new();

        feed_page.search_history();

        assert_eq!(feed_page.state, FeedState::SearchingHistory);

        let event_sent = feed_page.local_event_rx.recv().await.expect("no event was sent");

        match event_sent {
            FeedEvents::LoadHistory(_) => {},
            _ => panic!("expected event LoadHistory "),
        }
    }

    #[tokio::test]
    async fn listen_key_event_to_switch_tabs() {
        let mut feed_page: Feed<MockMangaPageProvider> = Feed::new();

        let initial_tab = feed_page.tabs;

        press_key(&mut feed_page, KeyCode::Tab);

        let action_sent = feed_page.local_action_rx.recv().await.expect("no key event was sent");

        feed_page.update(action_sent);

        assert_eq!(feed_page.state, FeedState::SearchingHistory);
        assert_ne!(feed_page.tabs, initial_tab);

        assert!(feed_page.history.is_none());

        let current_tab = feed_page.tabs;

        press_key(&mut feed_page, KeyCode::Tab);

        let action_sent = feed_page.local_action_rx.recv().await.expect("no key event was sent");

        feed_page.update(action_sent);

        assert_ne!(feed_page.tabs, current_tab);
    }

    #[tokio::test]
    async fn when_switching_tabs_remove_previous_history() {
        let mut feed_page: Feed<MockMangaPageProvider> = Feed::new();

        let manga_history = MangaHistoryResponse {
            mangas: vec![MangaHistory::default()],
            ..Default::default()
        };

        feed_page.load_history(Some(manga_history));

        press_key(&mut feed_page, KeyCode::Tab);

        let action_sent = feed_page.local_action_rx.recv().await.expect("no key event was sent");

        feed_page.update(action_sent);

        assert!(feed_page.history.is_none());
    }

    #[tokio::test]
    async fn scrolls_history_up_and_down() {
        let mut feed_page: Feed<MockMangaPageProvider> = Feed::new();

        let manga_history = MangaHistoryResponse {
            mangas: vec![MangaHistory::default(), MangaHistory::default(), MangaHistory::default()],
            ..Default::default()
        };

        feed_page.load_history(Some(manga_history));

        assert!(feed_page.get_history().state.selected.is_none());

        let area = Rect::new(0, 0, 20, 20);
        let mut buf = Buffer::empty(area);

        feed_page.render_history(area, &mut buf);

        // Scroll up
        press_key(&mut feed_page, KeyCode::Char('j'));

        let action_sent = feed_page.local_action_rx.recv().await.expect("no key event was sent");

        feed_page.update(action_sent);

        assert!(feed_page.get_history().state.selected.is_some_and(|index| index == 0));

        // index selected should be 1
        feed_page.select_next_manga();
        // index selected should be 2
        feed_page.select_next_manga();

        // Scroll up
        press_key(&mut feed_page, KeyCode::Char('k'));

        let action_sent = feed_page.local_action_rx.recv().await.expect("no key event was sent");

        feed_page.update(action_sent);

        assert!(feed_page.get_history().state.selected.is_some_and(|index| index == 1));
    }

    #[tokio::test]
    async fn focus_search_bar_when_pressing_s_and_unfocus_when_pressing_esc() {
        let mut feed_page: Feed<MockMangaPageProvider> = Feed::new();

        assert!(!feed_page.is_typing(), "search_bar should not be focused by default");

        press_key(&mut feed_page, KeyCode::Char('s'));

        let action_sent = feed_page.local_action_rx.recv().await.expect("no key event was sent");

        feed_page.update(action_sent);

        assert!(feed_page.is_typing(), "search_bar shoud be focused");

        press_key(&mut feed_page, KeyCode::Esc);

        let action_sent = feed_page.local_action_rx.recv().await.expect("no key event was sent");

        feed_page.update(action_sent);

        assert!(!feed_page.is_typing(), "should have unfocused the search bar");
    }

    #[tokio::test]
    async fn type_into_search_bar_when_focused() {
        let mut feed_page: Feed<MockMangaPageProvider> = Feed::new();

        feed_page.toggle_focus_search_bar();

        press_key(&mut feed_page, KeyCode::Char('s'));
        press_key(&mut feed_page, KeyCode::Char('e'));
        press_key(&mut feed_page, KeyCode::Char('a'));

        while let Ok(action) = feed_page.local_action_rx.try_recv() {
            feed_page.update(action);
        }

        let expected = "sea";

        assert_eq!(expected, feed_page.search_bar.value());
    }

    #[tokio::test]
    async fn when_searching_manga_page_should_not_listen_to_key_events() {
        let (tx, _) = unbounded_channel::<Events>();
        let mut feed_page: Feed<MockMangaPageProvider> =
            Feed::new().with_global_sender(tx).with_api_client(MockMangaPageProvider::new().into());

        render_history_and_select(&mut feed_page);

        feed_page.go_to_manga_page();

        assert_eq!(feed_page.state, FeedState::SearchingMangaPage);

        press_key(&mut feed_page, KeyCode::Char('j'));

        if feed_page.local_action_rx.try_recv().is_ok() {
            panic!("should not receive events")
        }
    }

    #[tokio::test]
    async fn goes_to_manga_page_when_pressing_r_with_selected_manga() {
        let (tx, mut rx) = unbounded_channel::<Events>();
        let mut feed_page: Feed<MockMangaPageProvider> =
            Feed::new().with_global_sender(tx).with_api_client(MockMangaPageProvider::new().into());

        render_history_and_select(&mut feed_page);
        press_key(&mut feed_page, KeyCode::Char('r'));

        let key_event = feed_page.local_action_rx.recv().await.expect("no key event was sent");

        feed_page.update(key_event);

        let event_sent = rx.recv().await.expect("no event was sent");

        match event_sent {
            Events::GoToMangaPage(_) => {},
            _ => panic!("wrong event was sent"),
        }
    }

    #[test]
    fn when_pressed_d_it_ask_for_confirmation() {
        let mut feed_page: Feed<MockMangaPageProvider> = Feed::new().with_api_client(MockMangaPageProvider::new().into());

        press_key(&mut feed_page, KeyCode::Char('D'));

        assert_eq!(FeedState::AskingDeleteAllConfirmation, feed_page.state)
    }
}
