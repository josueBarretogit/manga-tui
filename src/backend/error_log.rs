use std::error::Error;
use std::fs::{File, OpenOptions, create_dir_all};
use std::io::Write;
use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};

use chrono::offset;
use manga_tui::exists;

use super::AppDirectories;

pub enum ErrorType<'a> {
    Panic(&'a PanicHookInfo<'a>),
    Error(Box<dyn Error>),
    String(&'a str),
}

impl<'a> From<Box<dyn Error>> for ErrorType<'a> {
    fn from(value: Box<dyn Error>) -> Self {
        Self::Error(value)
    }
}

impl<'a> From<String> for ErrorType<'a> {
    fn from(value: String) -> Self {
        Self::Error(value.into())
    }
}

impl<'a> From<&'a str> for ErrorType<'a> {
    fn from(value: &'a str) -> Self {
        Self::String(value)
    }
}

fn get_error_logs_path() -> PathBuf {
    let path = AppDirectories::ErrorLogs.get_base_directory();

    if !exists!(&path) {
        create_dir_all(&path).ok();
    }

    AppDirectories::ErrorLogs.get_full_path()
}

pub fn write_to_error_log(e: ErrorType<'_>) {
    let error_file_name = get_error_logs_path();

    let now = offset::Local::now();

    let error_format = match e {
        ErrorType::Panic(panic_info) => match panic_info.location() {
            Some(location) => format!("{now} | {panic_info} | {location} \n \n"),
            None => format!("{now} | {panic_info} \n \n"),
        },
        ErrorType::Error(boxed_err) => format!("{now} | {boxed_err} \n \n"),
        ErrorType::String(str) => format!("{now} | {str} \n \n"),
    };

    // `create(true)` + `append(true)` avoids a race when two tasks try to create the file at the
    // same time, writing the error log must never panic
    if let Ok(mut error_logs) = OpenOptions::new().create(true).append(true).open(error_file_name) {
        let _ = error_logs.write_all(error_format.as_bytes());
    }
}

pub fn create_error_logs_files(base_directory: &Path) -> std::io::Result<()> {
    let error_logs_path = base_directory.join(AppDirectories::ErrorLogs.get_path());
    if !exists!(&error_logs_path) {
        File::create(error_logs_path)?;
    }
    Ok(())
}
