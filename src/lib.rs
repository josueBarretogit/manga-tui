/// Shortcut for: Path::new($path).try_exists().is_ok_and(|is_true| is_true)
#[macro_export]
macro_rules! exists {
    ($path:expr) => {
        Path::new($path).try_exists().is_ok_and(|is_true| is_true)
    };
}

#[macro_export]
macro_rules! build_check_exists_function {
    ($func_name:ident, $target_table:expr) => {
        fn $func_name(id: &str, conn: &Connection) -> rusqlite::Result<bool> {
            let exists: bool = conn.query_row(
                format!(
                    "SELECT EXISTS(SELECT id FROM {} WHERE id = ?1) as row_exists",
                    $target_table
                ).as_str(),
                rusqlite::params![id],
                |row| row.get(0),
            )?;
            Ok(exists)
        }
    };
}
