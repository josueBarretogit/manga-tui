use std::error::Error;
use std::fmt::Write as FmtWrite;
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
use crate::backend::manga_provider::MangaProviders;
use crate::cli::Credentials;
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

/// Like the `ConfigParam` trait but for making a toml table parameter
trait TableParam {
    fn table_name(&self) -> &'static str;

    fn comments(&self) -> &'static str;

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

    fn param(&self) -> String {
        String::from(r#"check_new_updates = true"#)
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

    fn param(&self) -> String {
        String::from(r#"default_manga_provider = "mangadex""#)
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

    fn param(&self) -> String {
        String::from(r#"client_id = """#)
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

    fn param(&self) -> String {
        format!("{} = {}", self.name(), self.defaults())
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

/// It's main job is to create the config file with the provided config params or update it if it
/// already exists, and also to create the config directory if it does not exist
struct ConfigBuilder<'a> {
    params: Vec<Box<dyn ConfigParam>>,
    table_params: Vec<Box<dyn TableParam>>,
    directory_path: &'a Path,
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
    fn new() -> Self {
        Self {
            table_params: table_config_params(),
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
            table_params: vec![],
            params,
            directory_path: Path::new("./"),
        }
    }

    fn with_table_config_params(table_params: Vec<Box<dyn TableParam>>) -> Self {
        Self {
            table_params,
            params: vec![],
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

        for table_param in &self.table_params {
            file.write_all(table_param.build_full_table().as_bytes())?
        }

        file.flush()?;
        Ok(())
    }

    fn add_missing_table_param(&self, mut config: impl Write + Read) -> Result<(), Box<dyn Error>> {
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
pub struct AnilistConfig {
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MangaTuiConfig {
    pub download_type: DownloadType,
    pub image_quality: ImageQuality,
    pub auto_bookmark: bool,
    pub amount_pages: u8,
    pub track_reading_when_download: bool,
    pub check_new_updates: bool,
    pub default_manga_provider: MangaProviders,
    pub anilist: AnilistConfig,
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

    /// Check wether or not to read the anilist credentials from the config file
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

    #[test]
    fn config_builder_adds_missing_params() -> Result<(), Box<dyn Error>> {
        let config = ConfigBuilder::with_params(vec![Box::new(AmountPagesParam), Box::new(AutoBookmarkParam)])
            .dir_path(CONFIG_TEST_DIRECTORY_PATH);

        let existing_file = br#"
# The format of the manga downloaded
# values: cbz, raw, epub, pdf
# default: cbz
download_type = "cbz"
"#;

        let mut test_file = Cursor::new(existing_file.to_vec());

        config.update_existing_config(&mut test_file)?;

        let file_contents = String::from_utf8(test_file.into_inner())?;
        let expected = toml::Table::from_str(&file_contents)?;

        assert!(expected.contains_key("amount_pages"));
        assert!(expected.contains_key("auto_bookmark"));

        Ok(())
    }

    #[test]
    fn config_builder_adds_missing_table_params_at_the_end() -> Result<(), Box<dyn Error>> {
        let config = ConfigBuilder::with_table_config_params(vec![Box::new(TestTableConfigParam)]);

        let existing_file = br#"
        download_type = "cbz"

        [example_table]
        some_param = 1

        "#;

        let mut test_file = Cursor::new(existing_file.to_vec());

        config.add_missing_table_param(&mut test_file)?;

        let file_contents = String::from_utf8(test_file.into_inner())?;
        let expected = toml::Table::from_str(&file_contents)?;

        let expected = expected.get("test_table").unwrap();

        assert!(expected.is_table());

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
