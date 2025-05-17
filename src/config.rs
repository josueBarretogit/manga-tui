//! Configuration management for manga-tui.
//!
//! This module provides the configuration system for manga-tui, including
//! config file creation, updating, and reading. It supports default values,
//! table parameters, and ensures the config file is always up-to-date with
//! the latest parameters.

use std::error::Error;
use std::fmt::Write as FmtWrite;
use std::fs::{File, OpenOptions, create_dir_all};
use std::io::{Cursor, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::LazyLock;

use manga_tui::exists;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter};
use toml::Table;

use crate::backend::AppDirectories;
use crate::backend::manga_provider::MangaProviders;
use crate::cli::Credentials;
use crate::logger::{DefaultLogger, ILogger};

static CONFIG_FILE_NAME: &str = "config.toml";

static CONFIG_FILE_NAME_BACKUP: &str = "config_backup.toml";

static CONFIG: OnceCell<MangaTuiConfig> = OnceCell::new();

static CONFIG_DIR_PATH: LazyLock<Option<PathBuf>> = LazyLock::new(|| {
    directories::ProjectDirs::from("", "", "manga-tui").map(|project_dirs| project_dirs.config_dir().to_path_buf())
});

/// Trait for a single configuration parameter.
///
/// Implementors define the name, documentation, allowed values, default,
/// and how the parameter is rendered in the config file.
trait ConfigParam {
    /// The name of the config parameter.
    fn name(&self) -> &'static str;
    /// Description of what the parameter does.
    fn comments(&self) -> &'static str;
    /// Allowed values for the parameter.
    fn values(&self) -> &'static str;
    /// Default value for the parameter.
    fn defaults(&self) -> &'static str;
    /// The TOML representation of the parameter.
    fn param(&self) -> String {
        format!("{} = {}", self.name(), self.defaults())
    }

    /// Builds the full parameter entry, including comments, for the config file.
    fn build_parameter(&self) -> String {
        let comments = self.comments();
        let values = self.values();
        let defaults = self.defaults();
        let param = self.param();

        let result = format!("# {comments}\n# values: {values}\n# default: {defaults}\n{param}\n\n");

        result
    }
}

/// Trait for a table of configuration parameters (TOML tables).
///
/// Implementors define the table name, documentation, and the parameters
/// contained within the table.
trait TableParam {
    /// The TOML table name.
    fn table_name(&self) -> &'static str;
    /// Description of the table.
    fn comments(&self) -> &'static str;
    /// The parameters contained in the table.
    fn parameters(&self) -> Vec<Box<dyn ConfigParam>>;

    fn add_parameters(&self, params: Vec<Box<dyn ConfigParam>>) -> String {
        params.iter().fold(String::new(), |mut accum, param| {
            let _ = write!(accum, "{}", param.build_parameter());
            accum
        })
    }

    fn build_full_table(&self) -> String {
        let table_name = self.table_name();
        let comments = self.comments();
        let added_parameters = self.add_parameters(self.parameters());

        format!("# {comments}\n[{table_name}]\n{added_parameters}")
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
        r#""cbz""#
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
        r#""low""#
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
}

#[derive(Debug, Default)]
struct CheckNewUpdates;

impl ConfigParam for CheckNewUpdates {
    fn name(&self) -> &'static str {
        "check_new_updates"
    }

    fn comments(&self) -> &'static str {
        "Enable / disable checking for new updates"
    }

    fn values(&self) -> &'static str {
        "true, false"
    }

    fn defaults(&self) -> &'static str {
        "true"
    }
}

#[derive(Debug, Default)]
struct DefaultMangaProvider;

impl ConfigParam for DefaultMangaProvider {
    fn name(&self) -> &'static str {
        "default_manga_provider"
    }

    fn comments(&self) -> &'static str {
        "Sets which manga provider will be used when running manga-tui, \n# you can override it by running manga-tui with the -p flag like this: manga-tui -p weebcentral"
    }

    fn values(&self) -> &'static str {
        "mangadex, weebcentral"
    }

    fn defaults(&self) -> &'static str {
        r#""mangadex""#
    }
}

#[derive(Debug, Default)]
struct AnilistClientId;

