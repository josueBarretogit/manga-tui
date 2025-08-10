//! Database module for managing manga reading history, bookmarks, and user data persistence.
//!
//! This module provides the `Database` struct and related types for managing manga-related data in a SQLite database.
//! It handles various aspects of manga tracking including:
//!
//! - **Reading History**: Track which manga chapters have been read and when
//! - **Plan to Read**: Manage a list of manga that users plan to read later
//! - **Downloads**: Track which chapters have been downloaded for offline reading
//! - **Bookmarks**: Save reading progress within specific chapters
//! - **Multi-provider Support**: Handle manga from different sources (MangaDx, Weebcentral, etc.)
//!
//! ## Database Schema
//!
//! The database consists of several interconnected tables:
//! - `mangas`: Core manga information (id, title, cover image, provider)
//! - `chapters`: Chapter details with read/download/bookmark status
//! - `history_types`: Categories for organizing manga (ReadingHistory, PlanToRead)
//! - `manga_history_union`: Links manga to history categories
//!
//! ## Key Features
//!
//! ### History Management
//! - Separate tracking for "currently reading" vs "plan to read" manga
//! - Search and pagination support for large manga collections
//! - Provider-specific filtering (e.g., only show MangaDx manga)
//!
//! ### Chapter Tracking  
//! - Mark chapters as read/unread
//! - Track download status for offline reading
//! - Bookmark specific pages within chapters
//! - Only one bookmark per manga (automatically replaces previous bookmarks)
//!
//! ### Data Integrity
//! - Automatic creation of manga records when adding chapters
//! - Foreign key relationships ensure data consistency
//! - Graceful handling of duplicate entries
//!
//! ## Example Usage
//!
//! ```rust
//! # use rusqlite::Connection;
//! # use crate::backend::database::{Database, MangaReadingHistorySave, ChapterToSaveHistory};
//! # use crate::backend::manga_provider::MangaProviders;
//! let conn = Connection::open("manga.db")?;
//! let db = Database::new(&conn);
//! db.setup()?;
//!
//! // Save reading progress
//! db.save_history(MangaReadingHistorySave {
//!     id: "manga_123",
//!     title: "One Piece",
//!     img_url: Some("cover.jpg"),
//!     chapter: ChapterToSaveHistory {
//!         id: "chapter_456",
//!         title: "Chapter 1000",
//!         translated_language: "en",
//!     },
//!     provider: MangaProviders::Mangadx,
//! })?;
//! ```
use chrono::Utc;
use manga_tui::SearchTerm;
use rusqlite::types::ToSqlOutput;
use rusqlite::{Connection, OptionalExtension, ToSql, params};
use strum::{Display, EnumIter};

use super::AppDirectories;
use super::manga_provider::{Languages, MangaProviders};
use crate::view::widgets::feed::FeedTabs;

/// Represents the type of manga history list.
///
/// Manga can be categorized into different lists for user organization:
/// - `PlanToRead`: Manga the user intends to read in the future
/// - `ReadingHistory`: Manga the user has actually started reading
#[derive(Display, Debug, Clone, Copy)]
pub enum MangaHistoryType {
    PlanToRead,
    ReadingHistory,
}

impl ToSql for MangaHistoryType {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        let as_string = self.to_string();

        Ok(ToSqlOutput::from(as_string))
    }
}

impl From<FeedTabs> for MangaHistoryType {
    fn from(value: FeedTabs) -> Self {
        match value {
            FeedTabs::History => Self::ReadingHistory,
            FeedTabs::PlantToRead => Self::PlanToRead,
        }
    }
}

#[derive(Debug, Clone, Copy, Display, EnumIter)]
pub enum Table {
    #[strum(to_string = "mangas")]
    Mangas,
    #[strum(to_string = "history_types")]
    HistoryTypes,
    #[strum(to_string = "chapters")]
    Chapters,
    #[strum(to_string = "manga_history_union")]
    MangaHistoryUnion,
}

#[derive(Clone)]
pub struct MangaInsert<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub img_url: Option<&'a str>,
    pub provider: MangaProviders,
}

#[derive(Debug, Default)]
pub struct ChapterToSaveHistory<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub translated_language: &'a str,
}

pub struct MangaReadingHistorySave<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub img_url: Option<&'a str>,
    pub chapter: ChapterToSaveHistory<'a>,
    pub provider: MangaProviders,
}

pub struct MangaReadingHistoryRetrieve {
    pub id: String,
    pub is_downloaded: bool,
    pub is_read: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MangaHistory {
    pub id: String,
    pub title: String,
    // img_url: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MangaHistoryResponse {
    pub mangas: Vec<MangaHistory>,
    pub page: u32,
    pub total_items: u32,
}

/// Arguments for retrieving manga history with filtering and pagination.
///
/// Used with `Database::get_history()` to specify exactly what manga to retrieve.
/// Supports complex queries with provider filtering, text search, and pagination.
#[derive(Debug)]
pub struct GetHistoryArgs {
    /// Which type of history to retrieve (ReadingHistory or PlanToRead)
    pub hist_type: MangaHistoryType,
    /// Page number (1-based) for pagination
    pub page: u32,
    /// Optional search term to filter by manga title
    pub search: Option<SearchTerm>,
    /// Number of items to return per page
    pub items_per_page: u32,
    /// Filter results to only this manga provider
    pub provider: MangaProviders,
}

/// This is used in the `feed` page to retrieve the mangas the user is currently reading
pub struct MangaPlanToReadSave<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub img_url: Option<&'a str>,
    pub provider: MangaProviders,
}

/// Information needed to mark a chapter as downloaded.
///
/// Used with `Database::set_chapter_downloaded()` when a user downloads
/// a chapter for offline reading. Contains both manga and chapter information
/// since the manga record may need to be created.
pub struct SetChapterDownloaded<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub manga_id: &'a str,
    pub manga_title: &'a str,
    pub img_url: Option<&'a str>,
    pub provider: MangaProviders,
}

/// Database wrapper providing high-level operations for manga data management.
///
/// The `Database` struct encapsulates a SQLite connection and provides methods for:
/// - Managing reading history and "plan to read" lists
/// - Tracking chapter read/download status
/// - Handling bookmarks with page-level precision
/// - Multi-provider manga support
///
/// ## Lifecycle
///
/// 1. Create with `Database::new(&connection)`
/// 2. Initialize schema with `setup()`
/// 3. Use various methods to manage manga data
///
/// ## Thread Safety
///
/// This struct holds a reference to a SQLite connection. SQLite connections are not
/// thread-safe by default, so ensure proper synchronization if using across threads.
#[derive(Debug)]
pub struct Database<'a> {
    connection: &'a Connection,
}

impl<'a> Database<'a> {
    /// Creates a new Database instance wrapping the provided SQLite connection.
    ///
    /// # Arguments
    ///
    /// * `conn` - Reference to an open SQLite connection
    ///
    /// # Example
    ///
    /// ```rust
    /// # use rusqlite::Connection;
    /// # use crate::backend::database::Database;
    /// let conn = Connection::open_in_memory()?;
    /// let db = Database::new(&conn);
    /// ```
    #[inline]
    pub fn new(conn: &'a Connection) -> Self {
        Self { connection: conn }
    }

    /// Initializes the database schema and default data.
    ///
    /// Creates all required tables if they don't exist:
    /// - `app_version`: Tracks application version
    /// - `mangas`: Core manga information
    /// - `chapters`: Chapter details and status
    /// - `history_types`: Categories (ReadingHistory, PlanToRead)
    /// - `manga_history_union`: Links manga to categories
    ///
    /// This method is idempotent - safe to call multiple times.
    ///
    /// # Errors
    ///
    /// Returns `rusqlite::Result<()>` on database errors.
    pub fn setup(&self) -> rusqlite::Result<()> {
        self.connection.execute(
            "CREATE TABLE if not exists app_version (
                version TEXT PRIMARY KEY
             )",
            (),
        )?;

        let already_has_data: i32 = self.connection.query_row("SELECT COUNT(*) from app_version", [], |row| row.get(0))?;

        if already_has_data == 0 {
            self.connection
                .execute("INSERT INTO app_version(version) VALUES (?1) ", [env!("CARGO_PKG_VERSION")])?;
        }

