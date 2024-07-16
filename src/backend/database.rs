use super::APP_DATA_DIR;
use once_cell::sync::Lazy;
use rusqlite::{params, Connection};
use std::sync::Mutex;
use strum::Display;

pub static DBCONN: Lazy<Mutex<Option<Connection>>> = Lazy::new(|| {
    let conn = Connection::open(APP_DATA_DIR.as_ref().unwrap().join("manga-tui-history.db"));

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

    let already_has_data: i32 = conn
        .query_row("SELECT COUNT(*) from app_version", [], |row| row.get(0))
        .unwrap();

    if already_has_data == 0 {
        conn.execute(
            "INSERT INTO app_version(version) VALUES (?1) ",
            [env!("CARGO_PKG_VERSION")],
        )
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

    let already_has_data: i32 = conn
        .query_row("SELECT COUNT(*) from history_types", [], |row| row.get(0))
        .unwrap();

    if already_has_data < 2 {
        conn.execute(
            "INSERT INTO history_types(name) VALUES (?1) ",
            [MangaHistoryType::ReadingHistory.to_string()],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO history_types(name) VALUES (?1) ",
            [MangaHistoryType::PlanToRead.to_string()],
        )
        .unwrap();
    }

    Mutex::new(Some(conn))
});

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

struct Manga {
    id: String,
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

    let is_already_reading: i32 = conn.query_row(
        "SELECT COUNT(*) FROM manga_history_union WHERE manga_id = ?1 AND type_id = ?2",
        params![manga_read.id, history_type],
        |row| row.get(0),
    )?;

    let mut manga_exists_already_exists_statement =
        conn.prepare("SELECT id FROM mangas WHERE id = ?1")?;

    let mut manga_exists = manga_exists_already_exists_statement
        .query_map(params![manga_read.id], |row| Ok(Manga { id: row.get(0)? }))?;

    // Check if manga already exists in table mangas
    if let Some(manga) = manga_exists.next() {
        let manga = manga?;
        conn.execute(
            "INSERT INTO chapters VALUES (?1, ?2, ?3)",
            (manga_read.chapter_id, manga_read.chapter_title, manga.id),
        )?;

        if is_already_reading == 0 {
            conn.execute(
                "INSERT INTO manga_history_union VALUES (?1, ?2)",
                (manga_read.id, history_type),
            )?;
        }
        return Ok(());
    }

    conn.execute(
        "INSERT INTO mangas(id, title, img_url) VALUES (?1, ?2, ?3)",
        (manga_read.id, manga_read.title, manga_read.img_url),
    )?;

    conn.execute(
        "INSERT INTO manga_history_union VALUES (?1, ?2)",
        (manga_read.id, history_type),
    )?;

    conn.execute(
        "INSERT INTO chapters VALUES (?1, ?2, ?3)",
        (
            manga_read.chapter_id,
            manga_read.chapter_title,
            manga_read.id,
        ),
    )?;

    Ok(())
}

pub struct MangaReadingHistoryRetrieve {
    pub id: String,
}

pub fn get_chapters_read(id: &str) -> rusqlite::Result<Vec<MangaReadingHistoryRetrieve>> {
    let binding = DBCONN.lock().unwrap();
    let conn = binding.as_ref().unwrap();

    let mut chapter_ids: Vec<MangaReadingHistoryRetrieve> = vec![];

    let mut result = conn
        .prepare("SELECT chapters.id from chapters INNER JOIN mangas ON mangas.id = chapters.manga_id WHERE mangas.id = ?1")?;

    let result_iter = result.query_map(params![id], |row| {
        Ok(MangaReadingHistoryRetrieve { id: row.get(0)? })
    })?;

    for chapter_id in result_iter {
        if let Ok(id) = chapter_id {
            chapter_ids.push(id);
        }
    }

    Ok(chapter_ids)
}

pub struct MangaHistory {
    pub id: String,
    pub title: String,
    // img_url: Option<String>,
}

/// This is used in the feed page to retrieve the mangas the user is currently reading
pub fn get_history(
    hist_type: MangaHistoryType,
    offset: u32,
) -> rusqlite::Result<(Vec<MangaHistory>, u32)> {
    let offset = (offset - 1) * 5;
    let binding = DBCONN.lock().unwrap();
    let conn = binding.as_ref().unwrap();

    let history_type_id: i32 = conn.query_row(
        "SELECT id from history_types WHERE name = ?1",
        params![hist_type.to_string()],
        |row| row.get(0),
    )?;

    let total_mangas: u32 = conn.query_row(
        "
                SELECT COUNT(*) from mangas
                INNER JOIN manga_history_union ON mangas.id = manga_history_union.manga_id 
                WHERE manga_history_union.type_id = ?1",
        params![history_type_id],
        |row| row.get(0),
    )?;

    let mut statement = conn.prepare(
        format!(
            "SELECT  mangas.id, mangas.title from mangas 
                     INNER JOIN manga_history_union ON mangas.id = manga_history_union.manga_id 
                     WHERE manga_history_union.type_id = ?1
                    ORDER BY mangas.created_at DESC
                     LIMIT 5 OFFSET {}",
            offset
        )
        .as_str(),
    )?;

    let mut manga_history: Vec<MangaHistory> = vec![];

    let iter_mangas = statement.query_map(params![history_type_id], |row| {
        Ok(MangaHistory {
            id: row.get(0)?,
            title: row.get(1)?,
            // img_url: row.get(2)?,
        })
    })?;

    for manga in iter_mangas {
        manga_history.push(manga?);
    }

    Ok((manga_history, total_mangas))
}

pub struct MangaPlanToReadSave<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub img_url: Option<&'a str>,
}

pub fn save_plan_to_read(manga: MangaPlanToReadSave<'_>) -> rusqlite::Result<()> {
    let binding = DBCONN.lock().unwrap();
    let conn = binding.as_ref().unwrap();

    let history_type: i32 = conn.query_row(
        "SELECT id FROM history_types where name = ?1",
        params![MangaHistoryType::PlanToRead.to_string()],
        |row| row.get(0),
    )?;

    let is_already_plan_to_read: i32 = conn.query_row(
        "SELECT COUNT(*) FROM manga_history_union WHERE manga_id = ?1 AND type_id = ?2",
        params![manga.id, history_type],
        |row| row.get(0),
    )?;

    if is_already_plan_to_read == 0 {
        let mut manga_exists_already_exists_statement =
            conn.prepare("SELECT id FROM mangas WHERE id = ?1")?;

        let mut manga_exists = manga_exists_already_exists_statement
            .query_map(params![manga.id], |row| Ok(Manga { id: row.get(0)? }))?;

        if let Some(manga) = manga_exists.next() {
            let manga = manga?;
            conn.execute(
                "INSERT INTO manga_history_union VALUES (?1, ?2)",
                (manga.id, history_type),
            )?;
            return Ok(());
        }

        conn.execute(
            "INSERT INTO mangas(id, title, img_url) VALUES (?1, ?2, ?3)",
            (manga.id, manga.title, manga.img_url),
        )?;

        conn.execute(
            "INSERT INTO manga_history_union VALUES (?1, ?2)",
            (manga.id, history_type),
        )?;

        return Ok(());
    }
    Ok(())
}