impl ConfigParam for AnilistClientId {
    fn name(&self) -> &'static str {
        "client_id"
    }

    fn comments(&self) -> &'static str {
        "Your client id from your anilist account, leave it as 0 if you don't want to use the config file to read your anilist credentials"
    }

    fn values(&self) -> &'static str {
        "string"
    }

    fn defaults(&self) -> &'static str {
        r#""""#
    }
}

#[derive(Debug, Default)]
struct AnilistAccessToken;

impl ConfigParam for AnilistAccessToken {
    fn name(&self) -> &'static str {
        "access_token"
    }

    fn comments(&self) -> &'static str {
        "Your acces token from your anilist account, leave it as an empty string \"\" if you don't want to use the config file to read your anilist credentials"
    }

    fn values(&self) -> &'static str {
        "string"
    }

    fn defaults(&self) -> &'static str {
        "\"\""
    }
}

#[derive(Debug, Default)]
struct AnilistConfigTable;

impl TableParam for AnilistConfigTable {
    fn table_name(&self) -> &'static str {
        "anilist"
    }

    fn comments(&self) -> &'static str {
        "Anilist-related config"
    }

    fn parameters(&self) -> Vec<Box<dyn ConfigParam>> {
        vec![Box::new(AnilistClientId), Box::new(AnilistAccessToken)]
    }
}

/// Builder for the configuration file.
///
/// Handles creation, updating, and writing of the config file and its directory.
struct ConfigBuilder<'a> {
    params: Vec<Box<dyn ConfigParam>>,
    table_params: Vec<Box<dyn TableParam>>,
    /// The directory under which the file is located
    base_directory: &'a Path,
}

/// The params the config file has which look like: param_name = "value"
fn config_params() -> Vec<Box<dyn ConfigParam>> {
    vec![
        Box::new(DownloadTypeParam),
        Box::new(ImageQualityParam),
        Box::new(AmountPagesParam),
        Box::new(AutoBookmarkParam),
        Box::new(TrackReadingWhenDownload),
        Box::new(CheckNewUpdates),
        Box::new(DefaultMangaProvider),
    ]
}

fn table_config_params() -> Vec<Box<dyn TableParam>> {
    vec![Box::new(AnilistConfigTable)]
}

impl<'a> ConfigBuilder<'a> {
    /// Creates a new `ConfigBuilder` with default parameters and tables.
    fn new() -> Self {
        Self {
            table_params: table_config_params(),
            params: config_params(),
            base_directory: Path::new("./"),
        }
    }

    /// Sets the base directory for the config file.
    fn dir_path<P: AsRef<Path> + ?Sized>(mut self, dir_path: &'a P) -> Self {
        self.base_directory = dir_path.as_ref();
        self
    }

    fn with_params(params: Vec<Box<dyn ConfigParam>>) -> Self {
        Self {
            table_params: vec![],
            params,
            base_directory: Path::new("./"),
        }
    }

    fn with_table_config_params(table_params: Vec<Box<dyn TableParam>>) -> Self {
        Self {
            table_params,
            params: vec![],
            base_directory: Path::new("./"),
        }
    }

    /// Creates the directory where the config file will be, so the final path looks something like
    /// this: `~/.config/manga-tui/`
    fn create_directory_if_not_exists(&self) -> Result<(), std::io::Error> {
        if !exists!(self.base_directory) {
            create_dir_all(self.base_directory)?
        }
        Ok(())
    }

    /// Returns the path where the config file is, usually `~/.config/manga-tui/config.toml`
    fn get_config_file_path(&self) -> PathBuf {
        self.base_directory.join(CONFIG_FILE_NAME).to_path_buf()
    }

    /// Returns the path where the config file bakcup is, usually
    /// `~/.config/manga-tui/config_backup.toml`
    fn get_config_backup_file_path(&self) -> PathBuf {
        self.base_directory.join(CONFIG_FILE_NAME_BACKUP).to_path_buf()
    }

