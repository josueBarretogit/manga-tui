use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::backend::error_log::{ErrorType, write_to_error_log};
use crate::backend::manga_provider::mangadex::filters::api_parameter::Filters;

static MANGADEX_FILTERS_CACHE_FILE_NAME: &str = "filters.toml";

pub struct FiltersCache<'a> {
    base_directory: &'a Path,
}

impl<'a> FiltersCache<'a> {
    pub fn new(base_directory: &'a Path) -> Self {
        Self { base_directory }
    }

    fn save_filters<T: Write>(&self, filters: &Filters, file: &mut T) -> Result<(), Box<dyn Error>> {
        let filters_as_toml = toml::to_string(filters)?;

        file.write_all(filters_as_toml.as_bytes())?;

        file.flush()?;

        Ok(())
    }

    #[inline]
    fn get_cache_file_path(&'a self) -> PathBuf {
        self.base_directory.join(MANGADEX_FILTERS_CACHE_FILE_NAME)
    }

    fn parse_cache<T: Read>(&self, file: &mut T) -> Result<Filters, Box<dyn Error>> {
        let mut contents = String::new();

        file.read_to_string(&mut contents)?;

        let filters: Filters = toml::from_str(&contents)?;

        Ok(filters)
    }

    pub fn get_cached_filters(self) -> Result<Option<Filters>, Box<dyn Error>> {
        let file_path = self.get_cache_file_path();

        let maybe_filters = File::open(file_path)
            .inspect_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => {},
                _ => write_to_error_log(ErrorType::String(&e.to_string())),
            })
            .ok()
            .map(|mut file| self.parse_cache(&mut file))
            .transpose();

        maybe_filters
    }

    pub fn write_to_cache(self, filters: &Filters) -> Result<(), Box<dyn Error>> {
        let file_path = self.get_cache_file_path();

        let mut file = File::create(dbg!(file_path))?;

        self.save_filters(filters, &mut file)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fs::create_dir_all;
    use std::io::Cursor;

    use pretty_assertions::assert_eq;
    use tokio::fs;

    use super::*;
    use crate::backend::manga_provider::Languages;
    use crate::backend::manga_provider::mangadex::filters::api_parameter::{
        AuthorFilterState, ContentRating, Filters, MagazineDemographic, PublicationStatus, SortBy, TagData, TagSelection, Tags,
        User,
    };

    const CACHE_TEST_DIRECTORY_PATH: &str = "./test_results/cache_test/";

    #[test]
    fn it_writes_filters_to_cache_file() -> Result<(), Box<dyn Error>> {
        let filters: Filters = Filters {
            content_rating: vec![ContentRating::Suggestive, ContentRating::Erotic],
            publication_status: vec![PublicationStatus::Completed, PublicationStatus::Ongoing],
            sort_by: SortBy::HighestRating,
            tags: Tags::new(vec![TagData::new("id_tag".to_string(), TagSelection::Included, "fantasy".to_string())]),
            magazine_demographic: vec![MagazineDemographic::Shoujo, MagazineDemographic::Seinen],
            authors: User::new(vec![AuthorFilterState::new("user_id".to_string())]),
            artists: User::default(),
            languages: vec![Languages::English, Languages::Spanish],
        };

        let mut test_file = Cursor::new(Vec::new());

        let filters_cache = FiltersCache {
            base_directory: Path::new(""),
        };

        filters_cache.save_filters(&filters, &mut test_file)?;

        let contents = String::from_utf8(test_file.into_inner())?;

        let result: Filters = toml::from_str(&contents)?;

        assert_eq!(filters, result);

        Ok(())
    }

    #[test]
    fn it_parses_the_cache_from_file() -> Result<(), Box<dyn Error>> {
        let filters: Filters = Filters {
            content_rating: vec![ContentRating::Suggestive, ContentRating::Erotic],
            publication_status: vec![PublicationStatus::Completed, PublicationStatus::Ongoing],
            sort_by: SortBy::HighestRating,
            tags: Tags::new(vec![TagData::new("id_tag".to_string(), TagSelection::Included, "fantasy".to_string())]),
            magazine_demographic: vec![MagazineDemographic::Shoujo, MagazineDemographic::Seinen],
            authors: User::new(vec![AuthorFilterState::new("user_id".to_string())]),
            artists: User::default(),
            languages: vec![Languages::English, Languages::Spanish],
        };

        let mut test_file = Cursor::new(toml::to_string(&filters)?);

        let filters_cache = FiltersCache {
            base_directory: Path::new(""),
        };

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
    fn it_check_if_cache_file_exists() -> Result<(), Box<dyn Error>> {
        let filters: Filters = Filters {
            content_rating: vec![ContentRating::Suggestive, ContentRating::Erotic],
            publication_status: vec![PublicationStatus::Completed, PublicationStatus::Ongoing],
            sort_by: SortBy::HighestRating,
            tags: Tags::new(vec![TagData::new("id_tag".to_string(), TagSelection::Included, "fantasy".to_string())]),
            magazine_demographic: vec![MagazineDemographic::Shoujo, MagazineDemographic::Seinen],
            authors: User::new(vec![AuthorFilterState::new("user_id".to_string())]),
            artists: User::default(),
            languages: vec![Languages::English, Languages::Spanish],
        };

        create_dir_all(CACHE_TEST_DIRECTORY_PATH)?;

        let file_cache = FiltersCache::new(CACHE_TEST_DIRECTORY_PATH.as_ref());

        delete_cached_file_if_already_exists(&file_cache.get_cache_file_path());

        let first_check = file_cache.get_cached_filters()?;

        assert!(first_check.is_none());

        let file_cache = FiltersCache::new(CACHE_TEST_DIRECTORY_PATH.as_ref());

        file_cache.write_to_cache(&filters).expect("failed to create cache file");

        let file_cache = FiltersCache::new(CACHE_TEST_DIRECTORY_PATH.as_ref());

        let second_check = file_cache.get_cached_filters()?;

        assert!(second_check.is_some());

        dbg!(second_check);

        Ok(())
    }
}
