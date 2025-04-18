use std::fs::{create_dir, create_dir_all};
use std::path::{Path, PathBuf};

use manga_tui::exists;
use once_cell::sync::Lazy;
use strum::{Display, EnumIter, IntoEnumIterator};

use self::error_log::create_error_logs_files;
use crate::config::{MangaTuiConfig, build_config_file};
use crate::logger::ILogger;

pub mod cache;
pub mod database;
pub mod error_log;
pub mod html_parser;
pub mod manga_downloader;
pub mod manga_provider;
pub mod migration;
pub mod release_notifier;
pub mod secrets;
pub mod tracker;
pub mod tui;

#[derive(Display, EnumIter)]
pub enum AppDirectories {
    #[strum(to_string = "mangaDownloads")]
    MangaDownloads,
    #[strum(to_string = "errorLogs")]
    ErrorLogs,
    #[strum(to_string = "history")]
    History,
}

static ERROR_LOGS_FILE: &str = "manga-tui-error-logs.txt";

static DATABASE_FILE: &str = "manga-tui-history.db";

impl AppDirectories {
    pub fn get_full_path(self) -> PathBuf {
        Self::get_app_directory().join(self.get_path())
    }

    pub fn build_if_not_exists(app_directory: &Path) -> Result<(), std::io::Error> {
        for dir in AppDirectories::iter() {
            let directory_path = app_directory.join(dir.to_string());
            if !exists!(&directory_path) {
                create_dir(directory_path)?;
            }
        }
        Ok(())
    }

    pub fn get_app_directory() -> &'static Path {
        APP_DATA_DIR.as_ref().unwrap()
    }

    pub fn get_base_directory(self) -> PathBuf {
        Self::get_app_directory().join(self.to_string())
    }

    pub fn get_path(self) -> PathBuf {
        let base_directory = self.to_string();
        match self {
            Self::History => PathBuf::from(base_directory).join(DATABASE_FILE),
            Self::ErrorLogs => PathBuf::from(base_directory).join(ERROR_LOGS_FILE),
            Self::MangaDownloads => PathBuf::from(base_directory),
        }
    }
}

#[cfg(not(test))]
pub static APP_DATA_DIR: Lazy<Option<PathBuf>> = Lazy::new(|| {
    directories::ProjectDirs::from("", "", "manga-tui").map(|dirs| match std::env::var("MANGA_TUI_DATA_DIR").ok() {
        Some(data_dir) => PathBuf::from(data_dir),
        None => dirs.data_dir().to_path_buf(),
    })
});

#[cfg(test)]
pub static APP_DATA_DIR: Lazy<Option<PathBuf>> = Lazy::new(|| Some(PathBuf::from("./test_results/data-directory")));

pub fn build_data_dir(logger: &impl ILogger) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let data_dir = APP_DATA_DIR.as_ref();
    match data_dir {
        Some(dir) => {
            if !exists!(dir) {
                create_dir_all(dir)?;
                logger.inform(format!("Creating directory: {}", dir.display()));
            }

            AppDirectories::build_if_not_exists(dir)?;

            create_error_logs_files(dir)?;

            build_config_file()?;

            Ok(dir.to_path_buf())
        },
        None => Err("data dir could not be found".into()),
    }
}

#[cfg(test)]
mod test {
    use std::error::Error;
    use std::fs;
    use std::thread::sleep;
    use std::time::Duration;

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::logger::DefaultLogger;

    #[test]
    #[ignore]
    fn data_directory_is_built() -> Result<(), Box<dyn Error>> {
        sleep(Duration::from_millis(1000));
        dbg!(build_data_dir(&DefaultLogger).expect("Could not build data directory"));

        let mut amount_directories = 0;

        let directory_built = fs::read_dir(AppDirectories::get_app_directory())?;

        for dir in directory_built {
            let dir = dir?;

            let directory_name = dir.file_name();

            let directory_was_created =
                AppDirectories::iter().any(|app_dir| app_dir.to_string() == directory_name.to_string_lossy());

            assert!(directory_was_created);

            amount_directories += 1;
        }

        assert_eq!(3, amount_directories);

        let error_logs_path = dbg!(AppDirectories::ErrorLogs.get_full_path());

        fs::File::open(error_logs_path).expect("Could not open error logs file");

        Ok(())
    }
}