    /// Creates the config file if it does not exist and write to it with the default configuration, if it exists then return the
    /// file handle which should be updated if any config param is missing.
    /// The resulting path for the config file will look something like: `~/.config/manga-tui/config.toml`
    fn create_file_if_not_exists(&self) -> Result<File, std::io::Error> {
        self.create_directory_if_not_exists()?;

        let config_path = self.get_config_file_path();

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
            })?
        };

        Ok(file)
    }

    /// Writes to the file with the config, if the config file is not empty then this method should
    /// not be used
    fn write_config(&self, mut file: impl Write + Read) -> Result<(), std::io::Error> {
        for config_param in &self.params {
            file.write_all(config_param.build_parameter().as_bytes())?;
        }

        for table_param in &self.table_params {
            file.write_all(table_param.build_full_table().as_bytes())?
        }

        file.flush()?;
        Ok(())
    }

    /// Appends missing table parameters to the config file.
    ///
    /// # Why tables are appended at the end
    /// In TOML, table definitions (e.g., `[table_name]`) must appear after all single-value parameters
    /// (e.g., `param = "value"`). If a table is inserted before or in the middle of single-value parameters,
    /// those parameters would be interpreted as belonging to the last table, which is not intended.
    /// Therefore, any missing table parameters are always appended at the end of the file to maintain
    /// correct TOML structure and parsing.
    fn append_missing_table_params(&self, file_contents: &str) -> Result<String, Box<dyn Error>> {
        let as_toml: Table = file_contents.parse()?;
        let mut updated_config = file_contents.to_string();

        for table in &self.table_params {
            if !as_toml.contains_key(table.table_name()) {
                updated_config = format!("{updated_config}{}", table.build_full_table());
            }
        }

        Ok(updated_config)
    }

    /// Prepends missing single parameters to the config file.
    ///
    /// # Why single parameters are prepended
    /// In TOML, single-value parameters (e.g., `param = "value"`) that appear after a table definition
    /// (e.g., `[table_name]`) are considered part of that table. To ensure that all single-value parameters
    /// are part of the root table (and not accidentally included in a table), any missing single parameters
    /// are always prepended to the top of the file, before any table definitions.
    fn prepend_missing_config_param(&self, file_contents: &str) -> Result<String, Box<dyn Error>> {
        let as_toml_parameter: Table = file_contents.parse()?;

        let mut updated_config = file_contents.to_string();

        for param in &self.params {
            if !as_toml_parameter.contains_key(param.name()) {
                updated_config = format!("{}{updated_config}", param.build_parameter());
            }
        }

        Ok(updated_config)
    }

    /// Updates the config file with any missing parameters or tables.
    fn update_existing_config(&self, mut config: impl Write + Read) -> Result<File, Box<dyn Error>> {
        let mut contents = String::new();

        config.read_to_string(&mut contents)?;

        let updated = self.prepend_missing_config_param(&contents)?;

        let updated = self.append_missing_table_params(&updated)?;

        let new_config_file = self.commit_changes(&updated)?;

        Ok(new_config_file)
    }

    /// Commits changes to the config file, creating a backup and writing the new config.
    fn commit_changes(&self, updated_config: &str) -> Result<File, Box<dyn Error>> {
        let config_file_path = self.get_config_file_path();
        let config_file_backup_path = self.get_config_backup_file_path();

        std::fs::copy(&config_file_path, &config_file_backup_path)?;

        std::fs::remove_file(&config_file_path)?;

        let mut open_options = OpenOptions::new();
        open_options.append(true).read(true).create(true);

        let mut new_config = open_options.open(&config_file_path)?;

        new_config.write_all(updated_config.as_bytes())?;

        new_config.flush()?;

        let new_config = open_options.open(config_file_path)?;

        std::fs::remove_file(&config_file_backup_path)?;

        Ok(new_config)
    }
}

/// Configuration for Anilist integration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnilistConfig {
    /// Credentials for Anilist API.
    #[serde(flatten)]
    pub credentials: Credentials,
}

impl Default for AnilistConfig {
    fn default() -> Self {
        Self {
            credentials: Credentials {
                access_token: "".to_string(),
                client_id: "".to_string(),
            },
        }
    }
}

/// Main configuration struct for manga-tui.
///
/// This struct is deserialized from the config file and contains all
/// user-configurable options.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MangaTuiConfig {
    /// The format to download manga in.
    pub download_type: DownloadType,
    /// The image quality for downloads.
    pub image_quality: ImageQuality,
    /// Whether to automatically bookmark chapters.
    pub auto_bookmark: bool,
    /// Number of pages to prefetch around the current page.
    pub amount_pages: u8,
    /// Whether downloading counts as reading for tracking services.
    pub track_reading_when_download: bool,
    /// Whether to check for new updates.
    pub check_new_updates: bool,
    /// The default manga provider.
    pub default_manga_provider: MangaProviders,
    /// Anilist configuration.
    pub anilist: AnilistConfig,
}

