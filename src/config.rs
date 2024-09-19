use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use manga_tui::exists;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter};

use crate::backend::AppDirectories;

#[derive(Default, Debug, Serialize, Deserialize, Display, EnumIter, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum DownloadType {
    #[default]
    Cbz,
    Raw,
    Epub,
}

#[derive(Default, Debug, Serialize, Deserialize, Display, EnumIter, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum ImageQuality {
    #[default]
    Low,
    High,
}

impl ImageQuality {
    pub fn as_param(self) -> &'static str {
        match self {
            Self::Low => "data-saver",
            Self::High => "data",
        }
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct MangaTuiConfig {
    pub download_type: DownloadType,
    pub image_quality: ImageQuality,
}

pub static CONFIG_FILE: &str = "manga-tui-config.toml";

pub static CONFIG: OnceCell<MangaTuiConfig> = OnceCell::new();

impl MangaTuiConfig {
    pub fn get() -> &'static Self {
        CONFIG.get().expect("Could not get download type")
    }

    pub fn read_raw_config(base_directory: &Path) -> Result<String, std::io::Error> {
        let config_file = base_directory.join(Self::get_config_file_path());

        let mut config_file = File::open(config_file)?;

        let mut contents = String::new();
        config_file.read_to_string(&mut contents)?;

        Ok(contents)
    }

    pub fn get_config_file_path() -> PathBuf {
        PathBuf::from(AppDirectories::Config.to_string()).join(CONFIG_FILE)
    }

    pub fn get_file_contents() -> &'static str {
        include_str!("../manga-tui-config.toml")
    }

    pub fn write_if_not_exists(base_directory: &Path) -> Result<(), std::io::Error> {
        let config_file = base_directory.join(AppDirectories::Config.to_string()).join(CONFIG_FILE);

        if !exists!(&config_file) {
            let contents = Self::get_file_contents();

            let mut config_file = File::create(config_file)?;
            config_file.write_all(contents.as_bytes())?
        }

        Ok(())
    }
}