        self.connection.execute(
            "CREATE TABLE if not exists history_types (
                id    INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE
             )",
            (),
        )?;

        self.connection.execute(
            "CREATE TABLE if not exists mangas (
                id    TEXT  PRIMARY KEY,
                title TEXT  NOT NULL,
                created_at  DATETIME DEFAULT (datetime('now')),
                updated_at  DATETIME DEFAULT (datetime('now')),
                last_read  DATETIME DEFAULT (datetime('now')),
                deleted_at  DATETIME NULL,
                img_url TEXT NULL,
                manga_provider TEXT NOT NULL DEFAULT mangadex
             )",
            (),
        )?;

        self.connection.execute(
            "CREATE TABLE if not exists chapters (
                id    TEXT  PRIMARY KEY,
                title TEXT  NOT NULL,
                manga_id TEXT  NOT NULL,
                is_read BOOLEAN NOT NULL DEFAULT 0,
                is_downloaded BOOLEAN NOT NULL DEFAULT 0,
                is_bookmarked BOOLEAN NOT NULL DEFAULT false,
                translated_language TEXT NULL,
                number_page_bookmarked INT NULL,
                FOREIGN KEY (manga_id) REFERENCES mangas (id)
            )",
            (),
        )?;

        self.connection.execute(
            "CREATE TABLE if not exists manga_history_union (
                manga_id TEXT, 
                type_id INTEGER, 
                PRIMARY KEY (manga_id, type_id),
                FOREIGN KEY (manga_id) REFERENCES mangas (id),
                FOREIGN KEY (type_id) REFERENCES history_types (id)
             )",
            (),
        )?;

        let already_has_data: i32 = self.connection.query_row("SELECT COUNT(*) from history_types", [], |row| row.get(0))?;

        if already_has_data < 2 {
            self.connection
                .execute("INSERT INTO history_types(name) VALUES (?1) ", [MangaHistoryType::ReadingHistory.to_string()])?;

            self.connection
                .execute("INSERT INTO history_types(name) VALUES (?1) ", [MangaHistoryType::PlanToRead.to_string()])?;
        }

        Ok(())
    }

    /// Gets a database connection, using in-memory for tests or file-based for production.
    ///
    /// # Returns
    ///
    /// - In test mode: Returns an in-memory SQLite connection
    /// - In production: Returns a file-based connection to the app's history database
    #[inline]
    pub fn get_connection() -> rusqlite::Result<Connection> {
        if cfg!(test) { Connection::open_in_memory() } else { Connection::open(AppDirectories::History.get_full_path()) }
    }

    /// Checks if a chapter has already been marked as read.
    ///
    /// # Arguments
    ///
    /// * `id` - Chapter ID to check
    ///
    /// # Returns
    ///
    /// - `Ok(true)` if chapter exists and is marked as read
    /// - `Ok(false)` if chapter doesn't exist or is not read
    ///
    /// # Example
    ///
    /// ```rust
    /// # let db = setup_test_db();
    /// if !db.check_chapter_is_already_reading("chapter_123")? {
    ///     // Chapter not read yet, safe to mark as read
    ///     db.save_history(reading_data)?;
    /// }
    /// ```
    fn check_chapter_is_already_reading(&self, id: &str) -> rusqlite::Result<bool> {
        let exists = self.check_exists(id, Table::Chapters)?;

        if !exists {
            return Ok(false);
        }

        let is_read: bool = self
            .connection
            .query_row("SELECT is_read FROM chapters WHERE id = ?1", params![id], |row| row.get(0))?;

        Ok(is_read)
    }

    fn create_manga_if_not_exists(&self, manga: MangaInsert<'_>) -> rusqlite::Result<()> {
        if self.check_exists(manga.id, Table::Mangas)? {
            return Ok(());
        }

        self.connection
            .execute("INSERT INTO mangas(id, title, img_url, manga_provider) VALUES(?1, ?2, ?3, ?4)", params![
                manga.id,
                manga.title,
                manga.img_url,
                manga.provider.to_string()
            ])?;

        Ok(())
    }

    fn create_chapter_if_not_exists(&self, chap: ChapterToInsert<'_>) -> rusqlite::Result<()> {
        if self.check_exists(chap.id, Table::Chapters)? {
            return Ok(());
        }

        self.connection
            .execute("INSERT INTO chapters(id, title, manga_id, is_read, translated_language, number_page_bookmarked, is_downloaded, is_bookmarked) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)", params![chap.id, chap.title, chap.manga_id, chap.is_read, chap.translated_language, chap.number_page_bookmarked, chap.is_downloaded, chap.is_bookmarked])?;

        Ok(())
    }

    fn bookmark_chapter(&mut self, chapter_to_bookmark: ChapterToBookmark<'_>) -> rusqlite::Result<()> {
        self.create_manga_if_not_exists(MangaInsert {
            id: chapter_to_bookmark.manga_id,
            title: chapter_to_bookmark.manga_title,
            img_url: chapter_to_bookmark.manga_cover_url,
            provider: chapter_to_bookmark.provider,
        })?;

        self.create_chapter_if_not_exists(ChapterToInsert {
            id: chapter_to_bookmark.chapter_id,
            title: chapter_to_bookmark.chapter_title,
            manga_id: chapter_to_bookmark.manga_id,
            is_read: false,
            is_downloaded: false,
            is_bookmarked: true,
            number_page_bookmarked: chapter_to_bookmark.page_number,
            translated_language: chapter_to_bookmark.translated_language.as_iso_code(),
        })?;

        self.connection
            .execute("UPDATE chapters SET is_bookmarked = false WHERE manga_id = ?1", [chapter_to_bookmark.manga_id])?;

        self.connection
            .execute("UPDATE chapters SET is_bookmarked = true, number_page_bookmarked = ?1 WHERE id = ?2", params![
                chapter_to_bookmark.page_number,
                chapter_to_bookmark.chapter_id
            ])?;

        Ok(())
    }

    fn get_chapter_bookmarked(&self, manga_id: &str) -> rusqlite::Result<Option<ChapterBookmarked>> {
        let query = r"
        SELECT chapters.id, chapters.translated_language, chapters.number_page_bookmarked, mangas.title, mangas.id 

        FROM chapters INNER JOIN mangas ON chapters.manga_id = mangas.id

        WHERE manga_id = ?1 AND is_bookmarked = true
        ";

        self.connection
            .query_row(query, params![manga_id], |row| {
                let chapter: ChapterBookmarked = ChapterBookmarked {
                    id: row.get(0)?,
                    translated_language: row.get(1)?,
                    number_page_bookmarked: row.get(2)?,
                    manga_title: row.get(3)?,
                    manga_id: row.get(4)?,
                };

                Ok(chapter)
            })
            .optional()
    }

    fn get_history_type(&self, hist_type: MangaHistoryType) -> rusqlite::Result<i32> {
        let history_type_id: i32 =
            self.connection
                .query_row("SELECT id FROM history_types where name = ?1", params![hist_type.to_string()], |row| row.get(0))?;
        Ok(history_type_id)
    }

    fn manga_is_plan_to_read(&self, manga_id: &str) -> rusqlite::Result<bool> {
        let history_type = self.get_history_type(MangaHistoryType::PlanToRead)?;
        let is_already_plan_to_read: bool = self.connection.query_row(
            "SELECT EXISTS(SELECT * FROM manga_history_union WHERE manga_id = ?1 AND type_id = ?2)",
            params![manga_id, history_type],
            |row| row.get(0),
        )?;

        Ok(is_already_plan_to_read)
    }

    /// check if a value exists in a table
    fn check_exists(&self, id: &str, table: Table) -> rusqlite::Result<bool> {
        let table = table.to_string();
        let exists: bool = self.connection.query_row(
            format!("SELECT EXISTS(SELECT id FROM {table} WHERE id = ?1) as row_exists").as_str(),
            rusqlite::params![id],
            |row| row.get(0),
        )?;
        Ok(exists)
    }

    fn manga_is_reading(&self, id: &str) -> rusqlite::Result<bool> {
        let history_type: i32 = self.connection.query_row(
            "SELECT id FROM history_types where name = ?1",
            params![MangaHistoryType::ReadingHistory.to_string()],
            |row| row.get(0),
        )?;
        let exists: bool = self.connection.query_row(
            "SELECT EXISTS(SELECT * FROM manga_history_union WHERE manga_id = ?1 AND type_id = ?2)",
            rusqlite::params![id, history_type],
            |row| row.get(0),
        )?;
        Ok(exists)
    }

    fn insert_manga_in_reading_history(&self, manga_id: &str) -> rusqlite::Result<()> {
        let reading_history: i32 = self.connection.query_row(
            "SELECT id FROM history_types where name = ?1",
            params![MangaHistoryType::ReadingHistory.to_string()],
            |row| row.get(0),
        )?;

        self.connection
            .execute("INSERT INTO manga_history_union VALUES (?1, ?2)", (manga_id, reading_history))?;
        Ok(())
    }

    /// Insert a manga in the reading history type or update the `last_read` field
    fn update_or_insert_manga_in_most_recent_reading_history(&self, manga_id: &str) -> rusqlite::Result<()> {
        if !self.manga_is_reading(manga_id)? {
            self.insert_manga_in_reading_history(manga_id)?;
            Ok(())
        } else {
            let now = Utc::now().naive_utc();
            self.connection
                .execute("UPDATE mangas SET last_read = ?1 WHERE id = ?2", params![now.to_string(), manga_id])?;
            Ok(())
        }
    }

    /// Marks a chapter as downloaded and read.
    ///
    /// This method handles the complete flow for downloaded chapters:
    /// 1. Creates manga record if it doesn't exist
    /// 2. Adds chapter to reading history
    /// 3. Marks chapter as both downloaded and read
    ///
    /// Used when a user downloads a chapter for offline reading.
    ///
    /// # Arguments
    ///
    /// * `chapter` - Chapter download information
    ///
    /// # Example
    ///
    /// ```rust
    /// # use crate::backend::database::SetChapterDownloaded;
    /// db.set_chapter_downloaded(SetChapterDownloaded {
    ///     id: "chapter_123",
    ///     title: "Chapter 1",
    ///     manga_id: "manga_456",
    ///     manga_title: "One Piece",
    ///     img_url: Some("cover.jpg"),
    ///     provider: MangaProviders::Mangadx,
    /// })?;
    /// ```
    pub fn set_chapter_downloaded(&self, chapter: SetChapterDownloaded<'_>) -> rusqlite::Result<()> {
        if self.check_exists(chapter.manga_id, Table::Mangas)? {
            self.update_or_insert_manga_in_most_recent_reading_history(chapter.manga_id)?;

            if self.check_exists(chapter.id, Table::Chapters)? {
                self.connection
                    .execute("UPDATE chapters SET is_downloaded = ?1, is_read = ?2 WHERE id = ?3", params![
                        true, true, chapter.id
                    ])?;
            } else {
                self.create_chapter_if_not_exists(ChapterToInsert {
                    id: chapter.id,
                    title: chapter.title,
                    manga_id: chapter.manga_id,
                    is_read: true,
                    is_downloaded: true,
                    is_bookmarked: false,
                    number_page_bookmarked: None,
                    translated_language: "",
                })?;
            }

            Ok(())
        } else {
            self.create_manga_if_not_exists(MangaInsert {
                id: chapter.manga_id,
                title: chapter.manga_title,
                img_url: chapter.img_url,
                provider: chapter.provider,
            })?;

            self.create_chapter_if_not_exists(ChapterToInsert {
                id: chapter.id,
                title: chapter.title,
                manga_id: chapter.manga_id,
                is_read: true,
                is_downloaded: true,
                is_bookmarked: false,
                number_page_bookmarked: None,
                translated_language: "",
            })?;

            self.insert_manga_in_reading_history(chapter.manga_id)?;

            Ok(())
        }
    }

    /// Records that a user has read a chapter, adding it to reading history.
    ///
    /// This is the main method for tracking reading progress. It:
    /// 1. Creates manga and chapter records if they don't exist
    /// 2. Adds manga to reading history (if not already there)
    /// 3. Marks the specific chapter as read
    /// 4. Updates the manga's last_read timestamp
    ///
    /// If the chapter is already marked as read, this method does nothing.
    ///
    /// # Arguments
    ///
    /// * `data` - Reading history data including manga and chapter info
    ///
    /// # Example
    ///
    /// ```rust
    /// # use crate::backend::database::{MangaReadingHistorySave, ChapterToSaveHistory};
    /// db.save_history(MangaReadingHistorySave {
    ///     id: "manga_123",
    ///     title: "Attack on Titan",
    ///     img_url: Some("cover.jpg"),
    ///     chapter: ChapterToSaveHistory {
    ///         id: "chapter_456",
    ///         title: "The Final Chapter",
    ///         translated_language: "en",
    ///     },
    ///     provider: MangaProviders::Mangadx,
    /// })?;
    /// ```
    pub fn save_history(&self, data: MangaReadingHistorySave<'_>) -> rusqlite::Result<()> {
        if self.check_chapter_is_already_reading(data.chapter.id)? {
            return Ok(());
        }

        self.create_manga_if_not_exists(MangaInsert {
            id: data.id,
            title: data.title,
            img_url: data.img_url,
            provider: data.provider,
        })?;

        self.create_chapter_if_not_exists(ChapterToInsert {
            id: data.chapter.id,
            title: data.chapter.title,
            manga_id: data.id,
            is_read: false,
            is_downloaded: false,
            is_bookmarked: false,
            translated_language: data.chapter.translated_language,
            number_page_bookmarked: None,
        })?;

        self.update_or_insert_manga_in_most_recent_reading_history(data.id)?;

        self.connection
            .execute("UPDATE chapters SET is_read = true WHERE id = ?1", params![data.chapter.id])?;
        Ok(())
    }

    /// Adds a manga to the "Plan to Read" list.
    ///
    /// This method allows users to bookmark manga they want to read later.
    /// If the manga is already in the plan to read list, this method does nothing.
    ///
    /// # Arguments
    ///
    /// * `manga` - Manga information to add to plan to read
    ///
    /// # Example
    ///
    /// ```rust
    /// # use crate::backend::database::MangaPlanToReadSave;
    /// db.save_plan_to_read(MangaPlanToReadSave {
    ///     id: "manga_789",
    ///     title: "Demon Slayer",
    ///     img_url: Some("cover.jpg"),
    ///     provider: MangaProviders::Mangadx,
    /// })?;
    /// ```
    pub fn save_plan_to_read(&self, manga: MangaPlanToReadSave<'_>) -> rusqlite::Result<()> {
        let history_type = self.get_history_type(MangaHistoryType::PlanToRead)?;

        if !self.manga_is_plan_to_read(manga.id)? {
            if self.check_exists(manga.id, Table::Mangas)? {
                self.connection
                    .execute("INSERT INTO manga_history_union VALUES (?1, ?2)", (manga.id, history_type))?;
                return Ok(());
            }

            self.create_manga_if_not_exists(MangaInsert {
                id: manga.id,
                title: manga.title,
                img_url: manga.img_url,
                provider: manga.provider,
            })?;

            self.connection
                .execute("INSERT INTO manga_history_union VALUES (?1, ?2)", (manga.id, history_type))?;
        }
        Ok(())
    }

    /// Retrieves the read/download status of all chapters for a specific manga.
    ///
    /// Returns a list of all chapters associated with the manga, along with their
    /// current read and download status. Useful for displaying chapter lists in the UI.
    ///
    /// # Arguments
    ///
    /// * `manga_id` - ID of the manga to get chapter status for
    ///
    /// # Returns
    ///
    /// Vector of `MangaReadingHistoryRetrieve` containing chapter IDs and their status.
    ///
    /// # Example
    ///
    /// ```rust
    /// let chapters = db.get_chapters_history_status("manga_123")?;
    /// for chapter in chapters {
    ///     println!(
    ///         "Chapter {}: read={}, downloaded={}",
    ///         chapter.id, chapter.is_read, chapter.is_downloaded
    ///     );
    /// }
    /// ```
    pub fn get_chapters_history_status(&self, manga_id: &str) -> rusqlite::Result<Vec<MangaReadingHistoryRetrieve>> {
        let mut chapter_ids: Vec<MangaReadingHistoryRetrieve> = vec![];

        let mut result = self.connection
        .prepare("SELECT chapters.id, chapters.is_downloaded, chapters.is_read from chapters INNER JOIN mangas ON mangas.id = chapters.manga_id WHERE mangas.id = ?1")?;

        let result_iter = result.query_map(params![manga_id], |row| {
            Ok(MangaReadingHistoryRetrieve {
                id: row.get(0)?,
                is_downloaded: row.get(1)?,
                is_read: row.get(2)?,
            })
        })?;

        for chapter_id in result_iter.flatten() {
            chapter_ids.push(chapter_id);
        }

        Ok(chapter_ids)
    }

    fn get_total_mangas_in_history(&self, history_type_id: i32, provider: MangaProviders) -> rusqlite::Result<u32> {
        let total: u32 = self.connection.query_row(
            "
                SELECT COUNT(*) from mangas
                INNER JOIN manga_history_union ON mangas.id = manga_history_union.manga_id 
                WHERE manga_history_union.type_id = ?1 AND mangas.manga_provider = ?2",
            params![history_type_id, provider.to_string()],
            |row| row.get(0),
        )?;

        Ok(total)
    }

    fn filter_manga_history_by_search_term(
        &self,
        search_term: SearchTerm,
        history_type_id: i32,
        current_page: u32,
        items_per_page: u32,
        offset: u32,
        provider: MangaProviders,
    ) -> rusqlite::Result<MangaHistoryResponse> {
        let search_term = search_term.get();
        let mut manga_history: Vec<MangaHistory> = vec![];
        let total_mangas_with_search: u32 = self.connection.query_row(
            "
                SELECT COUNT(*) from mangas
                INNER JOIN manga_history_union ON mangas.id = manga_history_union.manga_id 
                WHERE manga_history_union.type_id = ?1 AND LOWER(mangas.title) LIKE '%' || ?2 || '%' AND mangas.manga_provider = ?3",
            params![history_type_id, search_term, provider.to_string()],
            |row| row.get(0),
        )?;

        let mut get_statement_with_search_term = self.connection.prepare(
            "SELECT  mangas.id, mangas.title from mangas 
                     INNER JOIN manga_history_union ON mangas.id = manga_history_union.manga_id 
                     WHERE manga_history_union.type_id = ?1 AND LOWER(mangas.title) LIKE '%' || ?2 || '%' AND mangas.manga_provider = ?3
                     ORDER BY mangas.last_read DESC
                     LIMIT ?4 OFFSET ?5",
        )?;
        let iter_mangas = get_statement_with_search_term.query_map(
            params![history_type_id, search_term, provider.to_string(), items_per_page, offset],
            |row| {
                Ok(MangaHistory {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    // img_url: row.get(2)?,
                })
            },
        )?;

        for manga in iter_mangas {
            manga_history.push(manga?);
        }

        Ok(MangaHistoryResponse {
            mangas: manga_history,
            total_items: total_mangas_with_search,
            page: current_page,
        })
    }

    /// Retrieves manga from reading history or plan to read list with pagination and search.
    ///
    /// This is the main method for fetching manga collections for display in the UI.
    /// Supports:
    /// - Pagination for large collections
    /// - Text search by manga title
    /// - Provider filtering (only show manga from specific sources)
    /// - Separate history types (ReadingHistory vs PlanToRead)
    ///
    /// Results are ordered by last_read timestamp (most recent first).
    ///
    /// # Arguments
    ///
    /// * `args` - Query parameters including page, search term, provider filter
    ///
    /// # Returns
    ///
    /// `MangaHistoryResponse` containing:
    /// - `mangas`: List of manga for the requested page
    /// - `page`: Current page number
    /// - `total_items`: Total count of matching manga (for pagination)
    ///
    /// # Example
    ///
    /// ```rust
    /// # use crate::backend::database::{GetHistoryArgs, MangaHistoryType};
    /// # use manga_tui::SearchTerm;
    /// let history = db.get_history(GetHistoryArgs {
    ///     hist_type: MangaHistoryType::ReadingHistory,
    ///     page: 1,
    ///     search: Some(SearchTerm::new("attack on titan")),
    ///     items_per_page: 20,
    ///     provider: MangaProviders::Mangadx,
    /// })?;
    ///
    /// println!("Found {} manga", history.total_items);
    /// for manga in history.mangas {
    ///     println!("- {}", manga.title);
    /// }
    /// ```
    pub fn get_history(&self, args: GetHistoryArgs) -> rusqlite::Result<MangaHistoryResponse> {
        let items_per_page = args.items_per_page;
        let offset = (args.page - 1) * items_per_page;

        let history_type_id: i32 = self.connection.query_row(
            "SELECT id from history_types WHERE name = ?1",
            params![args.hist_type.to_string()],
            |row| row.get(0),
        )?;

        if let Some(search_term) = args.search {
            return self.filter_manga_history_by_search_term(
                search_term,
                history_type_id,
                args.page,
                items_per_page,
                offset,
                args.provider,
            );
        }

        let total_mangas: u32 = self.get_total_mangas_in_history(history_type_id, args.provider)?;

        let mut get_statement = self.connection.prepare(
            "SELECT  mangas.id, mangas.title from mangas 
                     INNER JOIN manga_history_union ON mangas.id = manga_history_union.manga_id 
                     WHERE manga_history_union.type_id = ?1 AND mangas.manga_provider = ?2
                     ORDER BY mangas.last_read DESC
                     LIMIT ?3 OFFSET ?4",
        )?;

        let mut manga_history: Vec<MangaHistory> = vec![];

        let iter_mangas =
            get_statement.query_map(params![history_type_id, args.provider.to_string(), items_per_page, offset], |row| {
                Ok(MangaHistory {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    // img_url: row.get(2)?,
                })
            })?;

        for manga in iter_mangas {
            manga_history.push(manga?);
        }

        Ok(MangaHistoryResponse {
            mangas: manga_history,
            total_items: total_mangas,
            page: args.page,
        })
    }

    /// Removes a specific manga from all history lists.
    ///
    /// This removes the manga from both reading history and plan to read lists,
    /// but does not delete the manga or chapter records themselves.
    ///
    /// # Arguments
    ///
    /// * `manga_id` - ID of manga to remove from history
    ///
    /// # Example
    ///
    /// ```rust
    /// // Remove manga from user's history lists
    /// db.remove_from_history("manga_123")?;
    /// ```
    pub fn remove_from_history(&self, manga_id: &str) -> rusqlite::Result<()> {
        self.connection
            .execute("DELETE FROM manga_history_union WHERE manga_history_union.manga_id = ?1", params![manga_id])?;

        Ok(())
    }

    /// Removes all manga of a specific provider from a specific history type.
    ///
    /// This is a bulk operation to clear entire sections of a user's library.
    /// For example, removing all MangaDx manga from reading history while
    /// leaving plan to read and other providers untouched.
    ///
    /// # Arguments
    ///
    /// * `hist_type` - Which history list to clear (ReadingHistory or PlanToRead)
    /// * `provider` - Which manga provider to remove (Mangadx, Weebcentral, etc.)
    ///
    /// # Example
    ///
    /// ```rust
    /// // Clear all MangaDx manga from reading history
    /// db.remove_all_from_history(MangaHistoryType::ReadingHistory, MangaProviders::Mangadx)?;
    /// ```
    pub fn remove_all_from_history(&self, hist_type: MangaHistoryType, provider: MangaProviders) -> rusqlite::Result<()> {
        let history_type_id = self.get_history_type(hist_type)?;

        let get_ids_manga_to_delete_statement = r#"SELECT  mangas.id from mangas 
                     INNER JOIN manga_history_union ON mangas.id = manga_history_union.manga_id 
                     WHERE manga_history_union.type_id = ?1 AND mangas.manga_provider = ?2
            "#;

        let delete_statement =
            format!("DELETE FROM manga_history_union WHERE manga_history_union.manga_id IN ({get_ids_manga_to_delete_statement})");

        self.connection
            .execute(&delete_statement, params![history_type_id, provider.to_string()])?;

        Ok(())
    }
}

