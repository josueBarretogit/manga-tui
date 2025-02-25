use std::error::Error;
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};

use manga_tui::{exists, SanitizedFilename};

use super::manga_provider::{ChapterPage, Languages};
use crate::config::DownloadType;

pub mod cbz_downloader;
pub mod epub_downloader;
pub mod raw_images;

/// This struct represents a chapter with its data not having characteres that may throw errors
/// when creating files such as `/` or `\`
#[derive(Debug, PartialEq, Clone, Default)]
pub struct ChapterToDownloadSanitized {
    pub chapter_id: String,
    pub manga_id: String,
    pub manga_title: SanitizedFilename,
    pub chapter_title: SanitizedFilename,
    pub chapter_number: String,
    pub volume_number: Option<String>,
    pub language: Languages,
    pub scanlator: SanitizedFilename,
    pub download_type: DownloadType,
    pub pages: Vec<ChapterPage>,
}

pub trait MangaDownloader {
    /// Construct the manga directory name which should look like this: New World Builders ~ Survive With Class 24 “Body”
    /// manga-yd1002286
    fn make_manga_base_directory_name(&self, base_directory: &Path, chapter: &ChapterToDownloadSanitized) -> PathBuf {
        base_directory
            .join(format!("{} {}", chapter.manga_title, chapter.manga_id))
            .join(chapter.language.as_human_readable())
    }

    fn create_manga_base_directory(&self, base_directory: &Path) -> Result<(), Box<dyn Error>> {
        if !exists!(base_directory) {
            create_dir_all(base_directory)?
        }
        Ok(())
    }

    fn make_chapter_name(&self, chapter: &ChapterToDownloadSanitized) -> PathBuf {
        PathBuf::from(format!(
            "Ch {} Vol {} {} {} {}",
            chapter.chapter_number,
            chapter.volume_number.as_ref().cloned().unwrap_or("none".to_string()),
            chapter.chapter_title,
            chapter.scanlator,
            chapter.chapter_id
        ))
    }

    fn save_chapter_in_file_system(&self, base_directory: &Path, chapter: ChapterToDownloadSanitized)
    -> Result<(), Box<dyn Error>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct MockMangaDownloader {}

    impl MangaDownloader for MockMangaDownloader {
        fn save_chapter_in_file_system(
            &self,
            _base_directory: &Path,
            _chapter: ChapterToDownloadSanitized,
        ) -> Result<(), Box<dyn Error>> {
            Ok(())
        }
    }

    #[test]
    fn default_implementation_makes_manga_directory_name() {
        let downloader = MockMangaDownloader {};

        let test_chapter: ChapterToDownloadSanitized = ChapterToDownloadSanitized {
            chapter_id: "chapter id".to_string(),
            manga_id: "manga id".to_string(),
            manga_title: "some manga title".to_string().into(),
            chapter_title: "some chapter title".to_string().into(),
            chapter_number: "3".to_string(),
            volume_number: None,
            language: Languages::default(),
            scanlator: "".to_string().into(),
            download_type: crate::config::DownloadType::Cbz,
            pages: vec![],
        };

        let expected = Path::new("./test/some manga title manga id/English");
        let result = downloader.make_manga_base_directory_name(Path::new("./test"), &test_chapter);

        assert_eq!(expected, result);
    }

    #[test]
    fn default_implementation_make_chapter_name() {
        let downloader = MockMangaDownloader {};

        let test_chapter: ChapterToDownloadSanitized = ChapterToDownloadSanitized {
            chapter_id: "chapter id".to_string(),
            manga_id: "manga id".to_string(),
            manga_title: "some manga title".to_string().into(),
            chapter_title: "some chapter title".to_string().into(),
            chapter_number: "3".to_string(),
            volume_number: Some(3.to_string()),
            language: Languages::default(),
            scanlator: "some scanlator".to_string().into(),
            download_type: crate::config::DownloadType::Cbz,
            pages: vec![],
        };

        let expected = Path::new("Ch 3 Vol 3 some chapter title some scanlator chapter id");
        let result = downloader.make_chapter_name(&test_chapter);

        assert_eq!(expected, result);
    }
}
