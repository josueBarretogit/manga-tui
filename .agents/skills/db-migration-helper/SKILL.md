---
name: db-migration-helper
description: >-
  Use this skill when modifying the SQLite database schema, adding new tables or
  columns, or writing migrations in manga-tui. Ensures proper versioning in
  migration.rs, updates database.rs, syncs database_scheme.md, and writes unit tests.
---

# Database Migration Helper Skill

This skill guides you through safely modifying the SQLite schema and creating incremental migrations in `manga-tui`.

---

## Migration Architecture

In `manga-tui`:
- Database operations and queries live in `src/backend/database.rs`.
- Migration engine and versions live in `src/backend/migration.rs`.
- Database documentation lives in `src/backend/database_scheme.md`.
- Migrations run automatically at startup via `update_database_with_migrations` called from `src/main.rs`.

---

## 1. Migration Checklist

Whenever you modify table structures:

1. **Define the Migration in `src/backend/migration.rs`**:
   Write a new `migrate_version_X_Y_Z` function with the target SemVer version string:
   ```rust
   /// Add column `new_col` to table `chapters`
   fn migrate_version_0_7_0(connection: &mut Connection, logger: &impl ILogger) -> rusqlite::Result<Option<MigrationTable>> {
       let queries = [Query::AlterTable {
           table_name: "chapters",
           command: AlterTableCommand::Add {
               column: "new_col",
               data_type: "TEXT NULL",
           },
       }];

       let migration = Migration::new(&queries)
           .with_name("Add column new_col to table chapters")
           .with_version("0.7.0")
           .up(connection)?;

       let migration_result = match migration {
           Some(available_migration) => {
               logger.inform("Updating database");
               let migration_result = available_migration.update(connection)?;
               logger.inform("Database updated");
               Some(migration_result)
           },
           None => None,
       };

       Ok(migration_result)
   }
   ```

2. **Register in `update_database_with_migrations`**:
   Append the new migration call to the list inside `update_database_with_migrations`:
   ```rust
   pub fn update_database_with_migrations(
       connection: &mut Connection,
       logger: &impl ILogger,
   ) -> rusqlite::Result<Vec<Option<MigrationTable>>> {
       let migrations: Vec<Option<MigrationTable>> = vec![
           migrate_version_0_4_0(connection, logger)?,
           migrate_version_0_6_0(connection, logger)?,
           migrate_version_0_7_0(connection, logger)?, // <- Added here
       ];

       Ok(migrations)
   }
   ```

3. **Update Schema Initialization in `src/backend/database.rs`**:
   Fresh installs don't start with legacy schemas; they run the complete `CREATE TABLE IF NOT EXISTS` statements. Ensure the new column is present in the table creation SQL in `database.rs`.

4. **Update Documentation in `src/backend/database_scheme.md`**:
   Keep table definitions and field comments synchronized.

5. **Write Unit Tests in `src/backend/migration.rs`**:
   Add a test verifying:
   - Migration runs on a legacy in-memory database.
   - Column exists after migration (`PRAGMA table_info`).
   - Running the migration a second time is a safe no-op.

---

## 2. Verification

Run the migration test suite:
```powershell
cargo test backend::migration -- --nocapture
```
And check database unit tests:
```powershell
cargo test backend::database -- --nocapture
```
