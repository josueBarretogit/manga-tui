use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use manga_tui::exists;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter};
use toml::Table;

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

#[derive(Debug, Serialize, Deserialize)]
pub struct MangaTuiConfig {
    pub download_type: DownloadType,
    pub image_quality: ImageQuality,
    pub auto_bookmark: bool,
    pub amount_pages: u8,
    pub track_reading_when_download: bool,
}

impl Default for MangaTuiConfig {
    fn default() -> Self {
        Self {
            amount_pages: 5,
            auto_bookmark: true,
            download_type: DownloadType::default(),
            image_quality: ImageQuality::default(),
            track_reading_when_download: false,
        }
    }
}

pub static CONFIG: OnceCell<MangaTuiConfig> = OnceCell::new();

static CONFIG_TEMPLATE: &str = include_str!("../manga-tui-config.toml");

impl MangaTuiConfig {
    pub fn get() -> &'static Self {
        CONFIG.get_or_init(MangaTuiConfig::default)
    }

    pub fn read_raw_config(base_directory: &Path) -> Result<String, std::io::Error> {
        let mut config_file = Self::get_config_file(base_directory)?;

        let mut contents = String::new();
        config_file.read_to_string(&mut contents)?;

        Ok(contents)
    }

    pub fn get_config_file_path() -> PathBuf {
        AppDirectories::Config.get_path()
    }

    pub fn get_config_template() -> &'static str {
        CONFIG_TEMPLATE
    }

    pub fn write_if_not_exists(base_directory: &Path) -> Result<(), std::io::Error> {
        let config_file = base_directory.join(Self::get_config_file_path());

        if !exists!(&config_file) {
            let contents = Self::get_config_template();

            let mut config_file = File::create(config_file).expect("cannot create conf file");
            config_file.write_all(contents.as_bytes())?
        }

        Ok(())
    }

    // refactor this function to make it more dynamic, at the moment the fields are hardcoded
    fn add_missing_fields(mut file: impl Write + Read, existing_config: Table) -> Result<Self, std::io::Error> {
        if !existing_config.contains_key("amount_pages") {
            file.write_all(
                "
# Pages around the currently selected page to try to prefetch
# values : 0-255
# default : 5
amount_pages = 5
"
                .as_bytes(),
            )?;
        }

        if !existing_config.contains_key("auto_bookmark") {
            file.write_all(
                "
# Whether or not bookmarking is done automatically, if false you decide which chapter to bookmark
# values : true, false
# default : true
auto_bookmark = true
"
                .as_bytes(),
            )?;
        }

        if !existing_config.contains_key("track_reading_when_download") {
            file.write_all(
                "
# Whether or not downloading a manga counts as reading it on services like anilist
# values : true, false
# default : false
track_reading_when_download = false
"
                .as_bytes(),
            )?;
        }

        let mut contents = String::new();

        file.read_to_string(&mut contents)?;

        let config = toml::from_str(&contents).unwrap_or_default();

        Ok(config)
    }

    /// Returns the config with either the fields already existing or with the new ones
    pub fn update_existing_config(config: &str, base_directory: &Path) -> Result<Self, Box<dyn Error>> {
        let already_existing: Table = toml::Table::from_str(config)?;

        let mut config_file = Self::get_config_file(base_directory)?;

        Ok(Self::add_missing_fields(&mut config_file, already_existing)?)
    }

    pub fn get_config_file(base_directory: &Path) -> Result<File, std::io::Error> {
        OpenOptions::new()
            .append(true)
            .read(true)
            .open(base_directory.join(Self::get_config_file_path()))
    }
}

#[cfg(test)]
mod tests {

    use std::io::Cursor;

    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_adds_missing_field_to_config() {
        let mut test_file = Cursor::new(Vec::new());

        let current_contents = r#"
        # The format of the manga downloaded
        # values : cbz , raw, epub
        # default : cbz
        download_type = "cbz"

        # Download image quality, low quality means images are compressed and is recommended for slow internet connections
        # values : low, high
        # default : low
        image_quality = "low"
                "#;

        let expected = r#"
    # Whether or not bookmarking is done automatically, if false you decide which chapter to bookmark
    # values : true, false
    # default : true
    auto_bookmark = true

    # Pages around the currently selected page to try to prefetch
    # values : 0-255
    #default : 5
    amount_pages = 5

# Whether or not downloading a manga counts as reading it on services like anilist
# values : true, false
# default : false
track_reading_when_download = false
                "#;

        MangaTuiConfig::add_missing_fields(&mut test_file, current_contents.parse::<Table>().unwrap()).unwrap();

        let expected_table = expected.parse::<Table>().unwrap();
        let result = test_file.into_inner();

        let result: Table = String::from_utf8(result).unwrap().parse().unwrap();

        assert_eq!(expected_table, result);
    }

    #[test]
    fn it_does_not_add_already_existing_keys() {
        let current_contents = r#"
# Whether or not bookmarking is done automatically, if false you decide which chapter to bookmark
# values : true, false
# default : true
auto_bookmark = true

# Pages around the currently selected page to try to prefetch
# values : 0-255
#default : 5
amount_pages = 5

# Whether or not downloading a manga counts as reading it on services like anilist
# values : true, false
# default : false
track_reading_when_download = false
            "#;

        let mut test_file = Cursor::new(Vec::new());

        test_file.write_all(current_contents.as_bytes()).unwrap();

        let expected = r#"
# Whether or not bookmarking is done automatically, if false you decide which chapter to bookmark
# values : true, false
# default : true
auto_bookmark = true

# Pages around the currently selected page to try to prefetch
# values : 0-255
#default : 5
amount_pages = 5

# Whether or not downloading a manga counts as reading it on services like anilist
# values : true, false
# default : false
track_reading_when_download = false
            "#;

        MangaTuiConfig::add_missing_fields(&mut test_file, current_contents.parse::<Table>().unwrap()).unwrap();

        let result = test_file.into_inner();

        assert_eq!(expected, String::from_utf8(result).unwrap());
    }
}
