use std::marker::PhantomData;

use rusqlite::{Connection, Transaction};

#[derive(Debug, PartialEq, Eq)]
pub struct MigrationTable {
    id: u32,
    name: String,
    version: String,
    applied_at: String,
}

impl MigrationTable {
    fn new(id: u32, name: String, version: String, applied_at: String) -> Self {
        Self {
            id,
            name,
            version,
            applied_at,
        }
    }

    fn get_schema() -> &'static str {
        r"
        CREATE TABLE IF NOT EXISTS migrations(
            id INTEGER PRIMARY KEY, 
            name VARCHAR NOT NULL,
            version VARCHAR NOT NULL,
            applied_at  DATETIME DEFAULT (datetime('now'))
        )"
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Up;

#[derive(Debug, PartialEq, Eq)]
pub struct Down;

#[derive(Debug, PartialEq, Eq)]
pub struct Building;

#[derive(Debug, PartialEq, Eq)]
pub struct Migration<'a, T = Building> {
    version: &'a str,
    queries: &'a [&'a str],
    name: &'a str,
    _phantom_data: PhantomData<T>,
}

impl<'a> Migration<'a, Building> {
    pub fn new() -> Self {
        Self {
            version: "",
            queries: &[],
            name: "",
            _phantom_data: PhantomData,
        }
    }

    pub fn with_version(mut self, version: &'a str) -> Self {
        self.version = version;
        self
    }

    pub fn with_name(mut self, name: &'a str) -> Self {
        self.name = name;
        self
    }

    pub fn with_queries(mut self, queries: &'a [&'a str]) -> Self {
        self.queries = queries;
        self
    }

    pub fn up(self, connection: &mut Connection) -> rusqlite::Result<Option<Migration<'a, Up>>> {
        let transaction = connection.transaction()?;

        self.create_table_migrations_if_not_exists(&transaction)?;

        if !self.should_run_migration(&transaction)? {
            transaction.commit()?;
            return Ok(None);
        }

        transaction.commit()?;
        Ok(Some(Migration {
            version: self.version,
            queries: self.queries,
            name: self.name,
            _phantom_data: PhantomData,
        }))
    }

    fn should_run_migration(&self, transaction: &Transaction) -> rusqlite::Result<bool> {
        let query = "SELECT EXISTS(SELECT id FROM migrations WHERE name = ?1 AND version = ?2) as row_exists";
        let migration_exists: bool = transaction.query_row(query, [self.name, self.version], |row| row.get(0))?;

        Ok(!migration_exists)
    }
}

impl<'a> Migration<'a, Up> {
    fn new_up_migration(version: &'a str, queries: &'a [&'a str], name: &'a str) -> Migration<'a, Up> {
        Migration {
            version,
            queries,
            name,
            _phantom_data: PhantomData,
        }
    }

    pub fn update(self, connection: &mut Connection) -> rusqlite::Result<MigrationTable> {
        let transaction = connection.transaction()?;

        self.run_queries(&transaction)?;

        let migration_saved = self.save_migration(&transaction)?;

        transaction.commit()?;

        Ok(migration_saved)
    }
}

impl<'a, T> Migration<'a, T> {
    fn run_queries(&self, transaction: &Transaction) -> rusqlite::Result<()> {
        for querie in self.queries {
            transaction.execute(querie, [])?;
        }
        Ok(())
    }

    fn create_table_migrations_if_not_exists(&self, transaction: &Transaction) -> rusqlite::Result<()> {
        let migrations_table = MigrationTable::get_schema();

        transaction.execute(migrations_table, [])?;

        Ok(())
    }

    fn save_migration(&self, transaction: &Transaction) -> rusqlite::Result<MigrationTable> {
        let insert_query = "INSERT INTO migrations(name, version) VALUES(?1, ?2) RETURNING id, name, version, applied_at";

        let result: MigrationTable = transaction.query_row(insert_query, [self.name, self.version], |row| {
            let migration_saved = MigrationTable::new(row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?);

            Ok(migration_saved)
        })?;

        Ok(result)
    }
}

