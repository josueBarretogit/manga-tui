use std::sync::Mutex;

use chrono::Utc;
use manga_tui::SearchTerm;
use once_cell::sync::Lazy;
use rusqlite::{params, Connection};
use strum::{Display, EnumIter};

#[cfg(not(test))]
use super::AppDirectories;
use crate::view::widgets::feed::FeedTabs;

#[derive(Display, Debug, Clone, Copy)]
pub enum MangaHistoryType {
    PlanToRead,
    ReadingHistory,
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

pub static DBCONN: Lazy<Mutex<Option<Connection>>> = Lazy::new(|| {
    #[cfg(not(test))]
    let conn = Connection::open(AppDirectories::History.get_full_path());

    #[cfg(test)]
    let conn = Connection::open_in_memory();

    if conn.is_err() {
        return Mutex::new(None);
    }

    let conn = conn.unwrap();

    conn.execute(
        "CREATE TABLE if not exists app_version (
                version TEXT PRIMARY KEY
             )",
        (),
    )
    .unwrap();

    let already_has_data: i32 = conn.query_row("SELECT COUNT(*) from app_version", [], |row| row.get(0)).unwrap();

    if already_has_data == 0 {
        conn.execute("INSERT INTO app_version(version) VALUES (?1) ", [env!("CARGO_PKG_VERSION")])
            .unwrap();
    }

    conn.execute(
        "CREATE TABLE if not exists history_types (
                id    INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE
             )",
        (),
    )
    .unwrap();

    conn.execute(
        "CREATE TABLE if not exists mangas (
                id    TEXT  PRIMARY KEY,
                title TEXT  NOT NULL,
                created_at  DATETIME DEFAULT (datetime('now')),
                updated_at  DATETIME DEFAULT (datetime('now')),
                last_read  DATETIME DEFAULT (datetime('now')),
                deleted_at  DATETIME NULL,
                img_url TEXT NULL
             )",
        (),
    )
    .unwrap();

    conn.execute(
        "CREATE TABLE if not exists chapters (
                id    TEXT  PRIMARY KEY,
                title TEXT  NOT NULL,
                manga_id TEXT  NOT NULL,
                is_read BOOLEAN NOT NULL DEFAULT 0,
                is_downloaded BOOLEAN NOT NULL DEFAULT 0,
                FOREIGN KEY (manga_id) REFERENCES mangas (id)
            )",
        (),
    )
    .unwrap();

    conn.execute(
        "CREATE TABLE if not exists manga_history_union (
                manga_id TEXT, 
                type_id INTEGER, 
                PRIMARY KEY (manga_id, type_id),
                FOREIGN KEY (manga_id) REFERENCES mangas (id),
                FOREIGN KEY (type_id) REFERENCES history_types (id)
             )",
        (),
    )
    .unwrap();

    let already_has_data: i32 = conn.query_row("SELECT COUNT(*) from history_types", [], |row| row.get(0)).unwrap();

    if already_has_data < 2 {
        conn.execute("INSERT INTO history_types(name) VALUES (?1) ", [MangaHistoryType::ReadingHistory.to_string()])
            .unwrap();

        conn.execute("INSERT INTO history_types(name) VALUES (?1) ", [MangaHistoryType::PlanToRead.to_string()])
            .unwrap();
    }

    Mutex::new(Some(conn))
});

/// check if a value exists in a table
fn check_exists(id: &str, conn: &Connection, table: Table) -> rusqlite::Result<bool> {
    let table = table.to_string();
    let exists: bool = conn.query_row(
        format!("SELECT EXISTS(SELECT id FROM {table} WHERE id = ?1) as row_exists").as_str(),
        rusqlite::params![id],
        |row| row.get(0),
    )?;
    Ok(exists)
}

fn manga_is_reading(id: &str, conn: &Connection) -> rusqlite::Result<bool> {
    let history_type: i32 = conn.query_row(
        "SELECT id FROM history_types where name = ?1",
        params![MangaHistoryType::ReadingHistory.to_string()],
        |row| row.get(0),
    )?;

    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT * FROM manga_history_union WHERE manga_id = ?1 AND type_id = ?2)",
        rusqlite::params![id, history_type],
        |row| row.get(0),
    )?;

    Ok(exists)
}