#[derive(Default, Debug, Clone)]
pub struct ChapterToInsert<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub manga_id: &'a str,
    pub is_read: bool,
    pub is_downloaded: bool,
    pub is_bookmarked: bool,
    pub translated_language: &'a str,
    pub number_page_bookmarked: Option<u32>,
}

/// Information needed to save a chapter bookmark.
///
/// Bookmarks allow users to save their exact reading position within a chapter,
/// including the specific page number. Only one bookmark per manga is allowed -
/// setting a new bookmark automatically removes any previous bookmark for that manga.
#[derive(Default, Debug)]
pub struct ChapterToBookmark<'a> {
    pub chapter_id: &'a str,
    pub manga_id: &'a str,
    pub chapter_title: &'a str,
    pub manga_title: &'a str,
    pub manga_cover_url: Option<&'a str>,
    pub translated_language: Languages,
    pub page_number: Option<u32>,
    pub provider: MangaProviders,
}

/// Trait for types that can create chapter bookmarks.
///
/// Implementing this trait allows a type to save user bookmarks, which track
/// the exact page where a user stopped reading within a chapter.
pub trait Bookmark {
    /// Creates or updates a bookmark for the specified chapter.
    ///
    /// If a bookmark already exists for this manga, it will be replaced.
    /// Only one bookmark per manga is allowed at a time.
    ///
    /// # Arguments
    ///
    /// * `chapter_to_bookmark` - Complete bookmark information
    ///
    /// # Errors
    ///
    /// Returns an error if the database operation fails.
    fn bookmark(&mut self, chapter_to_bookmark: ChapterToBookmark<'_>) -> Result<(), Box<dyn std::error::Error>>;
}

/// A retrieved bookmark containing the user's saved reading position.
///
/// This represents a bookmark that was previously saved and is now being
/// retrieved from the database. Contains all the information needed to
/// resume reading at the exact position where the user left off.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChapterBookmarked {
    pub id: String,
    pub translated_language: Option<String>,
    pub number_page_bookmarked: Option<u32>,
    pub manga_title: String,
    pub manga_id: String,
}

