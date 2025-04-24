use std::error::Error;
use std::fs::{File, OpenOptions, create_dir_all};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::LazyLock;

use manga_tui::exists;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter};
use toml::Table;

use crate::backend::AppDirectories;
use crate::logger::{DefaultLogger, ILogger};

static CONFIG_FILE_NAME: &str = "config.toml";

static CONFIG: OnceCell<MangaTuiConfig> = OnceCell::new();

static CONFIG_DIR_PATH: LazyLock<Option<PathBuf>> = LazyLock::new(|| {
    directories::ProjectDirs::from("", "", "manga-tui").map(|project_dirs| project_dirs.config_dir().to_path_buf())
});

/// Defines what a parameter in the config file should implement
/// `comments` for explaining to the user what the config param does
/// `values` to define what kind of values the config param can take
/// `defaults` to provide what the config param defaults to
/// and `param` which is the actual config param as defined by toml syntax
///
///  # Example
///  \# The format of the manga downloaded
///  \# values: cbz , raw, epub, pdf
///  \# default: cbz
///  download_type = "cbz"
trait ConfigParam {
    /// The name by which the config param is identified
    fn name(&self) -> &'static str;
    fn comments(&self) -> &'static str;
    fn values(&self) -> &'static str;
    fn defaults(&self) -> &'static str;
    fn param(&self) -> String;

    fn build_parameter(&self) -> String {
        let comments = self.comments();
        let values = self.values();
        let defaults = self.defaults();
        let param = self.param();

        let result = format!("# {comments}\n# values: {values}\n# default: {defaults}\n{param}\n\n");

        result
    }
}

#[derive(Debug, Default)]
struct DownloadTypeParam;

impl ConfigParam for DownloadTypeParam {
    fn name(&self) -> &'static str {
        "download_type"
    }

    fn comments(&self) -> &'static str {
        "The format of the manga downloaded"
    }

    fn values(&self) -> &'static str {
        "cbz, raw, epub, pdf"
    }

    fn defaults(&self) -> &'static str {
        "cbz"
    }

    fn param(&self) -> String {
        String::from(r#"download_type = "cbz""#)
    }
}

#[derive(Debug, Default)]
struct ImageQualityParam;

impl ConfigParam for ImageQualityParam {
    fn name(&self) -> &'static str {
        "image_quality"
    }

    fn comments(&self) -> &'static str {
        "Download image quality, low quality means images are compressed and is recommended for slow internet connections"
    }

    fn values(&self) -> &'static str {
        "low, high "
    }

    fn defaults(&self) -> &'static str {
        "low"
    }

    fn param(&self) -> String {
        String::from(r#"image_quality = "low""#)
    }
}

#[derive(Debug, Default)]
struct AutoBookmarkParam;

impl ConfigParam for AutoBookmarkParam {
    fn name(&self) -> &'static str {
        "auto_bookmark"
    }

    fn comments(&self) -> &'static str {
        "Whether or not bookmarking is done automatically, if false you decide which chapter to bookmark"
    }

    fn values(&self) -> &'static str {
        "true, false"
    }

    fn defaults(&self) -> &'static str {
        "true"
    }

    fn param(&self) -> String {
        String::from(r#"auto_bookmark = true"#)
    }
}

#[derive(Debug, Default)]
struct AmountPagesParam;

impl ConfigParam for AmountPagesParam {
    fn name(&self) -> &'static str {
        "amount_pages"
    }

    fn comments(&self) -> &'static str {
        "Pages around the currently selected page to try to prefetch"
    }

    fn values(&self) -> &'static str {
        "0-255"
    }

    fn defaults(&self) -> &'static str {
        "5"
    }

    fn param(&self) -> String {
        String::from(r#"amount_pages = 5"#)
    }
}
#[derive(Debug, Default)]
struct TrackReadingWhenDownload;

impl ConfigParam for TrackReadingWhenDownload {
    fn name(&self) -> &'static str {
        "track_reading_when_download"
    }

    fn comments(&self) -> &'static str {
        "Whether or not downloading a manga counts as reading it on services like anilist"
    }

    fn values(&self) -> &'static str {
        "true, false"
    }

    fn defaults(&self) -> &'static str {
        "false"
    }

    fn param(&self) -> String {
        String::from(r#"track_reading_when_download = false"#)
    }
}