impl<'a> Migration<'a, Down> {
    pub fn rollback(self, connection: &mut Connection) -> rusqlite::Result<()> {
        let transaction = connection.transaction()?;

        self.run_queries(&transaction)?;

        transaction.commit()?;

        Ok(())
    }
}

pub fn migrate_version() -> rusqlite::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use fake::faker::name::en::Name;
    use fake::Fake;
    use pretty_assertions::assert_eq;
    use uuid::Uuid;

    use super::*;
    use crate::backend::filter::Languages;

    #[test]
    fn it_creates_migration_table() -> Result<(), Box<dyn Error>> {
        let mut connection = Connection::open_in_memory()?;

        let migration: Migration<Up> = Migration::<Up>::new_up_migration("1.0.1", &[], "some change");

        let transaction = connection.transaction()?;

        migration.create_table_migrations_if_not_exists(&transaction)?;

        let confirmation: String = transaction
            .query_row("SELECT name FROM sqlite_master WHERE type='table' AND name='migrations';", [], |row| {
                let table_name: String = row.get(0)?;
                Ok(table_name)
            })
            .unwrap();

        assert_eq!(confirmation, "migrations");

        Ok(())
    }

    #[test]
    fn it_saves_migration() -> Result<(), Box<dyn Error>> {
        let mut connection = Connection::open_in_memory()?;

        connection.execute(
            "
        CREATE TABLE  migrations(
            id INTEGER PRIMARY KEY, 
            name VARCHAR NOT NULL,
            version VARCHAR NOT NULL,
            applied_at  DATETIME DEFAULT (datetime('now'))
        )
        ",
            [],
        )?;

        let version = "0.4.0";
        let queries = ["ALTER TABLE contacts ADD address VARCHAR NULL"];

        let migration: Migration<Up> = Migration::<Up>::new_up_migration(version, &queries, "some name");

        let transaction = connection.transaction()?;

        let migration_info: MigrationTable = migration.save_migration(&transaction).expect("could not save migration");

        assert_eq!(migration_info.version, version);

        Ok(())
    }

    #[test]
    fn it_runs_queries_provided() -> Result<(), Box<dyn Error>> {
        let mut connection = Connection::open_in_memory()?;
        connection.execute("CREATE TABLE test(id PRIMARY KEY)", [])?;

        let queries = ["ALTER TABLE test ADD name VARCHAR NULL", "ALTER TABLE test ADD date VARCHAR NULL"];

        let migration = Migration::<Up>::new_up_migration("", &queries, "");

        let transaction = connection.transaction()?;

        migration.run_queries(&transaction)?;

        let _confirmation = transaction
            .execute("INSERT INTO test(name, date) VALUES(?1, ?2)", ["val1", "val2"])
            .expect("table was not updated");

        Ok(())
    }

    #[test]
    fn it_runs_migration() -> Result<(), Box<dyn Error>> {
        let mut connection = Connection::open_in_memory()?;

        let table_test_query = r"CREATE TABLE contacts (
                                   contact_id INTEGER PRIMARY KEY,
                                    first_name TEXT  NULL,
                                    last_name TEXT  NULL
                                );";

        connection.execute(table_test_query, [])?;

        let expected_result = MigrationTable {
            id: 1,
            name: "add new column".into(),
            version: "1.0.1".into(),
            applied_at: "20-30-10".into(),
        };

        let queries = ["ALTER TABLE contacts ADD address VARCHAR NULL", "ALTER TABLE contacts ADD email VARCHAR NULL"];

        let migration: Migration<Up> = Migration::new()
            .with_version(&expected_result.version)
            .with_name(&expected_result.name)
            .with_queries(&queries)
            .up(&mut connection)
            .expect("this migration should be run")
            .unwrap();

        let migration = migration.update(&mut connection)?;

        assert_eq!(expected_result.id, migration.id);
        assert_eq!(expected_result.name, migration.name);
        assert_eq!(expected_result.version, migration.version);

        Ok(())
    }

    #[test]
    fn it_runs_migration_add_multiple_columns() -> Result<(), Box<dyn Error>> {
        let mut connection = Connection::open_in_memory()?;

        let table_test_query = "CREATE TABLE contacts (contact_id INTEGER PRIMARY KEY);";

        connection.execute(table_test_query, [])?;
        connection.execute("INSERT INTO contacts(contact_id) VALUES(1)", [])?;

        let queries = [
            "ALTER TABLE contacts ADD first_name VARCHAR NULL",
            "ALTER TABLE contacts ADD last_name VARCHAR NOT NULL DEFAULT 'undefined'",
        ];

        let expected_result = MigrationTable {
            id: 1,
            name: "add datetime column".into(),
            version: "1.0.1".into(),
            applied_at: "20-30-10".into(),
        };

        let migration: Migration<Up> = Migration::new()
            .with_name(&expected_result.name)
            .with_version(&expected_result.version)
            .with_queries(&queries)
            .up(&mut connection)
            .expect("this migration should be run")
            .unwrap();

        let migration_result = migration.update(&mut connection)?;

        connection.execute("INSERT INTO contacts(first_name, last_name) VALUES('john', 'doe')", [])?;

        assert_eq!(migration_result.version, expected_result.version);
        assert_eq!(migration_result.id, expected_result.id);
        assert_eq!(migration_result.name, expected_result.name);

        Ok(())
    }

    #[test]
    fn it_knows_it_must_run_migration() -> Result<(), Box<dyn Error>> {
        let mut connection = Connection::open_in_memory()?;

        connection.execute(MigrationTable::get_schema(), [])?;

        let migration = Migration::new()
            .with_version("1.0.0")
            .with_name("some_migration")
            .with_queries(&["ALTER TABLE chapters ADD is_read VARCHAR NOT NULL DEFAULT 'no'"]);

        let transaction = connection.transaction()?;

        assert!(migration.should_run_migration(&transaction)?);

        Ok(())
    }

    #[test]
    fn it_knows_it_must_not_run_migration() -> Result<(), Box<dyn Error>> {
        let mut connection = Connection::open_in_memory()?;

        let already_existing_migration_version = "1.0.3";
        let already_existing_name = "Update table chapters";

        connection.execute(MigrationTable::get_schema(), [])?;

        connection.execute("INSERT INTO migrations(name, version) VALUES(?1, ?2)", [
            already_existing_name,
            already_existing_migration_version,
        ])?;

        let migration = Migration::new()
            .with_version(already_existing_migration_version)
            .with_name(already_existing_name)
            .with_queries(&["ALTER TABLE chapters ADD is_read VARCHAR NOT NULL DEFAULT 'no'"]);

        let transaction = connection.transaction()?;

        assert!(!migration.should_run_migration(&transaction)?);

        Ok(())
    }

    #[test]
    fn migrate_version_0_5_0() -> Result<(), Box<dyn Error>> {
        let mut conn = Connection::open_in_memory()?;

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
        )?;

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
        )?;

        let manga_id = Uuid::new_v4().to_string();
        let chapter_id = Uuid::new_v4().to_string();

        conn.execute("INSERT INTO mangas(id, title) VALUES(?1, ?2)", [manga_id.clone(), Name().fake()])?;
        conn.execute("INSERT INTO chapters(id, title, manga_id) VALUES(?1, ?2, ?3)", [
            chapter_id,
            Name().fake(),
            manga_id.clone(),
        ])?;

        migrate_version().expect("the update did not ran successfully");

        conn.execute("INSERT INTO chapters(id, title, manga_id, translated_language, is_bookmarked) VALUES(?1, ?2, ?3, ?4, ?5)", [
            Uuid::new_v4().to_string(),
            "some_title".to_string(),
            manga_id,
            Languages::default().as_iso_code().to_string(),
            true.to_string(),
        ])
        .expect("migration did not update table chapters");

        Ok(())
    }
}