/// Trait for types that can retrieve saved bookmarks.
///
/// Implementing this trait allows a type to fetch previously saved bookmarks,
/// enabling users to resume reading exactly where they left off.
pub trait RetrieveBookmark {
    /// Gets the current bookmark for a specific manga.
    ///
    /// Since only one bookmark per manga is allowed, this returns the single
    /// active bookmark if one exists.
    ///
    /// # Arguments
    ///
    /// * `manga_id` - ID of the manga to get the bookmark for
    ///
    /// # Returns
    ///
    /// - `Ok(Some(bookmark))` if a bookmark exists
    /// - `Ok(None)` if no bookmark is set for this manga
    /// - `Err(...)` if the database operation fails
    ///
    /// # Example
    ///
    /// ```rust
    /// # let db = setup_test_db();
    /// if let Some(bookmark) = db.get_bookmarked("manga_123")? {
    ///     println!("Resume reading at page {}", bookmark.number_page_bookmarked.unwrap_or(1));
    /// }
    /// ```
    fn get_bookmarked(&self, manga_id: &str) -> Result<Option<ChapterBookmarked>, Box<dyn std::error::Error>>;
}

impl<'a> Bookmark for Database<'a> {
    /// Creates or updates a bookmark for the specified chapter.
    ///
    /// This implementation handles the complete bookmark workflow:
    /// 1. Creates manga record if it doesn't exist
    /// 2. Creates chapter record if it doesn't exist
    /// 3. Removes any existing bookmark for this manga (only one bookmark per manga allowed)
    /// 4. Sets the new bookmark with the specified page number
    ///
    /// Bookmarks are used to save the exact page where a user stopped reading,
    /// allowing them to resume exactly where they left off.
    ///
    /// # Arguments
    ///
    /// * `chapter_to_bookmark` - Complete bookmark information including manga, chapter, and page number
    ///
    /// # Errors
    ///
    /// Returns an error if any database operation fails (insert, update, etc.)
    ///
    /// # Example
    ///
    /// ```rust
    /// # use crate::backend::database::{Database, ChapterToBookmark};
    /// # use crate::backend::manga_provider::{MangaProviders, Languages};
    /// # let mut db = setup_test_db();
    /// let bookmark_data = ChapterToBookmark {
    ///     chapter_id: "chapter_123",
    ///     manga_id: "manga_456",
    ///     chapter_title: "Chapter 1",
    ///     manga_title: "One Piece",
    ///     manga_cover_url: Some("cover.jpg"),
    ///     translated_language: Languages::English,
    ///     page_number: Some(15), // User stopped at page 15
    ///     provider: MangaProviders::Mangadx,
    /// };
    ///
    /// db.bookmark(bookmark_data)?;
    /// ```
    fn bookmark(&mut self, chapter_to_bookmark: ChapterToBookmark<'_>) -> Result<(), Box<dyn std::error::Error>> {
        Ok(self.bookmark_chapter(chapter_to_bookmark)?)
    }
}

