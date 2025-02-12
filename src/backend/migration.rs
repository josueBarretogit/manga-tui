use std::fmt::{Debug, Display};
use std::marker::PhantomData;

use rusqlite::{Connection, Result, Transaction};

use crate::logger::ILogger;

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

pub enum AlterTableCommand<'a> {
    Add { column: &'a str, data_type: &'a str },
}

pub enum Query<'a> {
    AlterTable {
        table_name: &'a str,
        command: AlterTableCommand<'a>,
    },
}

impl<'a> Display for Query<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlterTable {
                table_name,
                command,
            } => match command {
                AlterTableCommand::Add {
                    column: column_to_add,
                    data_type,
                } => write!(f, "ALTER TABLE {} ADD {} {}", table_name, column_to_add, data_type),
            },
        }
    }
}

impl<'a> Debug for Query<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

#[derive(Debug)]
pub struct Up;

#[derive(Debug)]
pub struct Down;

#[derive(Debug)]
pub struct Building;

#[derive(Debug)]
pub struct Migration<'a, T = Building> {
    version: &'a str,
    queries: &'a [Query<'a>],
    name: &'a str,
    _phantom_data: PhantomData<T>,
}

impl<'a> Migration<'a, Building> {
    pub fn new(queries: &'a [Query<'a>]) -> Self {
        Self {
            version: "",
            queries,
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

    pub fn up(self, connection: &mut Connection) -> Result<Option<Migration<'a, Up>>> {
        let transaction = connection.transaction()?;

        self.create_table_migrations_if_not_exists(&transaction)?;

        if !self.should_run_migration(&transaction)? {
            transaction.commit()?;
            return Ok(None);
        }

        transaction.commit()?;

        let migration: Migration<Up> = Migration::new_up_migration(self.version, self.queries, self.name);

        Ok(Some(migration))
    }
}

impl<'a> Migration<'a, Up> {
    fn new_up_migration(version: &'a str, queries: &'a [Query<'a>], name: &'a str) -> Migration<'a, Up> {
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
        for query in self.queries {
            if self.can_run_query(query, transaction)? {
                transaction.execute(&query.to_string(), [])?;
            }
        }
        Ok(())
    }

    fn can_run_query(&self, query: &'a Query<'a>, transaction: &Transaction) -> rusqlite::Result<bool> {
        let can_run_query = match query {
            Query::AlterTable {
                table_name,
                command,
            } => match command {
                AlterTableCommand::Add { column, .. } => !self.column_exists(table_name, column, transaction)?,
            },
        };

        Ok(can_run_query)
    }