/// Download format options.
#[derive(Default, Debug, Serialize, Deserialize, Display, EnumIter, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadType {
    #[default]
    Cbz,
    Raw,
    Epub,
    Pdf,
}

/// Image quality options.
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
            check_new_updates: true,
            download_type: DownloadType::default(),
            image_quality: ImageQuality::default(),
            track_reading_when_download: false,
            default_manga_provider: MangaProviders::default(),
            anilist: AnilistConfig::default(),
        }
    }
}

impl MangaTuiConfig {
    /// Returns a reference to the global configuration.
    pub fn get() -> &'static Self {
        CONFIG.get_or_init(MangaTuiConfig::default)
    }

    /// Reads the configuration from a reader.
    fn read_config_file(mut config: impl Read) -> Result<Self, Box<dyn Error>> {
        let mut contents = String::new();
        config.read_to_string(&mut contents)?;

        Ok(toml::from_str(&contents)?)
    }

    /// Parses the configuration from a string.
    fn from_str(raw_file: &str) -> Result<Self, Box<dyn Error>> {
        Ok(toml::from_str(raw_file)?)
    }

    /// Returns Anilist credentials if both client_id and access_token are set.
    pub fn check_anilist_credentials(&self) -> Option<Credentials> {
        if self.anilist.credentials.access_token.is_empty() || self.anilist.credentials.access_token.is_empty() {
            return None;
        }

        Some(Credentials {
            access_token: self.anilist.credentials.access_token.clone(),
            client_id: self.anilist.credentials.client_id.clone(),
        })
    }
}

/// Builds the config file, creating or updating as needed, and sets the global config.
///
/// Returns an error if the config directory cannot be found or written.
pub fn build_config_file() -> Result<(), Box<dyn Error>> {
    let path = CONFIG_DIR_PATH.as_ref().ok_or("No home directory was found")?;

    let config_builder = ConfigBuilder::new().dir_path(path);

    let mut config = config_builder.create_file_if_not_exists()?;

    let config = MangaTuiConfig::read_config_file(&mut config).unwrap_or_default();

    CONFIG.get_or_init(|| config);

    Ok(())
}

/// Returns the path to the config directory.
pub fn get_config_directory_path() -> PathBuf {
    CONFIG_DIR_PATH.as_ref().expect("Failed to find home directory").to_path_buf()
}

#[cfg(test)]
mod tests {

    use std::fmt::{Debug, Write as FmtWrite};
    use std::fs;
    use std::io::{Cursor, Write};

    use pretty_assertions::{assert_eq, assert_str_eq};

    use super::*;

    const CONFIG_TEST_DIRECTORY_PATH: &str = "./test_results/config_test_dir/";

    /// Should contain two keys:
    /// param = ""
    /// param2 = ""
    struct TestTableConfigParam;

    impl TableParam for TestTableConfigParam {
        fn table_name(&self) -> &'static str {
            "test_table"
        }

