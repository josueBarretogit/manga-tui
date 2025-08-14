//! This module provides the `FiltersCache` struct, which is responsible for caching and retrieving filter data used in manga search
//! operations.
//!
//! The `FiltersCache` allows you to serialize filter configurations (such as languages, publication status, sort order, tags,
//! authors, and more) into TOML files for persistent storage, and deserialize them back when needed. This is useful for persisting
//! user-selected filters or default filter sets between application runs.
//!
//! The cache is stored in a specified directory and file, and the module provides methods to write filter data to the cache and
//! read it back. The filter data must implement `serde::Serialize` and `serde::de::DeserializeOwned`, making it flexible for
//! various filter types.
//!
//! Example use cases include caching search filters for manga providers like MangaDex, where filters may include fields such as
//! languages, publication status, sort order, tags, magazine demographics, authors, and artists.
use std::error::Error;
use std::fs::{File, create_dir_all};
use std::io::{Read, Write};
use std::path::PathBuf;

use serde::Serialize;
use serde::de::DeserializeOwned;

/// A cache handler for serializing and deserializing filter data to and from TOML files.
///
/// `FiltersCache` is designed to persist filter configurations used in manga search operations, such as those for MangaDex.
/// It stores filter data (implementing `serde::Serialize` and `serde::de::DeserializeOwned`) in a specified directory and file.
///
/// # Example Usage
///
/// The struct is typically used to cache filters like the following (see tests for more details):
///
/// ```rust
/// # use crate::backend::manga_provider::mangadex::filters::api_parameter::{Filters, ContentRating, PublicationStatus, SortBy, Tags, TagData, TagSelection, MagazineDemographic, User, AuthorFilterState};
/// # use crate::backend::manga_provider::Languages;
/// let filters = Filters {
///     content_rating: vec![ContentRating::Suggestive, ContentRating::Erotic],
///     publication_status: vec![PublicationStatus::Completed, PublicationStatus::Ongoing],
///     sort_by: SortBy::HighestRating,
///     tags: Tags::new(vec![TagData::new("id_tag".to_string(), TagSelection::Included, "fantasy".to_string())]),
///     magazine_demographic: vec![MagazineDemographic::Shoujo, MagazineDemographic::Seinen],
///     authors: User::new(vec![AuthorFilterState::new("user_id".to_string(), "".to_string())]),
///     artists: User::default(),
///     languages: vec![Languages::English, Languages::Spanish],
/// };
/// ```
///
/// You can then write these filters to a cache file and retrieve them later:
///
/// ```rust
/// # use std::path::Path;
/// # let filters_cache = FiltersCache::new(Path::new("./cache_dir"), "filters.toml");
/// filters_cache.write_to_cache(&filters).unwrap();
/// let cached: Option<Filters> = filters_cache.get_cached_filters();
/// ```
///
/// This enables persistent storage and retrieval of user or default filter sets between application runs.
pub struct FiltersCache {
    base_directory: PathBuf,
    cache_filename: &'static str,
}

impl FiltersCache {
    pub fn new<T: Into<PathBuf>>(base_directory: T, cache_filename: &'static str) -> Self {
        let path: PathBuf = base_directory.into();
        Self {
            base_directory: path,
            cache_filename,
        }
    }

    fn save_filters<T: Write, I: Serialize>(&self, filters: &I, file: &mut T) -> Result<(), Box<dyn Error>> {
        let filters_as_toml = toml::to_string(filters)?;

        file.write_all(filters_as_toml.as_bytes())?;

        file.flush()?;

        Ok(())
    }

    #[inline]
    fn get_cache_file_path(&self) -> PathBuf {
        self.base_directory.join(self.cache_filename)
    }

    fn parse_cache<T: Read, I: DeserializeOwned>(&self, file: &mut T) -> Result<I, Box<dyn Error>> {
        let mut contents = String::new();

        file.read_to_string(&mut contents)?;

        let filters: I = toml::from_str(&contents)?;

        Ok(filters)
    }

    fn ensure_cache_directory_exists(&self) -> Result<(), std::io::Error> {
        if !self.base_directory.exists() {
            create_dir_all(&self.base_directory)?
        }

        Ok(())
    }

