#[macro_export]
macro_rules! exists {
    ($path:expr) => {
        Path::new(&$path).try_exists().is_ok_and(|is_true| is_true)
    };
}
