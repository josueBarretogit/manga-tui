use std::error::Error;
use std::fs::{File, OpenOptions, create_dir_all};
use std::io::Write;
use std::panic::PanicInfo;
use std::path::{Path, PathBuf};

use chrono::offset;
use manga_tui::exists;

use super::AppDirectories;

pub enum ErrorType<'a> {
    Panic(&'a PanicInfo<'a>),
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
        ErrorType::Panic(panic_info) => format!("{} | {} | {} \n \n", now, panic_info, panic_info.location().unwrap()),
        ErrorType::Error(boxed_err) => format!("{now} | {boxed_err} \n \n"),
        ErrorType::String(str) => format!("{now} | {str} \n \n"),
    };

    let error_format_bytes = error_format.as_bytes();

    if !exists!(&error_file_name) {
        let mut error_logs = File::create_new(error_file_name).unwrap();

        error_logs.write_all(error_format_bytes).unwrap();
    } else {
        let mut error_logs = OpenOptions::new().append(true).open(error_file_name).unwrap();

        error_logs.write_all(error_format_bytes).unwrap();
    }
}

pub fn create_error_logs_files(base_directory: &Path) -> std::io::Result<()> {
    let error_logs_path = base_directory.join(AppDirectories::ErrorLogs.get_path());
    if !exists!(&error_logs_path) {
        File::create(error_logs_path)?;
    }
    Ok(())
}