#[derive(Clone)]
pub struct MangaInsert<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub img_url: Option<&'a str>,
}

fn insert_manga(manga_to_insert: MangaInsert<'_>, conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO mangas(id, title, img_url) VALUES (?1, ?2, ?3)",
        (manga_to_insert.id, manga_to_insert.title, manga_to_insert.img_url),
    )?;
    Ok(())
}

#[derive(Clone)]
pub struct ChapterInsert<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub manga_id: &'a str,
    pub is_read: bool,
    pub is_downloaded: bool,
}

fn insert_chapter(chap: ChapterInsert<'_>, conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO chapters(id, title, is_read, is_downloaded, manga_id) VALUES (?1, ?2, ?3, ?4, ?5)",
        (chap.id, chap.title, chap.is_read, chap.is_downloaded, chap.manga_id),
    )?;
    Ok(())
}

fn insert_manga_in_reading_history(manga_id: &str, conn: &Connection) -> rusqlite::Result<()> {
    let reading_history: i32 = conn.query_row(
        "SELECT id FROM history_types where name = ?1",
        params![MangaHistoryType::ReadingHistory.to_string()],
        |row| row.get(0),
    )?;

    conn.execute("INSERT INTO manga_history_union VALUES (?1, ?2)", (manga_id, reading_history))?;
    Ok(())
}

/// Insert a manga in the reading history type or update the `last_read` field
fn update_or_insert_manga_most_recent_read(manga_id: &str, conn: &Connection) -> rusqlite::Result<()> {
    if !manga_is_reading(manga_id, conn)? {
        insert_manga_in_reading_history(manga_id, conn)?;
        Ok(())
    } else {
        let now = Utc::now().naive_utc();
        conn.execute("UPDATE mangas SET last_read = ?1 WHERE id = ?2", params![now.to_string(), manga_id])?;
        Ok(())
    }
}

pub struct MangaReadingHistorySave<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub img_url: Option<&'a str>,
    pub chapter_id: &'a str,
    pub chapter_title: &'a str,
    pub is_already_reading: bool,
}

/// This function creates a manga in the database if it does not exists and saves it in the reading
/// history section
pub fn save_history(chapter: MangaReadingHistorySave<'_>, conn: &Connection) -> rusqlite::Result<()> {
    if chapter.is_already_reading {
        return Ok(());
    }

    if check_exists(chapter.id, conn, Table::Mangas)? {
        insert_chapter(
            ChapterInsert {
                id: chapter.chapter_id,
                title: chapter.chapter_title,
                is_downloaded: false,
                is_read: true,
                manga_id: chapter.id,
            },
            conn,
        )?;

        if !manga_is_reading(chapter.id, conn)? {
            insert_manga_in_reading_history(chapter.id, conn)?;
        } else {
            let now = Utc::now().naive_utc();
            conn.execute("UPDATE mangas SET last_read = ?1 WHERE id = ?2", params![now.to_string(), chapter.id])?;
        }
        return Ok(());
    }

    insert_manga(
        MangaInsert {
            id: chapter.id,
            title: chapter.title,
            img_url: chapter.img_url,
        },
        conn,
    )?;

    insert_manga_in_reading_history(chapter.id, conn)?;

    insert_chapter(
        ChapterInsert {
            id: chapter.chapter_id,
            title: chapter.chapter_title,
            is_read: true,
            is_downloaded: false,
            manga_id: chapter.id,
        },
        conn,
    )?;

    Ok(())
}

pub struct MangaReadingHistoryRetrieve {
    pub id: String,
    pub is_downloaded: bool,
    pub is_read: bool,
}

