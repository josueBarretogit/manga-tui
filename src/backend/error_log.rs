use std::error::Error;
use std::fs::{create_dir_all, File, OpenOptions};
use std::io::Write;
use std::panic::PanicInfo;
use std::path::{Path, PathBuf};

use chrono::offset;
use color_eyre::config::HookBuilder;
use manga_tui::exists;

use super::tui::restore;
use super::AppDirectories;

pub enum ErrorType<'a> {
    Panic(&'a PanicInfo<'a>),
    Error(Box<dyn Error>),
    String(&'a str),
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
        ErrorType::Error(boxed_err) => format!("{} | {} \n \n", now, boxed_err),
        ErrorType::String(str) => format!("{} | {} \n \n", now, str),
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

pub fn init_error_hooks() -> color_eyre::Result<()> {
    let (panic, error) = HookBuilder::default().into_hooks();
    let panic = panic.into_panic_hook();
    let error = error.into_eyre_hook();

    color_eyre::eyre::set_hook(Box::new(move |e| {
        let _ = restore();
        error(e)
    }))?;

    std::panic::set_hook(Box::new(move |info| {
        let _ = restore();
        write_to_error_log(ErrorType::Panic(info));
        panic(info);
    }));

    Ok(())
}