        fn comments(&self) -> &'static str {
            "This table contains some example keys"
        }

        fn parameters(&self) -> Vec<Box<dyn ConfigParam>> {
            vec![Box::new(TestConfigParam), Box::new(TestConfigParam2)]
        }
    }

    /// Example single param which should look like this: "param = """
    struct TestConfigParam;
    struct TestConfigParam2;

    impl ConfigParam for TestConfigParam {
        fn name(&self) -> &'static str {
            "param"
        }

        fn comments(&self) -> &'static str {
            "A test parameter of a table param"
        }

        fn values(&self) -> &'static str {
            "string"
        }

        fn defaults(&self) -> &'static str {
            "empty string"
        }

        fn param(&self) -> String {
            r#"param = """#.to_string()
        }
    }

    impl ConfigParam for TestConfigParam2 {
        fn name(&self) -> &'static str {
            "param2"
        }

        fn comments(&self) -> &'static str {
            "A test parameter of a table param"
        }

        fn values(&self) -> &'static str {
            "string"
        }

        fn defaults(&self) -> &'static str {
            "empty string"
        }

        fn param(&self) -> String {
            r#"param2 = """#.to_string()
        }
    }

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
    fn config_builder_writes_to_the_config_file_with_default_parameters() -> Result<(), Box<dyn Error>> {
        let config = ConfigBuilder::new().dir_path(CONFIG_TEST_DIRECTORY_PATH);

        let mut test_file = Cursor::new(Vec::new());

        config.write_config(&mut test_file)?;

        let expected = MangaTuiConfig::default();

        let file_contents = String::from_utf8(test_file.into_inner())?;

        assert_eq!(expected, MangaTuiConfig::from_str(&file_contents)?);

        Ok(())
    }

    //In this thest the config param should be added at the beginning of the file, otherwise it
    //will belong to the last table param
    #[test]
    fn config_builder_adds_missing_params_single_params() -> Result<(), Box<dyn Error>> {
        let config = ConfigBuilder::with_params(vec![Box::new(AmountPagesParam), Box::new(AutoBookmarkParam)]);

        let existing_file_content = r#"
        # The format of the manga downloaded
        # values: cbz, raw, epub, pdf
        # default: cbz
        download_type = "cbz"

        [some_table]
        param = 1

        "#;

        let updated_config = config.prepend_missing_config_param(&existing_file_content)?;

        let expected = toml::Table::from_str(&updated_config)?;

        assert!(expected.contains_key("amount_pages"));
        assert!(expected.contains_key("auto_bookmark"));

        assert_eq!("cbz", expected.get("download_type").unwrap().as_str().unwrap());

        Ok(())
    }

    #[test]
    fn config_builder_adds_missing_table_params_at_the_end() -> Result<(), Box<dyn Error>> {
        let config = ConfigBuilder::with_table_config_params(vec![Box::new(TestTableConfigParam)]);

        let existing_file = r#"
        download_type = "epub"

        check_new_updates = true

        [example_table]
        some_param = 1

        another_test_param_which_belongs_to_table = false
        "#;

        let updated = config.append_missing_table_params(existing_file)?;

        println!("{updated}");

        let new_config = toml::Table::from_str(&updated)?;

        let expected = new_config.get("test_table").unwrap();

        assert!(expected.is_table());

        assert_eq!("epub", new_config.get("download_type").unwrap().as_str().unwrap());
        assert_eq!(true, new_config.get("check_new_updates").unwrap().as_bool().unwrap());

        Ok(())
    }

    #[test]
    fn anilist_credentials_are_returned_if_not_empty() {
        let mut config = MangaTuiConfig::default();

        assert!(config.check_anilist_credentials().is_none());

        config.anilist.credentials.client_id = "12938".to_string();

        assert!(config.check_anilist_credentials().is_none());

        config.anilist.credentials.client_id = "1290347".to_string();
        config.anilist.credentials.access_token = "some_token".to_string();

        assert_eq!(
            Credentials {
                client_id: config.anilist.credentials.client_id.clone(),
                access_token: config.anilist.credentials.access_token.clone()
            },
            config.check_anilist_credentials().unwrap()
        );
    }

    #[test]
    fn table_config_param_is_built_with_expected_params() {
        let table = TestTableConfigParam;

        let expected = r#"# This table contains some example keys
[test_table]
# A test parameter of a table param
# values: string
# default: empty string
param = ""

# A test parameter of a table param
# values: string
# default: empty string
param2 = ""

"#;

        assert_eq!(expected, table.build_full_table());
    }

    //#[test]
    //fn config_builder_adds_missing_keys_to_table() -> Result<(), Box<dyn Error>> {
    //    let config = ConfigBuilder::with_table_config_params(vec![Box::new(TestTableConfigParam)]);
    //
    //    let existing_file = br#"
    //# The format of the manga downloaded
    //# values: cbz, raw, epub, pdf
    //# default: cbz
    //download_type = "cbz"
    //
    //[test_table]
    //# A test parameter of a table param
    //# values: string
    //# default: empty string
    //param = ""
    //
    //"#;
    //
    //    let mut test_file = Cursor::new(existing_file.to_vec());
    //
    //    config.update_existing_config(&mut test_file)?;
    //
    //    let file_contents = String::from_utf8(test_file.into_inner())?;
    //    let result = toml::Table::from_str(&file_contents)?;
    //
    //    let result = result.get("test_table").unwrap();
    //
    //    match result {
    //        toml::Value::Table(table) => {
    //            assert!(table.contains_key("param"));
    //            assert!(table.contains_key("param2"));
    //        },
    //        _ => panic!("test_table was not a table toml table param but something else"),
    //    }
    //
    //    Ok(())
    //}
}
