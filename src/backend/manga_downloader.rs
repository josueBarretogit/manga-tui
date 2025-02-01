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
    /// The `base_directory` where the pages will be saved, for `raw_images`
    fn make_manga_base_directory_name(&self, base_directory: &Path, chapter: &ChapterToDownloadSanitized) -> PathBuf {
        let base_directory = base_directory
            .join(format!("{} {}", chapter.manga_title, chapter.manga_id))
            .join(chapter.language.as_human_readable());

        PathBuf::from(base_directory)
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
    //use std::fs;
    //
    //use fake::faker::name::en::Name;
    //use fake::Fake;
    //use pretty_assertions::assert_eq;
    //use uuid::Uuid;
    //
    //use super::*;
    //use crate::backend::manga_provider::Languages;
    //
    //fn create_tests_directory() -> Result<PathBuf, std::io::Error> {
    //    let base_directory = Path::new("./test_results/download");
    //
    //    if !exists!(&base_directory) {
    //        fs::create_dir_all(base_directory)?;
    //    }
    //
    //    Ok(base_directory.to_path_buf())
    //}
    //
    //fn get_chapter_for_testing() -> DownloadChapter {
    //    DownloadChapter::new(
    //        &Uuid::new_v4().to_string(),
    //        &Uuid::new_v4().to_string(),
    //        &Name().fake::<String>(),
    //        &Name().fake::<String>(),
    //        "1",
    //        &Name().fake::<String>(),
    //        &Languages::default().as_human_readable(),
    //    )
    //}
    //
    ///// For creating epub or cbz chapter file
    //#[test]
    //#[ignore]
    //fn make_chapter_file_name() {
    //    let chapter_to_download = get_chapter_for_testing();
    //
    //    let chapter_name = chapter_to_download.make_chapter_file_name();
    //
    //    let expected_chapter_name = format!(
    //        "Ch. {} {} {} {}",
    //        chapter_to_download.number,
    //        chapter_to_download.chapter_title,
    //        chapter_to_download.scanlator,
    //        chapter_to_download.id_chapter
    //    );
    //
    //    assert_eq!(expected_chapter_name, chapter_name)
    //}
    //
    //#[test]
    //#[ignore]
    //fn make_manga_directory_name() {
    //    let chapter = get_chapter_for_testing();
    //
    //    let expected = format!("{} {}", chapter.manga_title, chapter.manga_id);
    //
    //    let directory_name = chapter.make_manga_directory_filename();
    //
    //    assert_eq!(expected, directory_name)
    //}
    //
    //#[test]
    //#[ignore]
    //fn make_base_directory_for_manga() -> Result<(), std::io::Error> {
    //    let chapter_to_download = get_chapter_for_testing();
    //
    //    let base_directory = create_tests_directory()?;
    //
    //    let directory_manga_path = chapter_to_download.make_base_manga_directory(&base_directory)?;
    //
    //    assert!(directory_manga_path.is_dir());
    //
    //    assert!(directory_manga_path.starts_with(&base_directory));
    //
    //    let manga_base_directory = base_directory.join(chapter_to_download.make_manga_directory_filename());
    //
    //    assert!(directory_manga_path.starts_with(&manga_base_directory));
    //
    //    let language_directory = manga_base_directory.join(Languages::default().as_human_readable());
    //
    //    assert!(directory_manga_path.starts_with(language_directory));
    //
    //    Ok(())
    //}
    //
    //#[test]
    //#[ignore]
    //fn make_raw_images_directory() -> Result<(), std::io::Error> {
    //    let chapter = get_chapter_for_testing();
    //    let base_directory = create_tests_directory()?;
    //
    //    let path = chapter.make_chapter_directory(&base_directory)?;
    //
    //    fs::read_dir(&path)?;
    //
    //    assert_eq!(path, base_directory.join(chapter.make_chapter_file_name()));
    //
    //    Ok(())
    //}
    //
    //#[test]
    //#[ignore]
    //fn make_image_file() -> Result<(), std::io::Error> {
    //    let chapter = get_chapter_for_testing();
    //    let base_directory = create_tests_directory()?;
    //
    //    let image_sample = include_bytes!("../../public/mangadex_support.jpg").to_vec();
    //    let image_name = format!("{}.jpg", Uuid::new_v4());
    //
    //    let image_path = chapter.create_image_file(&image_sample, &base_directory, SanitizedFilename::new(image_name))?;
    //
    //    let file = fs::read(&image_path)?;
    //
    //    assert_eq!(image_sample, file);
    //
    //    fs::remove_file(image_path)?;
    //
    //    Ok(())
    //}
    //
    //#[test]
    //#[ignore]
    //fn create_cbz_file() -> Result<(), std::io::Error> {
    //    let chapter = get_chapter_for_testing();
    //
    //    let (mut zip, cbz_path) = chapter.create_cbz_file(&create_tests_directory()?)?;
    //
    //    assert_eq!(format!("{}.cbz", chapter.make_chapter_file_name()).as_str(), cbz_path.file_name().unwrap());
    //
    //    chapter.insert_into_cbz(&mut zip, "create_cbz1.jpg", include_bytes!("../../data_test/images/1.jpg"));
    //    chapter.insert_into_cbz(&mut zip, "create_cbz2.jpg", include_bytes!("../../data_test/images/2.jpg"));
    //
    //    zip.finish()?;
    //
    //    let zip_file_created = File::open(&cbz_path)?;
    //
    //    let mut zip_file_created = zip::ZipArchive::new(zip_file_created)?;
    //
    //    for file_in_cbz_index in 0..zip_file_created.len() {
    //        let file = zip_file_created.by_index(file_in_cbz_index)?;
    //        assert_eq!(format!("create_cbz{}.jpg", file_in_cbz_index + 1), file.name());
    //    }
    //
    //    Ok(())
    //}
    //
    //#[test]
    //#[ignore]
    //fn create_epub_file() -> color_eyre::eyre::Result<()> {
    //    let chapter = get_chapter_for_testing();
    //
    //    let base_directory = create_tests_directory()?;
    //
    //    let (mut epub_builder, mut file, epub_path) = chapter.create_epub_file(&base_directory)?;
    //
    //    chapter.insert_into_epub(&mut epub_builder, "test.jpg", "jpg", 0, include_bytes!("../../data_test/images/1.jpg"));
    //    chapter.insert_into_epub(&mut epub_builder, "test2.jpg", "jpg", 1, include_bytes!("../../data_test/images/2.jpg"));
    //
    //    epub_builder.generate(&mut file)?;
    //
    //    fs::File::open(&epub_path)?;
    //
    //    let expected_epub_name = format!("{}.epub", chapter.make_chapter_file_name());
    //
    //    assert_eq!(expected_epub_name.as_str(), epub_path.file_name().unwrap());
    //
    //    Ok(())
    //}
}
