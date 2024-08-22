use std::sync::Mutex;

use chrono::Utc;
use manga_tui::build_check_exists_function;
use once_cell::sync::Lazy;
use rusqlite::{params, Connection};
use strum::Display;

use super::{AppDirectories, APP_DATA_DIR};

// Todo! document database schema

pub static DBCONN: Lazy<Mutex<Option<Connection>>> = Lazy::new(|| {
    let conn = Connection::open(
        APP_DATA_DIR
            .as_ref()
            .unwrap()
            .join(AppDirectories::History.to_string())
            .join("manga-tui-history.db"),
    );

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

build_check_exists_function!(check_chapter_exists, "chapters");
build_check_exists_function!(check_manga_already_exists, "mangas");

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

fn update_or_insert_manga_most_recent_read(manga_id: &str, conn: &Connection) -> rusqlite::Result<()> {
    let history_type: i32 = conn.query_row(
        "SELECT id FROM history_types where name = ?1",
        params![MangaHistoryType::ReadingHistory.to_string()],
        |row| row.get(0),
    )?;

    if !manga_is_reading(manga_id, conn)? {
        conn.execute("INSERT INTO manga_history_union VALUES (?1, ?2)", (manga_id, history_type))?;
        Ok(())
    } else {
        let now = Utc::now().naive_utc();
        conn.execute("UPDATE mangas SET last_read = ?1 WHERE id = ?2", params![now.to_string(), manga_id])?;
        Ok(())
    }
}

#[derive(Display)]
pub enum MangaHistoryType {
    PlanToRead,
    ReadingHistory,
}

pub struct MangaReadingHistorySave<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub img_url: Option<&'a str>,
    pub chapter_id: &'a str,
    pub chapter_title: &'a str,
}

// if it's the first time the user is reading a manga then save it to mangas table and save the
// current chapter that is read, else just save the chapter and associate the manga,
pub fn save_history(manga_read: MangaReadingHistorySave<'_>) -> rusqlite::Result<()> {
    let binding = DBCONN.lock().unwrap();
    let conn = binding.as_ref().unwrap();

    let history_type: i32 = conn.query_row(
        "SELECT id FROM history_types where name = ?1",
        params![MangaHistoryType::ReadingHistory.to_string()],
        |row| row.get(0),
    )?;

    // Check if manga already exists in table mangas
    if check_manga_already_exists(manga_read.id, conn)? {
        insert_chapter(
            ChapterInsert {
                id: manga_read.chapter_id,
                title: manga_read.chapter_title,
                is_downloaded: false,
                is_read: true,
                manga_id: manga_read.id,
            },
            conn,
        )?;

        if !manga_is_reading(manga_read.id, conn)? {
            conn.execute("INSERT INTO manga_history_union VALUES (?1, ?2)", (manga_read.id, history_type))?;
        } else {
            let now = Utc::now().naive_utc();
            conn.execute("UPDATE mangas SET last_read = ?1 WHERE id = ?2", params![now.to_string(), manga_read.id])?;
        }
        return Ok(());
    }

    insert_manga(
        MangaInsert {
            id: manga_read.id,
            title: manga_read.title,
            img_url: manga_read.img_url,
        },
        conn,
    )?;

    conn.execute("INSERT INTO manga_history_union VALUES (?1, ?2)", (manga_read.id, history_type))?;

    insert_chapter(
        ChapterInsert {
            id: manga_read.chapter_id,
            title: manga_read.chapter_title,
            is_read: true,
            is_downloaded: false,
            manga_id: manga_read.id,
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
pub fn get_chapters_history_status(id: &str) -> rusqlite::Result<Vec<MangaReadingHistoryRetrieve>> {
    let binding = DBCONN.lock().unwrap();
    let conn = binding.as_ref().unwrap();

    let mut chapter_ids: Vec<MangaReadingHistoryRetrieve> = vec![];

    let mut result = conn
        .prepare("SELECT chapters.id, chapters.is_downloaded, chapters.is_read from chapters INNER JOIN mangas ON mangas.id = chapters.manga_id WHERE mangas.id = ?1")?;

    let result_iter = result.query_map(params![id], |row| {
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

pub struct MangaHistory {
    pub id: String,
    pub title: String,
    // img_url: Option<String>,
}

pub struct MangaHistoryResponse {
    pub mangas: Vec<MangaHistory>,
    pub page: u32,
    pub total_items: u32,
}
/// This is used in the `feed` page to retrieve the mangas the user is currently reading
pub fn get_history(hist_type: MangaHistoryType, page: u32, search: &str) -> rusqlite::Result<MangaHistoryResponse> {
    let offset = (page - 1) * 5;
    let binding = DBCONN.lock().unwrap();
    let conn = binding.as_ref().unwrap();

    let history_type_id: i32 =
        conn.query_row("SELECT id from history_types WHERE name = ?1", params![hist_type.to_string()], |row| row.get(0))?;

    let total_mangas: u32 = conn.query_row(
        "
                SELECT COUNT(*) from mangas
                INNER JOIN manga_history_union ON mangas.id = manga_history_union.manga_id 
                WHERE manga_history_union.type_id = ?1",
        params![history_type_id],
        |row| row.get(0),
    )?;

    let mut get_statement = conn.prepare(
        "SELECT  mangas.id, mangas.title from mangas 
                     INNER JOIN manga_history_union ON mangas.id = manga_history_union.manga_id 
                     WHERE manga_history_union.type_id = ?1
                     ORDER BY mangas.last_read DESC
                     LIMIT 5 OFFSET ?2",
    )?;

    let mut get_statement_with_search_term = conn.prepare(
        "SELECT  mangas.id, mangas.title from mangas 
                     INNER JOIN manga_history_union ON mangas.id = manga_history_union.manga_id 
                     WHERE manga_history_union.type_id = ?1 AND LOWER(mangas.title) LIKE '%' || ?2 || '%'
                     ORDER BY mangas.last_read DESC
                     LIMIT 5 OFFSET ?3",
    )?;

    let mut manga_history: Vec<MangaHistory> = vec![];

    if search.trim().is_empty() {
        let iter_mangas = get_statement.query_map(params![history_type_id, offset], |row| {
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
            page,
        })
    } else {
        let total_mangas_with_search: u32 = conn.query_row(
            "
                SELECT COUNT(*) from mangas
                INNER JOIN manga_history_union ON mangas.id = manga_history_union.manga_id 
                WHERE manga_history_union.type_id = ?1 AND LOWER(mangas.title) LIKE '%' || ?2 || '%'",
            params![history_type_id, search.trim().to_lowercase()],
            |row| row.get(0),
        )?;
        let iter_mangas =
            get_statement_with_search_term.query_map(params![history_type_id, search.trim().to_lowercase(), offset], |row| {
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
            total_items: total_mangas_with_search,
            page,
        })
    }
}

pub struct MangaPlanToReadSave<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub img_url: Option<&'a str>,
}

pub fn save_plan_to_read(manga: MangaPlanToReadSave<'_>) -> rusqlite::Result<()> {
    let binding = DBCONN.lock().unwrap();
    let conn = binding.as_ref().unwrap();

    let history_type: i32 =
        conn.query_row("SELECT id FROM history_types where name = ?1", params![MangaHistoryType::PlanToRead.to_string()], |row| {
            row.get(0)
        })?;

    let is_already_plan_to_read: bool = conn.query_row(
        "SELECT EXISTS(SELECT * FROM manga_history_union WHERE manga_id = ?1 AND type_id = ?2)",
        params![manga.id, history_type],
        |row| row.get(0),
    )?;

    if !is_already_plan_to_read {
        if check_manga_already_exists(manga.id, conn)? {
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

// First check if the chapters is already in the database, if not insert it, or else update and set
// its download status to true
pub fn set_chapter_downloaded(chapter: SetChapterDownloaded<'_>) -> rusqlite::Result<()> {
    let binding = DBCONN.lock().unwrap();
    let conn = binding.as_ref().unwrap();

    let history_type: i32 = conn.query_row(
        "SELECT id FROM history_types where name = ?1",
        params![MangaHistoryType::ReadingHistory.to_string()],
        |row| row.get(0),
    )?;

    if check_chapter_exists(chapter.id, conn)? && check_manga_already_exists(chapter.manga_id, conn)? {
        update_or_insert_manga_most_recent_read(chapter.manga_id, conn)?;
        conn.execute("UPDATE chapters SET is_downloaded = ?1, is_read = ?2 WHERE id = ?3", params![true, true, chapter.id])?;
        Ok(())
    } else if !check_manga_already_exists(chapter.manga_id, conn)? {
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

        conn.execute("INSERT INTO manga_history_union VALUES (?1, ?2)", (chapter.manga_id, history_type))?;

        Ok(())
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

        update_or_insert_manga_most_recent_read(chapter.manga_id, conn)?;

        Ok(())
    }
}