    fn column_exists(&self, table_name: &str, column_name: &str, transaction: &Transaction) -> rusqlite::Result<bool> {
        let query = format!("PRAGMA table_info({table_name})");

        let mut query = transaction.prepare(&query)?;

        let rows = query.query_map([], |row| row.get::<_, String>(1))?;

        for column in rows {
            let column = column?;
            if column == column_name {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn should_run_migration(&self, transaction: &Transaction) -> rusqlite::Result<bool> {
        let query = "SELECT EXISTS(SELECT id FROM migrations WHERE name = ?1 AND version = ?2) as row_exists";
        let migration_exists: bool = transaction.query_row(query, [self.name, self.version], |row| row.get(0))?;

        Ok(!migration_exists)
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

/// migrate to version 0.4.0
pub fn migrate_version(connection: &mut Connection, logger: &impl ILogger) -> rusqlite::Result<Option<MigrationTable>> {
    let queries = [
        Query::AlterTable {
            table_name: "chapters",
            command: AlterTableCommand::Add {
                column: "is_bookmarked",
                data_type: "BOOLEAN NOT NULL DEFAULT false",
            },
        },
        Query::AlterTable {
            table_name: "chapters",
            command: AlterTableCommand::Add {
                column: "number_page_bookmarked",
                data_type: "INT NULL",
            },
        },
        Query::AlterTable {
            table_name: "chapters",
            command: AlterTableCommand::Add {
                column: "translated_language",
                data_type: "TEXT NULL",
            },
        },
    ];

    let migration = Migration::new(&queries)
        .with_name("Add columns is_bookmarked, number_page_bookmarked and translated_language to table chapters")
        .with_version("0.4.0")
        .up(connection)?;

    let migration_result = match migration {
        Some(available_migration) => {
            logger.inform("Updating database");
            let migration_result = available_migration.update(connection)?;
            logger.inform("Database schema is up to date");
            Some(migration_result)
        },
        None => None,
    };

    Ok(migration_result)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use fake::faker::name::en::Name;
    use fake::Fake;
    use pretty_assertions::assert_eq;
    use uuid::Uuid;

    use super::*;
    use crate::backend::manga_provider::Languages;
    use crate::logger::DefaultLogger;

    #[test]
    fn it_makes_alter_table_add_query() {
        let query = Query::AlterTable {
            table_name: "chapters",
            command: AlterTableCommand::Add {
                column: "is_bookmarked",
                data_type: "BOOLEAN NOT NULL DEFAULT false",
            },
        };

        assert_eq!(query.to_string(), "ALTER TABLE chapters ADD is_bookmarked BOOLEAN NOT NULL DEFAULT false");

        let query = Query::AlterTable {
            table_name: "chapters",
            command: AlterTableCommand::Add {
                column: "number_page_bookmarked",
                data_type: "INT NULL",
            },
        };

        assert_eq!(query.to_string(), "ALTER TABLE chapters ADD number_page_bookmarked INT NULL");
    }

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

        let queries = [Query::AlterTable {
            table_name: "contacts",
            command: AlterTableCommand::Add {
                column: "address",
                data_type: "VARCHAR NULL",
            },
        }];

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

        let queries = [
            Query::AlterTable {
                table_name: "test",
                command: AlterTableCommand::Add {
                    column: "name",
                    data_type: "VARCHAR NULL",
                },
            },
            Query::AlterTable {
                table_name: "test",
                command: AlterTableCommand::Add {
                    column: "date",
                    data_type: "VARCHAR NULL",
                },
            },
        ];

        let migration = Migration::<Up>::new_up_migration("", &queries, "");

        let transaction = connection.transaction()?;

        migration.run_queries(&transaction)?;

        let _confirmation = transaction
            .execute("INSERT INTO test(name, date) VALUES(?1, ?2)", ["val1", "val2"])
            .expect("table was not updated");

        Ok(())
    }

    #[test]
    fn it_checks_for_column_already_existing() -> Result<(), Box<dyn Error>> {
        let mut connection = Connection::open_in_memory()?;
        connection
            .execute("CREATE TABLE mangas(id PRIMARY KEY, title VARCHAR NULL, is_read BOOLEAN NOT NULL DEFAULT false)", [])?;

        let transaction = connection.transaction()?;

        let migration: Migration<Up> = Migration::new_up_migration("0.1.0", &[], "add column title");

        assert!(migration.column_exists("mangas", "title", &transaction)?);
        assert!(!migration.column_exists("mangas", "description", &transaction)?);
        assert!(!migration.column_exists("mangas", "chapters", &transaction)?);
        assert!(migration.column_exists("mangas", "is_read", &transaction)?);

        Ok(())
    }

    #[test]
    fn it_knows_not_to_run_alter_table_query() -> Result<(), Box<dyn Error>> {
        let mut connection = Connection::open_in_memory()?;
        connection.execute("CREATE TABLE mangas(id PRIMARY KEY, title VARCHAR NULL)", [])?;

        let queries = [Query::AlterTable {
            table_name: "mangas",
            command: AlterTableCommand::Add {
                column: "title",
                data_type: "VARCHAR NULL",
            },
        }];

        let should_run_this_query = Query::AlterTable {
            table_name: "mangas",
            command: AlterTableCommand::Add {
                column: "description",
                data_type: "VARCHAR NULL",
            },
        };

        let transaction = connection.transaction()?;

        let migration: Migration<Up> = Migration::new_up_migration("0.1.0", &queries, "add column title");

        assert!(!migration.can_run_query(&queries[0], &transaction)?);
        assert!(migration.can_run_query(&should_run_this_query, &transaction)?);

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

        let queries = [
            Query::AlterTable {
                table_name: "contacts",
                command: AlterTableCommand::Add {
                    column: "address",
                    data_type: "VARCHAR NULL",
                },
            },
            Query::AlterTable {
                table_name: "contacts",
                command: AlterTableCommand::Add {
                    column: "email",
                    data_type: "VARCHAR NULL",
                },
            },
        ];

        let migration: Migration<Up> = Migration::new(&queries)
            .with_version(&expected_result.version)
            .with_name(&expected_result.name)
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
            Query::AlterTable {
                table_name: "contacts",
                command: AlterTableCommand::Add {
                    column: "first_name",
                    data_type: "VARCHAR NULL",
                },
            },
            Query::AlterTable {
                table_name: "contacts",
                command: AlterTableCommand::Add {
                    column: "last_name",
                    data_type: "VARCHAR NOT NULL DEFAULT 'undefined'",
                },
            },
        ];

        let expected_result = MigrationTable {
            id: 1,
            name: "add datetime column".into(),
            version: "1.0.1".into(),
            applied_at: "20-30-10".into(),
        };

        let migration: Migration<Up> = Migration::new(&queries)
            .with_name(&expected_result.name)
            .with_version(&expected_result.version)
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

        let migration = Migration::new(&[Query::AlterTable {
            table_name: "chapters",
            command: AlterTableCommand::Add {
                column: "is_read",
                data_type: "VARCHAR NOT NULL DEFAULT 'no'",
            },
        }])
        .with_version("1.0.0")
        .with_name("some_migration");

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

        let migration = Migration::new(&[Query::AlterTable {
            table_name: "chapters",
            command: AlterTableCommand::Add {
                column: "is_read",
                data_type: "VARCHAR NOT NULL DEFAULT 'no'",
            },
        }])
        .with_version(already_existing_migration_version)
        .with_name(already_existing_name);

        let transaction = connection.transaction()?;

        assert!(!migration.should_run_migration(&transaction)?);

        Ok(())
    }

    #[test]
    fn migration_does_not_add_columns_that_already_exist() -> Result<(), Box<dyn Error>> {
        let mut connection = Connection::open_in_memory()?;
        connection.execute("CREATE TABLE clients(id PRIMARY KEY, name VARCHAR NULL, address VARCHAR NULL)", [])?;
        let table_name = "clients";

        let queries = [
            Query::AlterTable {
                table_name,
                command: AlterTableCommand::Add {
                    column: "name", // already exists
                    data_type: "VARCHAR NULL",
                },
            },
            Query::AlterTable {
                table_name,
                command: AlterTableCommand::Add {
                    column: "last_name", // doesnt exist, should be added
                    data_type: "VARCHAR NULL",
                },
            },
            Query::AlterTable {
                table_name,
                command: AlterTableCommand::Add {
                    column: "address",
                    data_type: "VARCHAR NULL",
                },
            },
            Query::AlterTable {
                table_name,
                command: AlterTableCommand::Add {
                    column: "email",
                    data_type: "VARCHAR NULL",
                },
            },
        ];

        let migration = Migration::new(&queries)
            .with_name("Add column name and last_name")
            .with_version("0.0.7")
            .up(&mut connection)
            .expect("must run this Up migration")
            .unwrap();

        migration.update(&mut connection)?;

        connection
            .execute("INSERT INTO clients(name, last_name, email, address) VALUES(?1, ?2, ?3, ?4)", [
                "some_name",
                "some_last_name",
                "some_email",
                "some_address",
            ])
            .expect("migration did not update the schema as expectd");

        Ok(())
    }

    #[test]
    fn migrate_version_0_4_0() -> Result<(), Box<dyn Error>> {
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

        let migration_result = migrate_version(&mut conn, &DefaultLogger)
            .expect("the update did not ran successfully")
            .unwrap();

        assert_eq!(migration_result.version, "0.4.0");

        conn.execute("INSERT INTO chapters(id, title, manga_id, translated_language, is_bookmarked, number_page_bookmarked) VALUES(?1, ?2, ?3, ?4, ?5, ?6)", [
            Uuid::new_v4().to_string(),
            "some_title".to_string(),
            manga_id,
            Languages::default().as_iso_code().to_string(),
            true.to_string(),
            "2".to_string(),
        ])
        .expect("migration did not update table chapters");

        let second_time = migrate_version(&mut conn, &DefaultLogger).expect("should not run migration twice");

        assert!(second_time.is_none());

        Ok(())
    }
}