impl<'a> RetrieveBookmark for Database<'a> {
    /// Retrieves the current bookmark for a specific manga.
    ///
    /// This implementation queries the database to find the active bookmark for the given manga.
    /// Since only one bookmark per manga is allowed, this returns the single bookmark if it exists.
    ///
    /// The bookmark contains all information needed to resume reading:
    /// - Chapter ID and title
    /// - Manga ID and title
    /// - Translated language
    /// - Exact page number where reading stopped
    ///
    /// # Arguments
    ///
    /// * `manga_id` - ID of the manga to get the bookmark for
    ///
    /// # Returns
    ///
    /// - `Ok(Some(bookmark))` if a bookmark exists for this manga
    /// - `Ok(None)` if no bookmark is set for this manga
    /// - `Err(...)` if the database query fails
    ///
    /// # Example
    ///
    /// ```rust
    /// # use crate::backend::database::Database;
    /// # let db = setup_test_db();
    /// match db.get_bookmarked("manga_456")? {
    ///     Some(bookmark) => {
    ///         println!(
    ///             "Resume reading '{}' at chapter '{}', page {}",
    ///             bookmark.manga_title,
    ///             bookmark.id,
    ///             bookmark.number_page_bookmarked.unwrap_or(1)
    ///         );
    ///     },
    ///     None => {
    ///         println!("No bookmark found for this manga");
    ///     },
    /// }
    /// ```
    fn get_bookmarked(&self, manga_id: &str) -> Result<Option<ChapterBookmarked>, Box<dyn std::error::Error>> {
        Ok(self.get_chapter_bookmarked(manga_id)?)
    }
}

#[cfg(test)]
mod test {

    use std::error::Error;

    use pretty_assertions::assert_eq;
    use rusqlite::Result;
    use strum::IntoEnumIterator;
    use uuid::Uuid;

    use super::*;

    fn check_tables_exist(connection: &Connection) -> Result<()> {
        for table in Table::iter() {
            connection.query_row(
                format!("SELECT name FROM sqlite_master WHERE type='table' AND name='{table}'").as_str(),
                [],
                |row| row.get::<_, String>(0),
            )?;
        }

        let amount_types: i32 =
            connection.query_row(format!("SELECT COUNT(*) from {}", Table::HistoryTypes).as_str(), [], |row| row.get(0))?;

        assert!(amount_types > 0, "there should be history types");

        Ok(())
    }

    struct GetChapters {
        id: String,
        title: String,
        manga_id: String,
        is_read: bool,
        is_downloaded: bool,
    }

    fn get_all_chapters(conn: &Connection) -> Result<Vec<GetChapters>> {
        let mut statement = conn.prepare(format!("SELECT * FROM {}", Table::Chapters).as_str())?;

        let mut chapters: Vec<GetChapters> = vec![];

        let chapter_rows = statement.query_map(params![], |row| {
            Ok(GetChapters {
                id: row.get(0)?,
                title: row.get(1)?,
                manga_id: row.get(2)?,
                is_read: row.get(3)?,
                is_downloaded: row.get(4)?,
            })
        })?;

        for chapter in chapter_rows {
            chapters.push(chapter?);
        }

        Ok(chapters)
    }

    #[test]
    fn database_is_initialized() -> Result<()> {
        let connection = Connection::open_in_memory()?;

        let database = Database::new(&connection);

        database.setup().expect("could not setup the database");

        check_tables_exist(&connection)?;

        Ok(())
    }

