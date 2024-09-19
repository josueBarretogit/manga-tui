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

            if !exists!(&dir.join(AppDirectories::ErrorLogs.to_string()).join(ERROR_LOGS_FILE)) {
                File::create(dir.join(AppDirectories::ErrorLogs.to_string()).join(ERROR_LOGS_FILE))?;
            }

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

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_config_file() -> Result<(), Box<dyn Error>> {
        let data_dir = build_data_dir().expect("Could not build data directory");

        let config_template = MangaTuiConfig::get_file_contents();

        toml::from_str::<MangaTuiConfig>(config_template).expect("error when deserializing config template");

        let contents = MangaTuiConfig::read_raw_config(&data_dir).expect("error when reading raw config file");

        toml::from_str::<MangaTuiConfig>(&contents).expect("error when deserializing config file");

        assert_eq!(contents, MangaTuiConfig::get_file_contents());

        Ok(())
    }

    #[test]
    fn data_directory_is_built() -> Result<(), Box<dyn Error>> {
        build_data_dir().expect("Could not build data directory");

        let mut amount_directories = 0;

        let directory_built = fs::read_dir(APP_DATA_DIR.as_ref().unwrap())?;

        for dir in directory_built {
            let dir = dir?;

            let directory_name = dir.file_name();

            assert!(AppDirectories::iter().any(|app_dir| app_dir.to_string() == directory_name.to_string_lossy()));

            amount_directories += 1;
        }

        assert_eq!(4, amount_directories);

        Ok(())
    }
}
