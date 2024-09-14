use std::fs::{create_dir, create_dir_all, File};
use std::path::{Path, PathBuf};

use manga_tui::exists;
use once_cell::sync::Lazy;
use strum::{Display, EnumIter, IntoEnumIterator};

use self::error_log::ERROR_LOGS_FILE;
use crate::config::{MangaTuiConfig, CONFIG};

pub mod api_responses;
pub mod database;
pub mod download;
pub mod error_log;
pub mod fetch;
pub mod filter;
pub mod tui;

#[derive(Display, EnumIter)]
pub enum AppDirectories {
    #[strum(to_string = "mangaDownloads")]
    MangaDownloads,
    #[strum(to_string = "errorLogs")]
    ErrorLogs,
    #[strum(to_string = "history")]
    History,
    #[strum(to_string = "config")]
    Config,
}

impl AppDirectories {
    pub fn into_path_buf(self) -> PathBuf {
        let base_directory = APP_DATA_DIR.as_ref();
        PathBuf::from(&base_directory.unwrap().join(self.to_string()))
    }

    pub fn build_if_not_exists(base_directory: &Path) -> Result<(), std::io::Error> {
        for dir in AppDirectories::iter() {
            if !exists!(&base_directory.join(dir.to_string())) {
                create_dir(base_directory.join(dir.to_string()))?;
            }
        }
        Ok(())
    }

    pub fn get_base_directory() -> &'static Path {
        APP_DATA_DIR.as_ref().unwrap()
    }
}

pub static APP_DATA_DIR: Lazy<Option<PathBuf>> = Lazy::new(|| {
    directories::ProjectDirs::from("", "", "manga-tui").map(|dirs| match std::env::var("MANGA_TUI_DATA_DIR").ok() {
        Some(data_dir) => PathBuf::from(data_dir),
        None => dirs.data_dir().to_path_buf(),
    })
});

pub fn build_data_dir() -> Result<(), std::io::Error> {
    let data_dir = APP_DATA_DIR.as_ref();
    match data_dir {
        Some(dir) => {
            if !exists!(dir) {
                create_dir_all(dir)?;
            }
            AppDirectories::build_if_not_exists(dir)?;

            if !exists!(&dir.join(AppDirectories::ErrorLogs.to_string()).join(ERROR_LOGS_FILE)) {
                File::create(dir.join(AppDirectories::ErrorLogs.to_string()).join(ERROR_LOGS_FILE))?;
            }

            MangaTuiConfig::write_config(dir)?;

            let config_contents = MangaTuiConfig::read_config(dir)?;

            let config_contents: MangaTuiConfig = toml::from_str(&config_contents).unwrap_or_default();

            CONFIG.set(config_contents).unwrap();

            Ok(())
        },
        None => Err(std::io::Error::other("data dir could not be found")),
    }
}