    #[test]
    fn insert_manga_and_chapter() -> Result<()> {
        let connection = Connection::open_in_memory()?;
        let database = Database::new(&connection);
        database.setup()?;

        let manga_id = Uuid::new_v4().to_string();

        let chapter_id = Uuid::new_v4().to_string();

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_id,
            title: "some_title",
            img_url: None,
            provider: MangaProviders::Weebcentral,
        })?;

        database.create_chapter_if_not_exists(ChapterToInsert {
            id: &chapter_id,
            title: "some_title",
            manga_id: &manga_id,
            is_read: true,
            is_downloaded: false,
            is_bookmarked: false,
            number_page_bookmarked: None,
            translated_language: "en",
        })?;

        Ok(())
    }

    #[test]
    fn check_chapter_is_already_reading() -> Result<()> {
        let conn = Connection::open_in_memory()?;

        let database = Database::new(&conn);

        database.setup()?;

        let manga_id = Uuid::new_v4().to_string();

        let chapter_id_not_read = Uuid::new_v4().to_string();
        let chapter_id_is_read = Uuid::new_v4().to_string();

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_id,
            title: "some_title",
            img_url: None,
            provider: MangaProviders::Weebcentral,
        })?;

        conn.execute("INSERT INTO chapters(id, title, manga_id) VALUES(?1, ?2, ?3)", params![
            chapter_id_not_read.clone(),
            "some_title",
            manga_id.clone(),
        ])?;

        conn.execute("INSERT INTO chapters(id, is_read, title, manga_id) VALUES(?1, ?2, ?3, ?4)", params![
            chapter_id_is_read.clone(),
            true,
            "some_title",
            manga_id,
        ])?;

        let non_existent = database.check_chapter_is_already_reading("non_existent")?;
        let is_already_reading = database.check_chapter_is_already_reading(&chapter_id_is_read)?;
        let not_reading = database.check_chapter_is_already_reading(&chapter_id_not_read)?;

        assert!(is_already_reading);
        assert!(!non_existent);
        assert!(!not_reading);

        Ok(())
    }

    #[test]
    fn save_manga_plan_to_read_which_does_not_exist() -> Result<()> {
        let connection = Database::get_connection()?;
        let database = Database::new(&connection);
        database.setup()?;
        let manga_id = Uuid::new_v4().to_string();

        database.save_plan_to_read(MangaPlanToReadSave {
            id: &manga_id,
            title: "some_title",
            img_url: None,
            provider: MangaProviders::Weebcentral,
        })?;

        let manga_was_saved = database.check_exists(&manga_id, Table::Mangas)?;

        assert!(manga_was_saved, "manga should have been saved");

        let manga_is_plan_to_read = database.manga_is_plan_to_read(&manga_id)?;

        assert!(manga_is_plan_to_read, "the manga was not plan to read");

        Ok(())
    }

    #[test]
    fn save_already_existing_manga_plan_to_read() -> Result<()> {
        let connection = Database::get_connection()?;
        let database = Database::new(&connection);
        database.setup()?;
        let manga_id = Uuid::new_v4().to_string();

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_id,
            title: "some_title",
            img_url: None,
            provider: MangaProviders::Weebcentral,
        })?;

        let manga_should_not_be_plan_to_read = database.manga_is_plan_to_read(&manga_id)?;

        assert!(!manga_should_not_be_plan_to_read);

        database.save_plan_to_read(MangaPlanToReadSave {
            id: &manga_id,
            title: "some_title",
            img_url: None,
            provider: MangaProviders::Weebcentral,
        })?;

        let manga_should_be_plan_to_read = database.manga_is_plan_to_read(&manga_id)?;

        assert!(manga_should_be_plan_to_read);

        Ok(())
    }

    // Both manga and chapter are not in the database
    #[test]
    fn save_manga_reading_status_which_does_not_exist() -> Result<()> {
        let connection = Connection::open_in_memory()?;

        let database = Database::new(&connection);

        database.setup()?;

        let manga_id = Uuid::new_v4().to_string();
        let chapter_id = Uuid::new_v4().to_string();

        database.save_history(MangaReadingHistorySave {
            id: &manga_id,
            title: "some_title",
            img_url: None,
            chapter: ChapterToSaveHistory {
                id: &chapter_id,
                ..Default::default()
            },
            provider: MangaProviders::Weebcentral,
        })?;

        let manga_was_created = database.check_exists(&manga_id, Table::Mangas)?;
        let chapter_was_created = database.check_exists(&chapter_id, Table::Chapters)?;

        assert!(manga_was_created);

        assert!(chapter_was_created);

        Ok(())
    }

    // manga is already in database, chapter isnt
    #[test]
    fn save_manga_reading_status_which_already_exists() -> Result<()> {
        let connection = Connection::open_in_memory()?;

        let database = Database::new(&connection);

        database.setup()?;

        let manga_id = Uuid::new_v4().to_string();
        let chapter_id = Uuid::new_v4().to_string();

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_id,
            title: "some_title",
            img_url: None,

            provider: MangaProviders::Weebcentral,
        })?;

        database.save_history(MangaReadingHistorySave {
            id: &manga_id,
            title: "some_title",
            img_url: None,
            chapter: ChapterToSaveHistory {
                id: &chapter_id,
                ..Default::default()
            },

            provider: MangaProviders::Weebcentral,
        })?;

        let chapters = get_all_chapters(&connection)?;

        let saved_chapter = chapters
            .iter()
            .find(|chap| chap.id == chapter_id)
            .expect("no chapter was saved as being read");

        assert!(saved_chapter.is_read);

        Ok(())
    }

    #[test]
    fn save_manga_reading_both_manga_and_chapter_exist_and_chapter_is_already_reading() -> Result<()> {
        let connection = Database::get_connection()?;

        let database = Database::new(&connection);
        database.setup()?;
        let manga_id = Uuid::new_v4().to_string();
        let chapter_id = Uuid::new_v4().to_string();

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_id,
            title: "some_title",
            img_url: None,
            provider: MangaProviders::Weebcentral,
        })?;

        let chapter_which_is_already_reading = ChapterToInsert {
            id: &chapter_id,
            title: "some_title",
            manga_id: &manga_id,
            is_read: true,
            is_downloaded: true,
            is_bookmarked: false,
            number_page_bookmarked: None,
            translated_language: "en",
        };

        database.create_chapter_if_not_exists(chapter_which_is_already_reading.clone())?;

        database
            .save_history(MangaReadingHistorySave {
                id: &manga_id,
                title: chapter_which_is_already_reading.title,
                img_url: None,
                chapter: ChapterToSaveHistory {
                    id: &chapter_id,
                    ..Default::default()
                },
                provider: MangaProviders::Weebcentral,
            })
            .expect("could not save chapter history");

        let chapters = get_all_chapters(&connection)?;

        let saved_chapter = chapters
            .iter()
            .find(|chap| chap.id == chapter_id)
            .expect("no chapter was saved as being read");

        // saving reading status should not have overwritten its donwload status
        assert!(saved_chapter.is_downloaded);
        assert!(saved_chapter.is_read);

        Ok(())
    }

    #[test]
    fn get_chapters_which_have_reading_status() -> Result<()> {
        let connection = Database::get_connection()?;
        let database = Database::new(&connection);
        database.setup()?;

        let manga_id = Uuid::new_v4().to_string();
        let chapter_id_not_read = Uuid::new_v4().to_string();
        let chapter_id_read = Uuid::new_v4().to_string();

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_id,
            title: "some_manga",
            img_url: None,
            provider: MangaProviders::Weebcentral,
        })?;

        database.create_chapter_if_not_exists(ChapterToInsert {
            id: &chapter_id_not_read,
            title: "some_chapter",
            manga_id: &manga_id,
            is_read: false,
            is_downloaded: false,
            ..Default::default()
        })?;

        database.create_chapter_if_not_exists(ChapterToInsert {
            id: &chapter_id_read,
            title: "some_chapter",
            manga_id: &manga_id,
            is_read: true,
            is_downloaded: false,
            ..Default::default()
        })?;

        let chapters = database.get_chapters_history_status(&manga_id)?;

        assert!(!chapters.is_empty());

        let mut chapters = chapters.into_iter();

        let first_chapter = chapters.next().unwrap();

        assert_eq!(chapter_id_not_read, first_chapter.id);
        assert!(!first_chapter.is_read);

        let second_chapter = chapters.next().unwrap();

        assert_eq!(chapter_id_read, second_chapter.id);
        assert!(second_chapter.is_read);

        Ok(())
    }

    #[test]
    fn get_manga_history_reading_with_no_search_term() -> Result<()> {
        let connection = Database::get_connection()?;
        let database = Database::new(&connection);
        database.setup()?;

        let manga_is_in_reading_history_id = Uuid::new_v4().to_string();
        let manga_not_in_reading_history_id = Uuid::new_v4().to_string();

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_is_in_reading_history_id,
            title: "some_title",
            img_url: None,
            provider: MangaProviders::Weebcentral,
        })?;

        database.insert_manga_in_reading_history(&manga_is_in_reading_history_id)?;

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_not_in_reading_history_id,
            title: "some_title",
            img_url: None,
            provider: MangaProviders::Weebcentral,
        })?;

        let history = database.get_history(GetHistoryArgs {
            hist_type: MangaHistoryType::ReadingHistory,
            page: 1,
            search: None,
            items_per_page: 100,
            provider: MangaProviders::Weebcentral,
        })?;

        assert!(history.total_items > 0);

        assert!(history.mangas.iter().any(|manga| manga.id == manga_is_in_reading_history_id));

        assert!(!history.mangas.iter().any(|manga| manga.id == manga_not_in_reading_history_id));

        Ok(())
    }

    #[test]
    fn get_manga_history_reading_of_provider() -> Result<()> {
        let connection = Database::get_connection()?;
        let database = Database::new(&connection);
        database.setup()?;

        let manga_id_mangadex = Uuid::new_v4().to_string();
        let manga_id_weebcentral = Uuid::new_v4().to_string();

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_id_weebcentral,
            title: "of Weebcentral",
            img_url: None,
            provider: MangaProviders::Weebcentral,
        })?;

        database.insert_manga_in_reading_history(&manga_id_weebcentral)?;

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_id_mangadex,
            title: "of mangadex",
            img_url: None,
            provider: MangaProviders::Mangadex,
        })?;

        database.insert_manga_in_reading_history(&manga_id_mangadex)?;

        // No search term
        let history = database.get_history(GetHistoryArgs {
            hist_type: MangaHistoryType::ReadingHistory,
            page: 1,
            search: None,
            items_per_page: 100,
            provider: MangaProviders::Weebcentral,
        })?;

        // There are 2 mangas but of Weebcentral there is only one
        assert_eq!(1, history.total_items);

        assert!(!history.mangas.iter().any(|manga| manga.id == manga_id_mangadex));

        assert!(history.mangas.iter().any(|manga| manga.id == manga_id_weebcentral));

        let history = database.get_history(GetHistoryArgs {
            hist_type: MangaHistoryType::ReadingHistory,
            page: 1,
            search: SearchTerm::trimmed_lowercased("Weebcentral"),
            items_per_page: 100,
            provider: MangaProviders::Mangadex,
        })?;

        // Should be 0 because it is requesting mangas from mangadex
        assert_eq!(0, history.total_items);

        assert!(!history.mangas.iter().any(|manga| manga.id == manga_id_mangadex));
        assert!(!history.mangas.iter().any(|manga| manga.id == manga_id_weebcentral));

        Ok(())
    }

    #[test]
    fn get_manga_history_reading_with_search_term() -> Result<()> {
        let connection = Database::get_connection()?;

        let database = Database::new(&connection);
        database.setup()?;
        let manga_id_filtered_out = Uuid::new_v4().to_string();
        let manga_id_included_in_search = Uuid::new_v4().to_string();

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_id_filtered_out,
            title: "filtered_out",
            img_url: None,
            provider: MangaProviders::Weebcentral,
        })?;

        database.insert_manga_in_reading_history(&manga_id_filtered_out)?;

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_id_included_in_search,
            title: "included",
            img_url: None,
            provider: MangaProviders::Weebcentral,
        })?;

        database.insert_manga_in_reading_history(&manga_id_included_in_search)?;

        let history = database.get_history(GetHistoryArgs {
            hist_type: MangaHistoryType::ReadingHistory,
            page: 1,
            search: SearchTerm::trimmed_lowercased("Included"),
            items_per_page: 100,
            provider: MangaProviders::Weebcentral,
        })?;

        assert!(history.total_items > 0);

        assert!(!history.mangas.iter().any(|manga| manga.id == manga_id_filtered_out));
        assert!(history.mangas.iter().any(|manga| manga.id == manga_id_included_in_search));

        Ok(())
    }

    #[test]
    fn get_manga_planned_to_read_with_search_term() -> Result<()> {
        let connection = Database::get_connection()?;

        let database = Database::new(&connection);
        database.setup()?;
        let manga_id_filtered_out = Uuid::new_v4().to_string();
        let manga_id_included_in_search = Uuid::new_v4().to_string();

        let manga_filtered_out = MangaPlanToReadSave {
            id: &manga_id_filtered_out,
            title: "filtered_out",
            img_url: None,
            provider: MangaProviders::Weebcentral,
        };

        let manga_included = MangaPlanToReadSave {
            id: &manga_id_included_in_search,
            title: "included",
            img_url: None,
            provider: MangaProviders::Weebcentral,
        };

        database.save_plan_to_read(manga_filtered_out)?;

        database.save_plan_to_read(manga_included)?;

        let history = database.get_history(GetHistoryArgs {
            hist_type: MangaHistoryType::PlanToRead,
            page: 1,
            search: SearchTerm::trimmed_lowercased("Included"),
            items_per_page: 100,
            provider: MangaProviders::Weebcentral,
        })?;

        assert!(history.total_items > 0);

        assert!(!history.mangas.iter().any(|manga| manga.id == manga_id_filtered_out));
        assert!(history.mangas.iter().any(|manga| manga.id == manga_id_included_in_search));

        Ok(())
    }

    #[test]
    fn get_manga_planned_to_read() -> Result<()> {
        let connection = Database::get_connection()?;
        let database = Database::new(&connection);
        database.setup()?;

        let manga_id_1 = Uuid::new_v4().to_string();
        let manga_id_2 = Uuid::new_v4().to_string();

        let manga_1 = MangaPlanToReadSave {
            id: &manga_id_1,
            title: "manga_1",
            img_url: None,
            provider: MangaProviders::Weebcentral,
        };

        let manga_2 = MangaPlanToReadSave {
            id: &manga_id_2,
            title: "manga_2",
            img_url: None,
            provider: MangaProviders::Weebcentral,
        };

        database.save_plan_to_read(manga_1)?;

        database.save_plan_to_read(manga_2)?;

        let history = database.get_history(GetHistoryArgs {
            hist_type: MangaHistoryType::PlanToRead,
            page: 1,
            search: None,
            items_per_page: 100,
            provider: MangaProviders::Weebcentral,
        })?;

        assert!(history.total_items > 0);

        assert!(history.mangas.iter().any(|manga| manga.id == manga_id_1));
        assert!(history.mangas.iter().any(|manga| manga.id == manga_id_2));

        Ok(())
    }

    // Test the case when a manga is not in the database and a chapters is not in the database
    // either
    #[test]
    fn save_chapter_download_status_manga_doesnt_exist_and_chapter_doesnt_exist() -> Result<()> {
        let connection = Database::get_connection()?;
        let database = Database::new(&connection);
        database.setup()?;

        let chapter_id = Uuid::new_v4().to_string();
        let manga_id = Uuid::new_v4().to_string();

        database.set_chapter_downloaded(SetChapterDownloaded {
            id: &chapter_id,
            title: "some_title",
            manga_id: &manga_id,
            manga_title: "some_title",
            img_url: None,
            provider: MangaProviders::Weebcentral,
        })?;

        assert!(database.check_exists(&manga_id, Table::Mangas)?);

        let chapters = get_all_chapters(&connection)?;

        let chapter_downloaded = chapters
            .iter()
            .find(|chap| chap.id == chapter_id)
            .expect("chapter downloaded was nost found");

        assert!(chapter_downloaded.is_read);
        assert!(chapter_downloaded.is_downloaded);

        Ok(())
    }

    // Test the case when both manga and chapter already exist in database
    #[test]
    fn save_chapter_download_status_manga_and_chapter_exists() -> Result<()> {
        let connection = Database::get_connection()?;

        let database = Database::new(&connection);
        database.setup()?;
        let chapter_id_exist_in_database = Uuid::new_v4().to_string();
        let manga_id_exist_in_database = Uuid::new_v4().to_string();

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_id_exist_in_database,
            title: "some_title",
            img_url: None,
            provider: MangaProviders::Weebcentral,
        })?;

        database.create_chapter_if_not_exists(ChapterToInsert {
            id: &chapter_id_exist_in_database,
            title: "some_title",
            manga_id: &manga_id_exist_in_database,
            is_read: false,
            is_downloaded: false,
            ..Default::default()
        })?;

        database.set_chapter_downloaded(SetChapterDownloaded {
            id: &chapter_id_exist_in_database,
            title: "some_title",
            manga_id: &manga_id_exist_in_database,
            manga_title: "some_title",
            img_url: None,
            provider: MangaProviders::Mangadex,
        })?;

        let chapters = get_all_chapters(&connection)?;

        let chapter_downloaded = chapters
            .iter()
            .find(|chap| chap.id == chapter_id_exist_in_database)
            .expect("chapter downloaded was nost found");

        assert!(chapter_downloaded.is_read);
        assert!(chapter_downloaded.is_downloaded);

        Ok(())
    }

    #[test]
    fn save_chapter_download_status_manga_exist_and_chapter_doesnt_exist() -> Result<()> {
        let connection = Database::get_connection()?;
        let database = Database::new(&connection);
        database.setup()?;

        let chapter_id = Uuid::new_v4().to_string();
        let manga_id = Uuid::new_v4().to_string();

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_id,
            title: "some_title",
            img_url: None,
            provider: MangaProviders::Weebcentral,
        })?;

        database.set_chapter_downloaded(SetChapterDownloaded {
            id: &chapter_id,
            title: "some_title",
            manga_id: &manga_id,
            manga_title: "some_title",
            img_url: None,
            provider: MangaProviders::Mangadex,
        })?;

        let chapters = get_all_chapters(&connection)?;

        let chapter_downloaded = chapters
            .iter()
            .find(|chap| chap.id == chapter_id)
            .expect("chapter downloaded was nost found");

        assert!(chapter_downloaded.is_read);
        assert!(chapter_downloaded.is_downloaded);

        Ok(())
    }

    #[test]
    fn database_bookmarks_chapter() -> Result<()> {
        let connection = Connection::open_in_memory()?;
        let mut database = Database::new(&connection);

        database.setup()?;

        let chapter_id = Uuid::new_v4().to_string();
        let manga_id = Uuid::new_v4().to_string();

        connection.execute("INSERT INTO mangas(id, title) VALUES(?1,?2)", params![manga_id.clone(), "some_title"])?;

        connection.execute("INSERT INTO chapters(id, title, manga_id) VALUES(?1,?2,?3)", params![
            chapter_id.clone(),
            "some_title",
            manga_id
        ])?;

        let chapter_to_bookmark1: ChapterToBookmark = ChapterToBookmark {
            chapter_id: &chapter_id,
            manga_id: &manga_id,
            page_number: Some(3),
            ..Default::default()
        };

        database.bookmark(chapter_to_bookmark1).expect("failed to bookmark chapter");

        let was_bookmarked: bool =
            connection.query_row("SELECT is_bookmarked FROM chapters WHERE id = ?1", params![chapter_id], |row| row.get(0))?;

        let page_set: Option<u32> =
            connection
                .query_row("SELECT number_page_bookmarked FROM chapters WHERE id = ?1", params![chapter_id], |row| row.get(0))?;

        assert!(was_bookmarked);
        assert_eq!(page_set.expect("should not be null"), 3);

        let chapter_to_bookmark1: ChapterToBookmark = ChapterToBookmark {
            chapter_id: &chapter_id,
            manga_id: &manga_id,
            page_number: None,
            ..Default::default()
        };

        database.bookmark(chapter_to_bookmark1).expect("failed to bookmark chapter");

        let page_set_to_none: Option<u32> =
            connection
                .query_row("SELECT number_page_bookmarked FROM chapters WHERE id = ?1", params![chapter_id], |row| row.get(0))?;

        assert!(page_set_to_none.is_none());
        Ok(())
    }

    #[test]
    fn database_only_bookmarks_one_chapter_at_a_time_per_manga() -> Result<()> {
        let connection = Connection::open_in_memory()?;
        let mut database = Database::new(&connection);

        database.setup()?;

        let chapter_id = Uuid::new_v4().to_string();
        let chapter_id_2 = Uuid::new_v4().to_string();
        let chapter_id_should_stay_bookmarked = Uuid::new_v4().to_string();
        let manga_id = Uuid::new_v4().to_string();
        let manga_id_2 = Uuid::new_v4().to_string();

        connection.execute("INSERT INTO mangas(id, title) VALUES(?1,?2)", params![manga_id.clone(), "some_title"])?;
        connection.execute("INSERT INTO mangas(id, title) VALUES(?1,?2)", params![manga_id_2.clone(), "some_title2"])?;

        connection.execute("INSERT INTO chapters(id, title, manga_id) VALUES(?1,?2,?3)", params![
            chapter_id.clone(),
            "some_title",
            manga_id
        ])?;

        connection.execute("INSERT INTO chapters(id, title, manga_id) VALUES(?1,?2,?3)", params![
            chapter_id_2.clone(),
            "some_title",
            manga_id
        ])?;

        connection.execute("INSERT INTO chapters(id, title, manga_id) VALUES(?1,?2,?3)", params![
            chapter_id_should_stay_bookmarked.clone(),
            "some_title",
            manga_id_2
        ])?;

        let chapter_to_bookmark1: ChapterToBookmark = ChapterToBookmark {
            chapter_id: &chapter_id,
            manga_id: &manga_id,
            page_number: None,
            ..Default::default()
        };

        database.bookmark_chapter(chapter_to_bookmark1).expect("failed to bookmark chapter1");

        let chapter_to_bookmark_should_stay_bookmarked: ChapterToBookmark = ChapterToBookmark {
            chapter_id: &chapter_id_should_stay_bookmarked,
            manga_id: &manga_id_2,
            page_number: None,
            ..Default::default()
        };

        database
            .bookmark_chapter(chapter_to_bookmark_should_stay_bookmarked)
            .expect("failed to bookmark chapter_id_should_stay_bookmarked");

        let chapter_to_bookmark2 = ChapterToBookmark {
            chapter_id: &chapter_id_2,
            manga_id: &manga_id,
            page_number: None,
            ..Default::default()
        };

        database.bookmark_chapter(chapter_to_bookmark2).expect("failed to bookmark chapter2");

        let was_bookmarked_1: bool =
            connection.query_row("SELECT is_bookmarked FROM chapters WHERE id = ?1", params![chapter_id], |row| row.get(0))?;

        let was_bookmarked_2: bool =
            connection.query_row("SELECT is_bookmarked FROM chapters WHERE id = ?1", params![chapter_id_2], |row| row.get(0))?;

        let should_stay_bookmarked: bool = connection.query_row(
            "SELECT is_bookmarked FROM chapters WHERE id = ?1",
            params![chapter_id_should_stay_bookmarked],
            |row| row.get(0),
        )?;

        assert!(!was_bookmarked_1);
        assert!(was_bookmarked_2);
        assert!(should_stay_bookmarked);

        Ok(())
    }

    #[test]
    fn it_inserts_manga_and_chapter_if_it_does_not_exists() -> Result<()> {
        let connection = Connection::open_in_memory()?;
        let database = Database::new(&connection);

        database.setup()?;

        let id_manga = Uuid::new_v4().to_string();
        let chapter_id = Uuid::new_v4().to_string();

        database.create_manga_if_not_exists(MangaInsert {
            id: &id_manga,
            title: "some_title",
            img_url: None,
            provider: MangaProviders::Weebcentral,
        })?;

        let id_was_created: String = connection
            .query_row("SELECT id from mangas WHERE id = ?1", params![id_manga], |row| row.get(0))
            .expect("manga was not created");

        assert_eq!(id_was_created, id_manga);

        database
            .create_manga_if_not_exists(MangaInsert {
                id: &id_manga,
                title: "some_title",
                img_url: None,
                provider: MangaProviders::Weebcentral,
            })
            .expect("should not try to create already existing manga");

        database
            .create_chapter_if_not_exists(ChapterToInsert {
                id: &chapter_id,
                title: "some_title",
                manga_id: &id_manga,
                ..Default::default()
            })
            .expect("should create chapter");

        let id_chapter_was_created: String = connection
            .query_row("SELECT id from chapters WHERE id = ?1", params![chapter_id], |row| row.get(0))
            .expect("chapter was not created");

        assert_eq!(chapter_id, id_chapter_was_created);

        database
            .create_chapter_if_not_exists(ChapterToInsert {
                id: &chapter_id,
                title: "some_title",
                manga_id: &id_manga,
                ..Default::default()
            })
            .expect("should try to create chapter already existing");

        Ok(())
    }

    #[test]
    fn it_bookmarks_chapter_if_it_does_not_exits_in_database() -> Result<()> {
        let connection = Connection::open_in_memory()?;
        let mut database = Database::new(&connection);

        database.setup()?;

        let chapter_id = Uuid::new_v4().to_string();
        let manga_id = Uuid::new_v4().to_string();

        let chapter_to_bookmark = ChapterToBookmark {
            chapter_id: &chapter_id,
            manga_id: &manga_id,
            page_number: None,
            ..Default::default()
        };

        database.bookmark_chapter(chapter_to_bookmark).expect("failed to bookmark chapter");

        let was_bookmarked: bool = connection
            .query_row("SELECT is_bookmarked FROM chapters WHERE id = ?1", params![chapter_id], |row| row.get(0))
            .expect("chapter was not created");

        assert!(was_bookmarked);

        Ok(())
    }

    #[test]
    fn database_gets_chapter_bookmarked() -> Result<()> {
        let connection = Connection::open_in_memory()?;
        let database = Database::new(&connection);

        database.setup()?;

        let chapter_id = Uuid::new_v4().to_string();
        let manga_id = Uuid::new_v4().to_string();

        connection.execute("INSERT INTO mangas(id, title) VALUES(?1, ?2)", params![manga_id.clone(), "some_title"])?;

        connection.execute("INSERT INTO chapters(id, title, manga_id) VALUES(?1,?2,?3)", params![
            chapter_id.clone(),
            "some_title",
            manga_id
        ])?;

        let expected: ChapterBookmarked = ChapterBookmarked {
            id: "bookmarked".to_string(),
            translated_language: Some("en".to_string()),
            number_page_bookmarked: Some(2),
            manga_id: manga_id.clone(),
            manga_title: "some_title".to_string(),
        };

        connection.execute(
            "INSERT INTO chapters(id, title, manga_id, translated_language, number_page_bookmarked, is_bookmarked) VALUES(?1,?2,?3,?4,?5,?6)",
            params![expected.id, "some_title", manga_id, expected.translated_language, expected.number_page_bookmarked, true],
        )?;

        let result = database.get_bookmarked(&manga_id).expect("should be ok").expect("should not be none");

        assert_eq!(expected, result);

        Ok(())
    }

    #[test]
    fn it_deletes_a_manga_from_reading_history() -> Result<(), Box<dyn Error>> {
        let connection = Database::get_connection()?;
        let database = Database::new(&connection);
        database.setup()?;

        let manga_id_mangadex = Uuid::new_v4().to_string();
        let manga_id_mangadex2 = Uuid::new_v4().to_string();

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_id_mangadex,
            title: "of mangadex 1",
            img_url: None,
            provider: MangaProviders::Mangadex,
        })?;

        database.insert_manga_in_reading_history(&manga_id_mangadex)?;

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_id_mangadex2,
            title: "of mangadex 2",
            img_url: None,
            provider: MangaProviders::Mangadex,
        })?;

        database.insert_manga_in_reading_history(&manga_id_mangadex2)?;

        let expected = database.get_history(GetHistoryArgs {
            hist_type: MangaHistoryType::ReadingHistory,
            page: 1,
            search: None,
            items_per_page: 10,
            provider: MangaProviders::Mangadex,
        })?;

        /* at this point 2 mangas must be stored in reading history */
        assert_eq!(expected.mangas.len(), 2);

        database.remove_from_history(&manga_id_mangadex)?;

        let expected = database.get_history(GetHistoryArgs {
            hist_type: MangaHistoryType::ReadingHistory,
            page: 1,
            search: None,
            items_per_page: 10,
            provider: MangaProviders::Mangadex,
        })?;

        /* now only one should exist */
        assert_eq!(expected.mangas.len(), 1);

        Ok(())
    }

    #[test]
    fn it_deletes_a_manga_from_plan_to_read() -> Result<(), Box<dyn Error>> {
        let connection = Database::get_connection()?;
        let database = Database::new(&connection);
        database.setup()?;

        let manga_id_mangadex = Uuid::new_v4().to_string();
        let manga_id_mangadex2 = Uuid::new_v4().to_string();

        database.save_plan_to_read(MangaPlanToReadSave {
            id: &manga_id_mangadex,
            title: "of mangadex 1",
            img_url: None,
            provider: MangaProviders::Mangadex,
        })?;

        database.insert_manga_in_reading_history(&manga_id_mangadex)?;

        database.save_plan_to_read(MangaPlanToReadSave {
            id: &manga_id_mangadex2,
            title: "of mangadex 2",
            img_url: None,
            provider: MangaProviders::Mangadex,
        })?;

        database.insert_manga_in_reading_history(&manga_id_mangadex2)?;

        let expected = database.get_history(GetHistoryArgs {
            hist_type: MangaHistoryType::PlanToRead,
            page: 1,
            search: None,
            items_per_page: 10,
            provider: MangaProviders::Mangadex,
        })?;

        /* at this point 2 mangas must be stored in reading history */
        assert_eq!(expected.mangas.len(), 2);

        database.remove_from_history(&manga_id_mangadex)?;

        let expected = database.get_history(GetHistoryArgs {
            hist_type: MangaHistoryType::PlanToRead,
            page: 1,
            search: None,
            items_per_page: 10,
            provider: MangaProviders::Mangadex,
        })?;

        /* now only one should exist */
        assert_eq!(expected.mangas.len(), 1);

        Ok(())
    }

    #[test]
    fn it_delets_all_mangas_from_history() -> Result<(), Box<dyn Error>> {
        let connection = Database::get_connection()?;
        let database = Database::new(&connection);
        database.setup()?;

        let manga_id_mangadex = Uuid::new_v4().to_string();
        let manga_id_mangadex2 = Uuid::new_v4().to_string();
        let manga_id_mangadex3 = Uuid::new_v4().to_string();

        let manga_id_plan_to_read = Uuid::new_v4().to_string();
        let manga_id_plan_to_read2 = Uuid::new_v4().to_string();

        let manga_id_weeb_central = Uuid::new_v4().to_string();
        let manga_id_weeb_centra2 = Uuid::new_v4().to_string();

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_id_mangadex,
            title: "of mangadex 1",
            img_url: None,
            provider: MangaProviders::Mangadex,
        })?;

        database.insert_manga_in_reading_history(&manga_id_mangadex)?;

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_id_mangadex2,
            title: "of mangadex 2",
            img_url: None,
            provider: MangaProviders::Mangadex,
        })?;

        database.insert_manga_in_reading_history(&manga_id_mangadex2)?;

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_id_mangadex3,
            title: "of mangadex 3",
            img_url: None,
            provider: MangaProviders::Mangadex,
        })?;

        database.insert_manga_in_reading_history(&manga_id_mangadex3)?;

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_id_weeb_central,
            title: "of weebcentral",
            img_url: None,
            provider: MangaProviders::Weebcentral,
        })?;

        database.insert_manga_in_reading_history(&manga_id_weeb_central)?;

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_id_weeb_centra2,
            title: "of weebcentral #2",
            img_url: None,
            provider: MangaProviders::Weebcentral,
        })?;

        database.insert_manga_in_reading_history(&manga_id_weeb_centra2)?;

        let expected = database.get_history(GetHistoryArgs {
            hist_type: MangaHistoryType::ReadingHistory,
            page: 1,
            search: None,
            items_per_page: 10,
            provider: MangaProviders::Mangadex,
        })?;

        /* at this point 3 mangas must be stored in reading history */
        assert_eq!(expected.mangas.len(), 3);

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_id_plan_to_read,
            title: "of mangadex 1",
            img_url: None,
            provider: MangaProviders::Mangadex,
        })?;

        database.save_plan_to_read(MangaPlanToReadSave {
            id: &manga_id_plan_to_read,
            title: "of mangadex 1 plan to read",
            img_url: None,
            provider: MangaProviders::Mangadex,
        })?;

        database.create_manga_if_not_exists(MangaInsert {
            id: &manga_id_plan_to_read2,
            title: "of mangadex 1",
            img_url: None,
            provider: MangaProviders::Mangadex,
        })?;

        database.save_plan_to_read(MangaPlanToReadSave {
            id: &manga_id_plan_to_read2,
            title: "of mangadex 2 plan to read",
            img_url: None,
            provider: MangaProviders::Mangadex,
        })?;

        let expected = database.get_history(GetHistoryArgs {
            hist_type: MangaHistoryType::PlanToRead,
            page: 1,
            search: None,
            items_per_page: 10,
            provider: MangaProviders::Mangadex,
        })?;

        /* at this point 2 mangas in plant to read should exist */
        assert_eq!(expected.mangas.len(), 2);

        database.remove_all_from_history(MangaHistoryType::ReadingHistory, MangaProviders::Mangadex)?;

        let expected = database.get_history(GetHistoryArgs {
            hist_type: MangaHistoryType::ReadingHistory,
            page: 1,
            search: None,
            items_per_page: 10,
            provider: MangaProviders::Mangadex,
        })?;

        /* now none from mangadex should exist */
        assert_eq!(expected.mangas.len(), 0);

        let expected = database.get_history(GetHistoryArgs {
            hist_type: MangaHistoryType::ReadingHistory,
            page: 1,
            search: None,
            items_per_page: 10,
            provider: MangaProviders::Weebcentral,
        })?;

        /* weebcentral reading history should remain untouched */
        assert_eq!(expected.mangas.len(), 2);

        let expected = database.get_history(GetHistoryArgs {
            hist_type: MangaHistoryType::PlanToRead,
            page: 1,
            search: None,
            items_per_page: 10,
            provider: MangaProviders::Mangadex,
        })?;

        /* the plan to read history should remain untouched */
        assert_eq!(expected.mangas.len(), 2);

        Ok(())
    }
}