/// It's main job is to create the config file with the provided config params or update it if it
/// already exists, and also to create the config directory if it does not exist
struct ConfigBuilder<'a> {
    params: Vec<Box<dyn ConfigParam>>,
    directory_path: &'a Path,
}

/// The params the config file has
fn config_params() -> Vec<Box<dyn ConfigParam>> {
    vec![
        Box::new(DownloadTypeParam),
        Box::new(ImageQualityParam),
        Box::new(AmountPagesParam),
        Box::new(AutoBookmarkParam),
        Box::new(TrackReadingWhenDownload),
    ]
}

impl<'a> ConfigBuilder<'a> {
    fn new() -> Self {
        Self {
            params: config_params(),
            directory_path: Path::new("./"),
        }
    }

    fn dir_path<P: AsRef<Path> + ?Sized>(mut self, dir_path: &'a P) -> Self {
        self.directory_path = dir_path.as_ref();
        self
    }

    fn with_params(params: Vec<Box<dyn ConfigParam>>) -> Self {
        Self {
            params,
            directory_path: Path::new("./"),
        }
    }

    /// Creates the directory where the config file will be, so the final path looks something like
    /// this: `~/.config/manga-tui/`
    fn create_directory_if_not_exists(&self) -> Result<(), std::io::Error> {
        if !exists!(self.directory_path) {
            create_dir_all(self.directory_path)?
        }
        Ok(())
    }

    /// Creates the config file if it does not exist and write to it with the default configuration, if it exists then return the
    /// file handle which should be updated if any config param is missing.
    /// The resulting path for the config file will look something like: `~/.config/manga-tui/config.toml`
    fn create_file_if_not_exists(&self) -> Result<File, std::io::Error> {
        self.create_directory_if_not_exists()?;

        let config_path = self.directory_path.join(CONFIG_FILE_NAME);

        let mut open_options = OpenOptions::new();
        open_options.append(true).read(true);

        let file = if !exists!(&config_path) {
            let mut file = File::create_new(&config_path)?;
            self.write_config(&mut file)?;
            open_options.open(config_path)?
        } else {
            let mut file = open_options.open(&config_path)?;

            self.update_existing_config(&mut file).map_err(|e| {
                std::io::Error::other(format!("Could not update existing config: more details about the error: \n\n {e}"))
            })?;

            open_options.open(config_path)?
        };

        Ok(file)
    }

    /// Writes to the file with the config, if the config file is not empty then this method should
    /// not be used
    fn write_config(&self, mut file: impl Write + Read) -> Result<(), std::io::Error> {
        for config_param in &self.params {
            file.write_all(config_param.build_parameter().as_bytes())?;
        }
        Ok(())
    }

    /// Checks for params which are missing in the existing config, either due to updates or the
    /// user removing them accidentally
    fn update_existing_config(&self, mut config: impl Write + Read) -> Result<(), Box<dyn Error>> {
        let mut contents = String::new();
        config.read_to_string(&mut contents)?;

        let as_toml_table = toml::Table::from_str(&contents)?;

        for param in &self.params {
            if !as_toml_table.contains_key(param.name()) {
                config.write_all(param.build_parameter().as_bytes())?
            }
        }

        config.flush()?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MangaTuiConfig {
    pub download_type: DownloadType,
    pub image_quality: ImageQuality,
    pub auto_bookmark: bool,
    pub amount_pages: u8,
    pub track_reading_when_download: bool,
}

#[derive(Default, Debug, Serialize, Deserialize, Display, EnumIter, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadType {
    #[default]
    Cbz,
    Raw,
    Epub,
    Pdf,
}

#[derive(Default, Debug, Serialize, Deserialize, Display, EnumIter, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImageQuality {
    #[default]
    Low,
    High,
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

impl MangaTuiConfig {
    pub fn get() -> &'static Self {
        CONFIG.get_or_init(MangaTuiConfig::default)
    }

    fn read_config_file(mut config: impl Read) -> Result<Self, Box<dyn Error>> {
        let mut contents = String::new();
        config.read_to_string(&mut contents)?;

        Ok(toml::from_str(&contents)?)
    }

    fn from_str(raw_file: &str) -> Result<Self, Box<dyn Error>> {
        Ok(toml::from_str(raw_file)?)
    }
}

