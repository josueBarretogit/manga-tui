// save what mangas the user is reading and which chapters where read
// need a file to store that data,
// need to update it

use std::sync::Mutex;

use once_cell::sync::Lazy;
use rusqlite::{params, Connection};
use strum::Display;

pub static DBCONN: Lazy<Mutex<Option<Connection>>> = Lazy::new(|| {
    let conn = Connection::open("./db_test.db");

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

    conn.execute(
        "CREATE TABLE if not exists history_types (
                id    INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE
             )",
        (),
    )
    .unwrap();

    let already_has_data: i32 = conn
        .query_row("SELECT COUNT(*) from history_types", [], |row| row.get(0))
        .unwrap();

    if already_has_data == 0 {
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

    conn.execute(
        "CREATE TABLE if not exists mangas (
                id    TEXT  PRIMARY KEY,
                title TEXT  NOT NULL,
                img_url TEXT   NULL,
                manga_type_id INTEGER NOT NULL,
                FOREIGN KEY (manga_type_id) REFERENCES history_types (id)
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

    let mut manga_exists_statement = conn.prepare("SELECT id FROM mangas WHERE id = ?1")?;

    let mut manga_exists = manga_exists_statement
        .query_map(params![manga_read.id], |row| Ok(Manga { id: row.get(0)? }))?;

    if let Some(manga) = manga_exists.next() {
        let manga = manga?;
        conn.execute(
            "INSERT INTO chapters VALUES (?1, ?2, ?3)",
            (manga_read.chapter_id, manga_read.chapter_title, manga.id),
        )?;
        return Ok(());
    }

    let history_type: i32 = conn.query_row(
        "SELECT id FROM history_types where name = ?1",
        params![MangaHistoryType::ReadingHistory.to_string()],
        |row| row.get(0),
    )?;

    conn.execute(
        "INSERT INTO mangas VALUES (?1, ?2, ?3, ?4)",
        (
            manga_read.id,
            manga_read.title,
            manga_read.img_url,
            history_type,
        ),
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

pub fn get_history(
    hist_type: MangaHistoryType,
    offset: u32,
) -> rusqlite::Result<(Vec<MangaHistory>, u32)> {
    let offset = (offset - 1) * 5;
    let binding = DBCONN.lock().unwrap();
    let conn = binding.as_ref().unwrap();

    match hist_type {
        MangaHistoryType::ReadingHistory => {
            let total_mangas: u32 =
                conn.query_row("SELECT COUNT(*) from mangas", [], |row| row.get(0))?;
            let mut statement = conn.prepare(
                format!(
                    "SELECT  id, title from mangas WHERE manga_type_id = 2 LIMIT 5 OFFSET {}",
                    offset
                )
                .as_str(),
            )?;

            let mut manga_history: Vec<MangaHistory> = vec![];

            let iter_mangas = statement.query_map([], |row| {
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
        MangaHistoryType::PlanToRead => todo!(),
    }
}