// retrieve the `is_reading` and `is_downloaded` data for a chapter
pub fn get_chapters_history_status(manga_id: &str, conn: &Connection) -> rusqlite::Result<Vec<MangaReadingHistoryRetrieve>> {
    let mut chapter_ids: Vec<MangaReadingHistoryRetrieve> = vec![];

    let mut result = conn
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

pub struct GetHistoryArgs<'a> {
    pub conn: &'a Connection,
    pub hist_type: MangaHistoryType,
    pub page: u32,
    pub search: Option<SearchTerm>,
    pub items_per_page: u32,
}
/// This is used in the `feed` page to retrieve the mangas the user is currently reading
pub fn get_history(args: GetHistoryArgs<'_>) -> rusqlite::Result<MangaHistoryResponse> {
    let items_per_page = args.items_per_page;
    let offset = (args.page - 1) * items_per_page;

    let history_type_id: i32 =
        args.conn
            .query_row("SELECT id from history_types WHERE name = ?1", params![args.hist_type.to_string()], |row| row.get(0))?;

    let total_mangas: u32 = args.conn.query_row(
        "
                SELECT COUNT(*) from mangas
                INNER JOIN manga_history_union ON mangas.id = manga_history_union.manga_id 
                WHERE manga_history_union.type_id = ?1",
        params![history_type_id],
        |row| row.get(0),
    )?;

    let mut get_statement = args.conn.prepare(
        "SELECT  mangas.id, mangas.title from mangas 
                     INNER JOIN manga_history_union ON mangas.id = manga_history_union.manga_id 
                     WHERE manga_history_union.type_id = ?1
                     ORDER BY mangas.last_read DESC
                     LIMIT ?2 OFFSET ?3",
    )?;

    let mut get_statement_with_search_term = args.conn.prepare(
        "SELECT  mangas.id, mangas.title from mangas 
                     INNER JOIN manga_history_union ON mangas.id = manga_history_union.manga_id 
                     WHERE manga_history_union.type_id = ?1 AND LOWER(mangas.title) LIKE '%' || ?2 || '%'
                     ORDER BY mangas.last_read DESC
                     LIMIT ?3 OFFSET ?4",
    )?;

    let mut manga_history: Vec<MangaHistory> = vec![];

    if let Some(search_term) = args.search {
        let search_term = search_term.get();
        let total_mangas_with_search: u32 = args.conn.query_row(
            "
                SELECT COUNT(*) from mangas
                INNER JOIN manga_history_union ON mangas.id = manga_history_union.manga_id 
                WHERE manga_history_union.type_id = ?1 AND LOWER(mangas.title) LIKE '%' || ?2 || '%'",
            params![history_type_id, search_term],
            |row| row.get(0),
        )?;
        let iter_mangas =
            get_statement_with_search_term.query_map(params![history_type_id, search_term, items_per_page, offset], |row| {
                Ok(MangaHistory {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    // img_url: row.get(2)?,
                })
            })?;

        for manga in iter_mangas {
            manga_history.push(manga?);
        }

        return Ok(MangaHistoryResponse {
            mangas: manga_history,
            total_items: total_mangas_with_search,
            page: args.page,
        });
    }

    let iter_mangas = get_statement.query_map(params![history_type_id, items_per_page, offset], |row| {
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

pub struct MangaPlanToReadSave<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub img_url: Option<&'a str>,
}

fn manga_is_plan_to_read(manga_id: &str, conn: &Connection) -> rusqlite::Result<bool> {
    let history_type = get_history_type(MangaHistoryType::PlanToRead, conn)?;
    let is_already_plan_to_read: bool = conn.query_row(
        "SELECT EXISTS(SELECT * FROM manga_history_union WHERE manga_id = ?1 AND type_id = ?2)",
        params![manga_id, history_type],
        |row| row.get(0),
    )?;

    Ok(is_already_plan_to_read)
}

fn get_history_type(hist_type: MangaHistoryType, conn: &Connection) -> rusqlite::Result<i32> {
    let history_type_id: i32 =
        conn.query_row("SELECT id FROM history_types where name = ?1", params![hist_type.to_string()], |row| row.get(0))?;
    Ok(history_type_id)
}

pub fn save_plan_to_read(manga: MangaPlanToReadSave<'_>, conn: &Connection) -> rusqlite::Result<()> {
    let history_type = get_history_type(MangaHistoryType::PlanToRead, conn)?;

    if !manga_is_plan_to_read(manga.id, conn)? {
        if check_exists(manga.id, conn, Table::Mangas)? {
            conn.execute("INSERT INTO manga_history_union VALUES (?1, ?2)", (manga.id, history_type))?;
            return Ok(());
        }

        insert_manga(
            MangaInsert {
                id: manga.id,
                title: manga.title,
                img_url: manga.img_url,
            },
            conn,
        )?;

        conn.execute("INSERT INTO manga_history_union VALUES (?1, ?2)", (manga.id, history_type))?;

        return Ok(());
    }
    Ok(())
}

pub struct SetChapterDownloaded<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub manga_id: &'a str,
    pub manga_title: &'a str,
    pub img_url: Option<&'a str>,
}

// a chapter cannot exist if a manga doesnt exist
// therefore if manga exists chapter exists

// First check if the chapters is already in the database, if not insert it, or else update and set
// its download status to true
pub fn set_chapter_downloaded(chapter: SetChapterDownloaded<'_>, conn: &Connection) -> rusqlite::Result<()> {
    if check_exists(chapter.manga_id, conn, Table::Mangas)? {
        update_or_insert_manga_most_recent_read(chapter.manga_id, conn)?;

        if check_exists(chapter.id, conn, Table::Chapters)? {
            conn.execute("UPDATE chapters SET is_downloaded = ?1, is_read = ?2 WHERE id = ?3", params![true, true, chapter.id])?;
        } else {
            insert_chapter(
                ChapterInsert {
                    id: chapter.id,
                    title: chapter.title,
                    manga_id: chapter.manga_id,
                    is_read: true,
                    is_downloaded: true,
                },
                conn,
            )?;
        }

        Ok(())
    } else {
        insert_manga(
            MangaInsert {
                id: chapter.manga_id,
                title: chapter.manga_title,
                img_url: chapter.img_url,
            },
            conn,
        )?;

        insert_chapter(
            ChapterInsert {
                id: chapter.id,
                title: chapter.title,
                manga_id: chapter.manga_id,
                is_read: true,
                is_downloaded: true,
            },
            conn,
        )?;

        insert_manga_in_reading_history(chapter.manga_id, conn)?;

        Ok(())
    }
}

#[cfg(test)]
mod test {

    use pretty_assertions::assert_eq;
    use rusqlite::Result;
    use strum::IntoEnumIterator;
    use uuid::Uuid;

    use super::*;

    fn check_tables_exist(connection: &Connection) -> Result<()> {
        for table in Table::iter() {
            connection.query_row(
                format!("SELECT name FROM sqlite_master WHERE type='table' AND name='{}'", table).as_str(),
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
        let binding = DBCONN.lock().expect("could not get db conn");
        let connection = binding.as_ref();
        assert!(connection.is_some());

        let connection = connection.unwrap();

        check_tables_exist(connection)?;

        Ok(())
    }

    #[test]
    fn insert_manga_and_chapter() -> Result<()> {
        let binding = DBCONN.lock().expect("could not get db conn");
        let connection = binding.as_ref().unwrap();

        let manga_id = Uuid::new_v4().to_string();

        let chapter_id = Uuid::new_v4().to_string();

        insert_manga(
            MangaInsert {
                id: &manga_id,
                title: "some_title",
                img_url: None,
            },
            connection,
        )?;

        insert_chapter(
            ChapterInsert {
                id: &chapter_id,
                title: "some_title",
                manga_id: &manga_id,
                is_read: true,
                is_downloaded: false,
            },
            connection,
        )?;

        Ok(())
    }

    #[test]
    fn save_manga_plan_to_read_which_does_not_exist() -> Result<()> {
        let binding = DBCONN.lock().expect("could not get db conn");
        let connection = binding.as_ref().unwrap();
        let manga_id = Uuid::new_v4().to_string();

        save_plan_to_read(
            MangaPlanToReadSave {
                id: &manga_id,
                title: "some_title",
                img_url: None,
            },
            connection,
        )?;

        let manga_was_saved = check_exists(&manga_id, connection, Table::Mangas)?;

        assert!(manga_was_saved, "manga should have been saved");

        let manga_is_plan_to_read = manga_is_plan_to_read(&manga_id, connection)?;

        assert!(manga_is_plan_to_read, "the manga was not plan to read");

        Ok(())
    }

    #[test]
    fn save_already_existing_manga_plan_to_read() -> Result<()> {
        let binding = DBCONN.lock().expect("could not get db conn");
        let connection = binding.as_ref().unwrap();
        let manga_id = Uuid::new_v4().to_string();

        insert_manga(
            MangaInsert {
                id: &manga_id,
                title: "some_title",
                img_url: None,
            },
            connection,
        )?;

        let manga_should_not_be_plan_to_read = manga_is_plan_to_read(&manga_id, connection)?;

        assert!(!manga_should_not_be_plan_to_read);

        save_plan_to_read(
            MangaPlanToReadSave {
                id: &manga_id,
                title: "some_title",
                img_url: None,
            },
            connection,
        )?;

        let manga_should_be_plan_to_read = manga_is_plan_to_read(&manga_id, connection)?;

        assert!(manga_should_be_plan_to_read);

        Ok(())
    }

    // Both manga and chapter are not in the database
    #[test]
    fn save_manga_reading_status_which_does_not_exist() -> Result<()> {
        let binding = DBCONN.lock().expect("could not get db conn");
        let connection = binding.as_ref().unwrap();

        let manga_id = Uuid::new_v4().to_string();
        let chapter_id = Uuid::new_v4().to_string();

        save_history(
            MangaReadingHistorySave {
                id: &manga_id,
                title: "some_title",
                img_url: None,
                chapter_id: &chapter_id,
                chapter_title: "some_chapter_title",
                is_already_reading: false,
            },
            connection,
        )?;

        let manga_was_created = check_exists(&manga_id, connection, Table::Mangas)?;
        let chapter_was_created = check_exists(&chapter_id, connection, Table::Chapters)?;

        assert!(manga_was_created);

        assert!(chapter_was_created);

        Ok(())
    }

    // manga is already in database, chapter isnt
    #[test]
    fn save_manga_reading_status_which_already_exists() -> Result<()> {
        let binding = DBCONN.lock().expect("could not get db conn");
        let connection = binding.as_ref().unwrap();

        let manga_id = Uuid::new_v4().to_string();
        let chapter_id = Uuid::new_v4().to_string();

        insert_manga(
            MangaInsert {
                id: &manga_id,
                title: "some_title",
                img_url: None,
            },
            connection,
        )?;

        save_history(
            MangaReadingHistorySave {
                id: &manga_id,
                title: "some_title",
                img_url: None,
                chapter_id: &chapter_id,
                chapter_title: "some_chapter_title",
                is_already_reading: false,
            },
            connection,
        )?;

        let chapters = get_all_chapters(connection)?;

        let saved_chapter = chapters
            .iter()
            .find(|chap| chap.id == chapter_id)
            .expect("no chapter was saved as being read");

        assert!(saved_chapter.is_read);

        Ok(())
    }

    #[test]
    fn save_manga_reading_both_manga_and_chapter_exist_and_chapter_is_already_reading() -> Result<()> {
        let binding = DBCONN.lock().expect("could not get db conn");
        let connection = binding.as_ref().unwrap();

        let manga_id = Uuid::new_v4().to_string();
        let chapter_id = Uuid::new_v4().to_string();

        insert_manga(
            MangaInsert {
                id: &manga_id,
                title: "some_title",
                img_url: None,
            },
            connection,
        )?;

        let chapter_which_is_already_reading = ChapterInsert {
            id: &chapter_id,
            title: "some_title",
            manga_id: &manga_id,
            is_read: true,
            is_downloaded: true,
        };

        insert_chapter(chapter_which_is_already_reading.clone(), connection)?;

        save_history(
            MangaReadingHistorySave {
                id: &manga_id,
                title: chapter_which_is_already_reading.title,
                img_url: None,
                chapter_id: chapter_which_is_already_reading.id,
                chapter_title: chapter_which_is_already_reading.title,
                is_already_reading: chapter_which_is_already_reading.is_read,
            },
            connection,
        )?;

        let chapters = get_all_chapters(connection)?;

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
        let binding = DBCONN.lock().expect("could not get db conn");
        let connection = binding.as_ref().unwrap();

        let manga_id = Uuid::new_v4().to_string();
        let chapter_id_not_read = Uuid::new_v4().to_string();
        let chapter_id_read = Uuid::new_v4().to_string();

        insert_manga(
            MangaInsert {
                id: &manga_id,
                title: "some_manga",
                img_url: None,
            },
            connection,
        )?;

        insert_chapter(
            ChapterInsert {
                id: &chapter_id_not_read,
                title: "some_chapter",
                manga_id: &manga_id,
                is_read: false,
                is_downloaded: false,
            },
            connection,
        )?;

        insert_chapter(
            ChapterInsert {
                id: &chapter_id_read,
                title: "some_chapter",
                manga_id: &manga_id,
                is_read: true,
                is_downloaded: false,
            },
            connection,
        )?;

        let chapters = get_chapters_history_status(&manga_id, connection)?;

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
        let binding = DBCONN.lock().expect("could not get db conn");
        let connection = binding.as_ref().unwrap();

        let manga_is_in_reading_history_id = Uuid::new_v4().to_string();
        let manga_not_in_reading_history_id = Uuid::new_v4().to_string();

        insert_manga(
            MangaInsert {
                id: &manga_is_in_reading_history_id,
                title: "some_title",
                img_url: None,
            },
            connection,
        )?;

        insert_manga_in_reading_history(&manga_is_in_reading_history_id, connection)?;

        insert_manga(
            MangaInsert {
                id: &manga_not_in_reading_history_id,
                title: "some_title",
                img_url: None,
            },
            connection,
        )?;

        let history = get_history(GetHistoryArgs {
            conn: connection,
            hist_type: MangaHistoryType::ReadingHistory,
            page: 1,
            search: None,
            items_per_page: 100,
        })?;

        assert!(history.total_items > 0);

        assert!(history.mangas.iter().any(|manga| manga.id == manga_is_in_reading_history_id));

        assert!(!history.mangas.iter().any(|manga| manga.id == manga_not_in_reading_history_id));

        Ok(())
    }

    #[test]
    fn get_manga_history_reading_with_search_term() -> Result<()> {
        let binding = DBCONN.lock().expect("could not get db conn");
        let connection = binding.as_ref().unwrap();

        let manga_id_filtered_out = Uuid::new_v4().to_string();
        let manga_id_included_in_search = Uuid::new_v4().to_string();

        insert_manga(
            MangaInsert {
                id: &manga_id_filtered_out,
                title: "filtered_out",
                img_url: None,
            },
            connection,
        )?;

        insert_manga_in_reading_history(&manga_id_filtered_out, connection)?;

        insert_manga(
            MangaInsert {
                id: &manga_id_included_in_search,
                title: "included",
                img_url: None,
            },
            connection,
        )?;

        insert_manga_in_reading_history(&manga_id_included_in_search, connection)?;

        let history = get_history(GetHistoryArgs {
            conn: connection,
            hist_type: MangaHistoryType::ReadingHistory,
            page: 1,
            search: SearchTerm::trimmed_lowercased("Included"),
            items_per_page: 100,
        })?;

        assert!(history.total_items > 0);

        assert!(!history.mangas.iter().any(|manga| manga.id == manga_id_filtered_out));
        assert!(history.mangas.iter().any(|manga| manga.id == manga_id_included_in_search));

        Ok(())
    }

    #[test]
    fn get_manga_planned_to_read_with_search_term() -> Result<()> {
        let binding = DBCONN.lock().expect("could not get db conn");
        let connection = binding.as_ref().unwrap();

        let manga_id_filtered_out = Uuid::new_v4().to_string();
        let manga_id_included_in_search = Uuid::new_v4().to_string();

        let manga_filtered_out = MangaPlanToReadSave {
            id: &manga_id_filtered_out,
            title: "filtered_out",
            img_url: None,
        };

        let manga_included = MangaPlanToReadSave {
            id: &manga_id_included_in_search,
            title: "included",
            img_url: None,
        };

        save_plan_to_read(manga_filtered_out, connection)?;

        save_plan_to_read(manga_included, connection)?;

        let history = get_history(GetHistoryArgs {
            conn: connection,
            hist_type: MangaHistoryType::PlanToRead,
            page: 1,
            search: SearchTerm::trimmed_lowercased("Included"),
            items_per_page: 100,
        })?;

        assert!(history.total_items > 0);

        assert!(!history.mangas.iter().any(|manga| manga.id == manga_id_filtered_out));
        assert!(history.mangas.iter().any(|manga| manga.id == manga_id_included_in_search));

        Ok(())
    }
    #[test]
    fn get_manga_planned_to_read() -> Result<()> {
        let binding = DBCONN.lock().expect("could not get db conn");
        let connection = binding.as_ref().unwrap();

        let manga_id_1 = Uuid::new_v4().to_string();
        let manga_id_2 = Uuid::new_v4().to_string();

        let manga_1 = MangaPlanToReadSave {
            id: &manga_id_1,
            title: "manga_1",
            img_url: None,
        };

        let manga_2 = MangaPlanToReadSave {
            id: &manga_id_2,
            title: "manga_2",
            img_url: None,
        };

        save_plan_to_read(manga_1, connection)?;

        save_plan_to_read(manga_2, connection)?;

        let history = get_history(GetHistoryArgs {
            conn: connection,
            hist_type: MangaHistoryType::PlanToRead,
            page: 1,
            search: None,
            items_per_page: 100,
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
        let binding = DBCONN.lock().expect("could not get db conn");
        let connection = binding.as_ref().unwrap();

        let chapter_id = Uuid::new_v4().to_string();
        let manga_id = Uuid::new_v4().to_string();

        set_chapter_downloaded(
            SetChapterDownloaded {
                id: &chapter_id,
                title: "some_title",
                manga_id: &manga_id,
                manga_title: "some_title",
                img_url: None,
            },
            connection,
        )?;

        assert!(check_exists(&manga_id, connection, Table::Mangas)?);

        let chapters = get_all_chapters(connection)?;

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
        let binding = DBCONN.lock().expect("could not get db conn");
        let connection = binding.as_ref().unwrap();

        let chapter_id_exist_in_database = Uuid::new_v4().to_string();
        let manga_id_exist_in_database = Uuid::new_v4().to_string();

        insert_manga(
            MangaInsert {
                id: &manga_id_exist_in_database,
                title: "some_title",
                img_url: None,
            },
            connection,
        )?;

        insert_chapter(
            ChapterInsert {
                id: &chapter_id_exist_in_database,
                title: "some_title",
                manga_id: &manga_id_exist_in_database,
                is_read: false,
                is_downloaded: false,
            },
            connection,
        )?;

        set_chapter_downloaded(
            SetChapterDownloaded {
                id: &chapter_id_exist_in_database,
                title: "some_title",
                manga_id: &manga_id_exist_in_database,
                manga_title: "some_title",
                img_url: None,
            },
            connection,
        )?;

        let chapters = get_all_chapters(connection)?;

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
        let binding = DBCONN.lock().expect("could not get db conn");
        let connection = binding.as_ref().unwrap();

        let chapter_id = Uuid::new_v4().to_string();
        let manga_id = Uuid::new_v4().to_string();

        insert_manga(
            MangaInsert {
                id: &manga_id,
                title: "some_title",
                img_url: None,
            },
            connection,
        )?;

        set_chapter_downloaded(
            SetChapterDownloaded {
                id: &chapter_id,
                title: "some_title",
                manga_id: &manga_id,
                manga_title: "some_title",
                img_url: None,
            },
            connection,
        )?;

        let chapters = get_all_chapters(connection)?;

        let chapter_downloaded = chapters
            .iter()
            .find(|chap| chap.id == chapter_id)
            .expect("chapter downloaded was nost found");

        assert!(chapter_downloaded.is_read);
        assert!(chapter_downloaded.is_downloaded);

        Ok(())
    }
}
