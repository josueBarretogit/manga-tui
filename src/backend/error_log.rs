use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::panic::PanicInfo;
use std::path::Path;

use chrono::offset;
use color_eyre::config::HookBuilder;
use manga_tui::exists;

use super::tui::restore;
use super::{AppDirectories, APP_DATA_DIR};

pub static ERROR_LOGS_FILE: &str = "manga-tui-error-logs.txt";

pub enum ErrorType<'a> {
    FromPanic(&'a PanicInfo<'a>),
    FromError(Box<dyn Error>),
}

pub fn write_to_error_log(e: ErrorType<'_>) {
    let error_file_name = APP_DATA_DIR
        .as_ref()
        .unwrap()
        .join(AppDirectories::ErrorLogs.to_string())
        .join(ERROR_LOGS_FILE);

    let now = offset::Local::now();

    let error_format = match e {
        ErrorType::FromPanic(panic_info) => format!("{} | {} | {} \n \n", now, panic_info, panic_info.location().unwrap()),
        ErrorType::FromError(boxed_err) => format!("{} | {} \n \n", now, boxed_err),
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
        write_to_error_log(ErrorType::FromPanic(info));
        panic(info);
    }));

    Ok(())
}