/// As a part of the config setup it must:
/// - create the config file if it doesnt exist or update it if it's missing configuration,
/// - read the config file again this time to read the configuration and set it globally
pub fn build_config_file() -> Result<(), Box<dyn Error>> {
    let path = CONFIG_DIR_PATH.as_ref().ok_or("No home directory was found")?;

    let config_builder = ConfigBuilder::new().dir_path(path);

    let mut config = config_builder.create_file_if_not_exists()?;

    let config = MangaTuiConfig::read_config_file(&mut config).unwrap_or_default();

    CONFIG.get_or_init(|| config);

    Ok(())
}

pub fn get_config_directory_path() -> PathBuf {
    CONFIG_DIR_PATH.as_ref().expect("Failed to find home directory").to_path_buf()
}

#[cfg(test)]
mod tests {

    use std::fs;
    use std::io::Cursor;

    use pretty_assertions::{assert_eq, assert_str_eq};

    use super::*;

    const CONFIG_TEST_DIRECTORY_PATH: &str = "./test_results/config_test_dir/";

    #[test]
    #[ignore]
    fn config_builder_creates_config_file() -> Result<(), Box<dyn Error>> {
        let config = ConfigBuilder::new().dir_path(CONFIG_TEST_DIRECTORY_PATH);

        let mut contents = String::new();

        let mut file = config.create_file_if_not_exists()?;

        let result = file.read_to_string(&mut contents)?;

        let contents = dbg!(contents);

        assert!(!contents.is_empty());

        fs::read(PathBuf::from(CONFIG_TEST_DIRECTORY_PATH).join("config.toml"))?;

        /// Running it a second time should not panic
        let _ = config.create_file_if_not_exists()?;

        Ok(())
    }

    #[test]
    fn config_builder_writes_to_the_config_file_with_parameters() -> Result<(), Box<dyn Error>> {
        let config = ConfigBuilder::new().dir_path(CONFIG_TEST_DIRECTORY_PATH);

        let mut test_file = Cursor::new(Vec::new());
        let expected = r#"
# The format of the manga downloaded
# values: cbz, raw, epub, pdf
# default: cbz
download_type = "cbz"

# Download image quality, low quality means images are compressed and is recommended for slow internet connections
# values: low, high 
# default: low
image_quality = "low"

# Pages around the currently selected page to try to prefetch
# values: 0-255
# default: 5
amount_pages = 5

# Whether or not bookmarking is done automatically, if false you decide which chapter to bookmark
# values: true, false
# default: true
auto_bookmark = true

# Whether or not downloading a manga counts as reading it on services like anilist
# values: true, false
# default: false
track_reading_when_download = false"#;

        config.write_config(&mut test_file)?;

        let expected = MangaTuiConfig::default();

        let file_contents = String::from_utf8(test_file.into_inner())?;

        assert_eq!(expected, MangaTuiConfig::from_str(&file_contents)?);

        Ok(())
    }

    #[test]
    fn config_builder_adds_missing_params() -> Result<(), Box<dyn Error>> {
        let config = ConfigBuilder::with_params(vec![Box::new(AmountPagesParam), Box::new(AutoBookmarkParam)])
            .dir_path(CONFIG_TEST_DIRECTORY_PATH);

        let mut test_file = Cursor::new(Vec::new());

        let existing_file = r#"
# The format of the manga downloaded
# values: cbz, raw, epub, pdf
# default: cbz
download_type = "cbz"
"#;

        config.update_existing_config(&mut test_file)?;

        let file_contents = String::from_utf8(test_file.into_inner())?;
        let expected = toml::Table::from_str(&file_contents)?;

        assert!(expected.contains_key("amount_pages"));
        assert!(expected.contains_key("auto_bookmark"));

        Ok(())
    }

    #[test]
    fn config_is_parse_from_raw_file() -> Result<(), Box<dyn Error>> {
        let config = ConfigBuilder::new().dir_path(CONFIG_TEST_DIRECTORY_PATH);

        let mut test_file = Cursor::new(Vec::new());

        config.write_config(&mut test_file)?;

        test_file.set_position(0);

        assert_eq!(MangaTuiConfig::default(), MangaTuiConfig::read_config_file(test_file)?);

        Ok(())
    }
}
