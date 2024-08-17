use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use manga_tui::exists;
use once_cell::sync::{Lazy, OnceCell};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter};

use crate::backend::AppDirectories;

#[derive(Default, Debug, Serialize, Deserialize, Display, EnumIter)]
#[serde(rename_all = "snake_case")]
pub enum DownloadType {
    #[default]
    Cbz,
    Raw,
    Pdf,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct MangaTuiConfig {
    pub download_type: DownloadType,
}

pub static CONFIG_FILE: &str = "manga-tui-config.toml";

pub static CONFIG: OnceCell<MangaTuiConfig> = OnceCell::new();

impl MangaTuiConfig {
    pub fn get() -> &'static Self {
        CONFIG.get().expect("Could not get download type")
    }

    pub fn read_config(base_directory: &Path) -> Result<String, std::io::Error> {
        let config_file = base_directory
            .join(AppDirectories::Config.to_string())
            .join(CONFIG_FILE);

        let mut config_file = File::open(config_file)?;

        let mut contents = String::new();
        config_file.read_to_string(&mut contents)?;

        Ok(contents)
    }

    pub fn write_config(base_directory: &Path) -> Result<(), std::io::Error> {
        let contents = r#"
        # available values : cbz, raw, pdf
        download_type = "cbz"

        "#;

        let config_file = base_directory
            .join(AppDirectories::Config.to_string())
            .join(CONFIG_FILE);

        if !exists!(&config_file) {
            let mut config_file = File::create(config_file)?;
            config_file.write_all(contents.as_bytes())?
        }

        Ok(())
    }
}