    /// Reads the cache directory, and returns:
    /// Some(filters) if there is already a cache filters file,
    /// None if the file doesnt exist
    pub fn get_cached_filters<I: DeserializeOwned>(&self) -> Option<I> {
        let file_path = self.get_cache_file_path();

        let maybe_filters = File::open(file_path)
            .inspect_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => {},
                _ => {
                    #[cfg(not(test))]
                    {
                        use crate::backend::error_log::{ErrorType, write_to_error_log};

                        write_to_error_log(ErrorType::String(&e.to_string()))
                    }
                },
            })
            .and_then(|mut file| self.parse_cache(&mut file).map_err(|e| std::io::Error::other(e.to_string())))
            .ok();

        maybe_filters
    }

    /// Writes the "Filters" to the cache file which is created if it
    /// doesnt exist in toml format
    pub fn write_to_cache<I: Serialize>(&self, filters: &I) -> Result<(), Box<dyn Error>> {
        let file_path = self.get_cache_file_path();

        self.ensure_cache_directory_exists()?;

        let mut file = File::create(file_path)?;

        self.save_filters(filters, &mut file)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fs::create_dir_all;
    use std::io::Cursor;
    use std::path::Path;

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::backend::manga_provider::Languages;
    use crate::backend::manga_provider::mangadex::filters::api_parameter::{
        AuthorFilterState, ContentRating, Filters, MagazineDemographic, PublicationStatus, SortBy, TagData, TagSelection, Tags,
        User,
    };

    const CACHE_TEST_DIRECTORY_PATH: &str = "./test_results/cache_test/";

    #[test]
    fn it_writes_mangadex_filters_to_cache_file() -> Result<(), Box<dyn Error>> {
        let filters: Filters = Filters {
            content_rating: vec![ContentRating::Suggestive, ContentRating::Erotic],
            publication_status: vec![PublicationStatus::Completed, PublicationStatus::Ongoing],
            sort_by: SortBy::HighestRating,
            tags: Tags::new(vec![TagData::new("id_tag".to_string(), TagSelection::Included, "fantasy".to_string())]),
            magazine_demographic: vec![MagazineDemographic::Shoujo, MagazineDemographic::Seinen],
            authors: User::new(vec![AuthorFilterState::new("user_id".to_string(), "".to_string())]),
            artists: User::default(),
            languages: vec![Languages::English, Languages::Spanish],
        };

        let mut test_file = Cursor::new(Vec::new());

        let filters_cache = FiltersCache::new(Path::new(""), "");

        filters_cache.save_filters(&filters, &mut test_file)?;

        let contents = String::from_utf8(test_file.into_inner())?;

        let result: Filters = toml::from_str(&contents)?;

        assert_eq!(filters, result);

        Ok(())
    }

    #[test]
    fn it_parses_mangadex_filters_from_the_cache_from_file() -> Result<(), Box<dyn Error>> {
        let filters: Filters = Filters {
            content_rating: vec![ContentRating::Suggestive, ContentRating::Erotic],
            publication_status: vec![PublicationStatus::Completed, PublicationStatus::Ongoing],
            sort_by: SortBy::HighestRating,
            tags: Tags::new(vec![TagData::new("id_tag".to_string(), TagSelection::Included, "fantasy".to_string())]),
            magazine_demographic: vec![MagazineDemographic::Shoujo, MagazineDemographic::Seinen],
            authors: User::new(vec![AuthorFilterState::new("user_id".to_string(), "".to_string())]),
            artists: User::default(),
            languages: vec![Languages::English, Languages::Spanish],
        };

        let mut test_file = Cursor::new(toml::to_string(&filters)?);

        let filters_cache = FiltersCache::new(Path::new(""), "");

        let cached = filters_cache.parse_cache(&mut test_file)?;

        assert_eq!(filters, cached);

        Ok(())
    }

    fn delete_cached_file_if_already_exists(path: &Path) {
        if path.exists() {
            std::fs::remove_file(path).unwrap()
        }
    }

    #[ignore]
    #[test]
    fn it_check_if_cache_file_exists_and_returns_none() -> Result<(), Box<dyn Error>> {
        let filters: Filters = Filters {
            content_rating: vec![ContentRating::Suggestive, ContentRating::Erotic],
            publication_status: vec![PublicationStatus::Completed, PublicationStatus::Ongoing],
            sort_by: SortBy::HighestRating,
            tags: Tags::new(vec![TagData::new("id_tag".to_string(), TagSelection::Included, "fantasy".to_string())]),
            magazine_demographic: vec![MagazineDemographic::Shoujo, MagazineDemographic::Seinen],
            authors: User::new(vec![AuthorFilterState::new("user_id".to_string(), "".to_string())]),
            artists: User::default(),
            languages: vec![Languages::English, Languages::Spanish],
        };

        create_dir_all(CACHE_TEST_DIRECTORY_PATH)?;

        let file_cache = FiltersCache::new(CACHE_TEST_DIRECTORY_PATH, "mangadex_filters.toml");

        delete_cached_file_if_already_exists(&file_cache.get_cache_file_path());

        let first_check: Option<Filters> = file_cache.get_cached_filters();

        assert!(first_check.is_none());

        file_cache.write_to_cache(&filters).expect("failed to create cache file");

        let second_check: Option<Filters> = file_cache.get_cached_filters();

        assert!(second_check.is_some());

        dbg!(second_check);

        Ok(())
    }
}
