use std::fs::{create_dir, create_dir_all};
use std::path::{Path, PathBuf};

use manga_tui::exists;
use once_cell::sync::Lazy;
use strum::{Display, EnumIter, IntoEnumIterator};

use self::error_log::create_error_logs_files;
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

static ERROR_LOGS_FILE: &str = "manga-tui-error-logs.txt";

static DATABASE_FILE: &str = "manga-tui-history.db";

static CONFIG_FILE: &str = "manga-tui-config.toml";

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

    pub fn get_path(self) -> PathBuf {
        let base_directory = self.to_string();
        match self {
            Self::Config => PathBuf::from(base_directory).join(CONFIG_FILE),
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

pub fn build_data_dir() -> Result<PathBuf, std::io::Error> {
    let data_dir = APP_DATA_DIR.as_ref();
    match data_dir {
        Some(dir) => {
            if !exists!(dir) {
                create_dir_all(dir)?;
            }

            AppDirectories::build_if_not_exists(dir)?;

            create_error_logs_files(dir)?;

            MangaTuiConfig::write_if_not_exists(dir)?;

            let config_contents = MangaTuiConfig::read_raw_config(dir)?;

            let config_contents: MangaTuiConfig = toml::from_str(&config_contents).unwrap_or_default();

            CONFIG.get_or_init(|| config_contents);

            Ok(dir.to_path_buf())
        },
        None => Err(std::io::Error::other("data dir could not be found")),
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

    #[test]
    #[ignore]
    fn test_config_file() -> Result<(), Box<dyn Error>> {
        sleep(Duration::from_millis(100));
        let data_dir = build_data_dir().expect("Could not build data directory");

        let config_template = MangaTuiConfig::get_config_template();

        toml::from_str::<MangaTuiConfig>(config_template).expect("error when deserializing config template");

        let contents = MangaTuiConfig::read_raw_config(&data_dir).expect("error when reading raw config file");

        toml::from_str::<MangaTuiConfig>(&contents).expect("error when deserializing config file");

        assert_eq!(contents, MangaTuiConfig::get_config_template());

        Ok(())
    }

    #[test]
    #[ignore]
    fn data_directory_is_built() -> Result<(), Box<dyn Error>> {
        sleep(Duration::from_millis(1000));
        dbg!(build_data_dir().expect("Could not build data directory"));

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

        assert_eq!(4, amount_directories);

        let error_logs_path = dbg!(AppDirectories::ErrorLogs.get_full_path());

        fs::File::open(error_logs_path).expect("Could not open error logs file");

        Ok(())
    }
}
