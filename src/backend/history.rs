// save what mangas the user is reading and which chapters where read
// need a file to store that data,
// need to update it
//

use rusqlite::Connection;

// Create sqlite file if it does not exist and its tables
pub fn create_history() {}

pub struct MangaReadingHistorySave<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub chapter_read_id: &'a str,
}

pub fn save_history(history: MangaReadingHistorySave<'_>) {
    let db_conn = Connection::open("./db_test.db").unwrap();

    db_conn
        .execute(
            "

CREATE TABLE IF NOT EXISTS mangas (
id TEXT PRIMARY KEY,
title VARCHAR NOT NULL,
)

    ",
            (),
        )
        .unwrap();
}

pub struct MangaReadingHistoryRetrieve<'a> {
    pub chapters_read: Vec<&'a str>,
}

pub fn get_manga_history(id: &str) -> MangaReadingHistoryRetrieve<'_> {
    let db_connection = Connection::open("./db_test.db").unwrap();

    let mut result = db_connection.prepare("SELECT id from mangas ").unwrap();

    MangaReadingHistoryRetrieve {
        chapters_read: vec![],
    }
}
