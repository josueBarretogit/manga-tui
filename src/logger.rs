use std::error::Error;

use log::{error, info, warn};

/// Abstraction for printing to the console messages
pub trait ILogger {
    fn inform(&self, message: impl AsRef<str>) {
        println!("{}", message.as_ref());
    }

    fn error(&self, error: Box<dyn Error>) {
        println!("ERROR | {error}")
    }

    fn warn(&self, warning: impl AsRef<str>) {
        println!("WARN | {}", warning.as_ref())
    }
}

pub struct DefaultLogger;

pub struct Logger;

impl ILogger for DefaultLogger {}

impl ILogger for Logger {
    fn inform(&self, message: impl AsRef<str>) {
        info!("{}", message.as_ref());
    }

    fn warn(&self, warning: impl AsRef<str>) {
        warn!("{}", warning.as_ref());
    }

    fn error(&self, error: Box<dyn Error>) {
        error!("{error}");
    }
}
