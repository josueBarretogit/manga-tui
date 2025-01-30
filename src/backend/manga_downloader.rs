use std::error::Error;
use std::fs::{create_dir, create_dir_all, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use bytes::Bytes;
use epub_builder::{EpubBuilder, EpubContent, ZipLibrary};
use manga_tui::{exists, SanitizedFilename};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use super::manga_provider::{ChapterPage, Languages};
use crate::config::DownloadType;

pub mod raw_images;
pub mod zip_downloader;

#[derive(Debug, PartialEq, Clone, Default)]
pub struct ChapterToDownload {
    pub chapter_id: String,
    pub manga_id: String,
    pub manga_title: String,
    pub chapter_title: String,
    pub chapter_number: String,
    pub volume_number: Option<String>,
    pub language: Languages,
    pub scanlator: String,
    pub download_type: DownloadType,
    pub pages: Vec<ChapterPage>,
}

impl ChapterToDownload {
    pub fn new(
        chapter_id: String,
        manga_id: String,
        manga_title: String,
        chapter_title: String,
        chapter_number: String,
        language: Languages,
        scanlator: String,
        download_type: DownloadType,
        volume_number: Option<String>,
        pages: Vec<ChapterPage>,
    ) -> Self {
        Self {
            chapter_id,
            manga_id,
            manga_title,
            chapter_title,
            chapter_number,
            language,
            scanlator,
            download_type,
            volume_number,
            pages,
        }
    }
}

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

impl From<ChapterToDownload> for ChapterToDownloadSanitized {
    fn from(value: ChapterToDownload) -> Self {
        Self {
            chapter_id: value.chapter_id,
            manga_id: value.manga_id,
            manga_title: value.manga_title.into(),
            chapter_title: value.chapter_title.into(),
            chapter_number: value.chapter_number,
            volume_number: value.volume_number,
            language: value.language,
            scanlator: value.scanlator.into(),
            download_type: value.download_type,
            pages: value.pages,
        }
    }
}

/// xml template to build epub files
static EPUB_FILE_TEMPLATE: &str = r#"
                            <?xml version='1.0' encoding='utf-8'?>
                            <!DOCTYPE html>
                            <html xmlns="http://www.w3.org/1999/xhtml">
                              <head>
                                <title>Panel</title>
                                <meta http-equiv="Content-Type" content="text/html; charset=utf-8"/>
                              </head>
                              <body>
                                <div class="centered_image">
                                    <img src="REPLACE_IMAGE_SOURCE" alt="Panel" />
                                </div>
                              </body>
                            </html>
"#;

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
            "Ch {} {} {} {}",
            chapter.chapter_number, chapter.chapter_title, chapter.scanlator, chapter.chapter_id
        ))
    }
    fn save_chapter_in_file_system(self, base_directory: &Path, chapter: ChapterToDownloadSanitized) -> Result<(), Box<dyn Error>>;
}

#[derive(Debug, Clone)]
pub struct DownloadChapter {
    id_chapter: SanitizedFilename,
    manga_id: SanitizedFilename,
    manga_title: SanitizedFilename,
    chapter_title: SanitizedFilename,
    number: String,
    scanlator: SanitizedFilename,
    lang: SanitizedFilename,
}

#[derive(Debug)]
pub struct ImageMetada {
    extension: String,
    image_bytes: Bytes,
}

impl ImageMetada {
    pub fn new(extension: &str, image_bytes: Bytes) -> Self {
        Self {
            extension: extension.to_string(),
            image_bytes,
        }
    }
}

impl<'a> DownloadChapter {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id_chapter: &'a str,
        manga_id: &'a str,
        manga_title: &'a str,
        chapter_title: &'a str,
        number: &'a str,
        scanlator: &'a str,
        lang: &'a str,
    ) -> Self {
        Self {
            id_chapter: SanitizedFilename::new(id_chapter),
            manga_id: SanitizedFilename::new(manga_id),
            manga_title: SanitizedFilename::new(manga_title),
            chapter_title: SanitizedFilename::new(chapter_title),
            number: number.to_string(),
            scanlator: SanitizedFilename::new(scanlator),
            lang: SanitizedFilename::new(lang),
        }
    }

    fn make_chapter_file_name(&'a self) -> String {
        let file_name = format!("Ch. {} {} {} {}", self.number, self.chapter_title, self.scanlator, self.id_chapter);
        file_name
    }

    fn make_manga_directory_filename(&'a self) -> String {
        format!("{} {}", self.manga_title, self.manga_id)
    }

    pub fn make_chapter_directory(&'a self, base_directory: &Path) -> Result<PathBuf, std::io::Error> {
        let directory = base_directory.join(self.make_chapter_file_name());
        if !exists!(&directory) {
            create_dir(&directory)?;
            Ok(directory)
        } else {
            Ok(directory)
        }
    }

    pub fn create_image_file(
        &'a self,
        image_bytes: &[u8],
        base_directory: &Path,
        image_filename: SanitizedFilename,
    ) -> Result<PathBuf, std::io::Error> {
        let image_path = base_directory.join(image_filename.as_path());

        let mut image_created = File::create(&image_path)?;

        image_created.write_all(image_bytes)?;

        Ok(image_path)
    }

    pub fn create_cbz_file(&'a self, base_directory: &Path) -> Result<(ZipWriter<File>, PathBuf), std::io::Error> {
        let cbz_filename = format!("{}.cbz", self.make_chapter_file_name());

        let cbz_path = base_directory.join(&cbz_filename);

        let cbz_file = File::create(&cbz_path)?;

        let zip = ZipWriter::new(cbz_file);

        Ok((zip, cbz_path))
    }

    pub fn insert_into_cbz(&'a self, zip_writer: &mut ZipWriter<File>, file_name: &'a str, image_bytes: &[u8]) {
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        zip_writer.start_file(file_name, options).ok();

        zip_writer.write_all(image_bytes).ok();
    }

    pub fn create_epub_file(&'a self, base_directory: &Path) -> color_eyre::eyre::Result<(EpubBuilder<ZipLibrary>, File, PathBuf)> {
        let epub_path = base_directory.join(format!("{}.epub", self.make_chapter_file_name()));

        let epub_file = File::create(&epub_path)?;

        let mut epub_builder = EpubBuilder::new(ZipLibrary::new()?)?;

        epub_builder.epub_version(epub_builder::EpubVersion::V30);

        epub_builder.metadata("title", self.manga_title.to_string()).ok();

        Ok((epub_builder, epub_file, epub_path))
    }

    pub fn insert_into_epub(
        &'a self,
        epub_builder: &mut EpubBuilder<ZipLibrary>,
        file_name: &'a str,
        extension: &'a str,
        index: usize,
        image_bytes: &[u8],
    ) {
        let image_path = format!("data/{}", file_name);

        let mime_type = format!("image/{}", extension);

        if index == 0 {
            epub_builder.add_cover_image(&image_path, image_bytes, &mime_type).ok();
        }

        epub_builder.add_resource(&image_path, image_bytes, &mime_type).ok();

        let xml_file_path = format!("{}.xhtml", index);

        epub_builder
            .add_content(EpubContent::new(
                xml_file_path,
                EPUB_FILE_TEMPLATE.replace("REPLACE_IMAGE_SOURCE", &image_path).as_bytes(),
            ))
            .ok();
    }

    pub fn make_base_manga_directory(&'a self, base_directory: &Path) -> Result<PathBuf, std::io::Error> {
        let dir_manga = base_directory.join(self.make_manga_directory_filename());

        if !exists!(&dir_manga) {
            create_dir_all(&dir_manga)?;
        }

        let chapter_language_dir = dir_manga.join(self.lang.as_path());

        if !exists!(&chapter_language_dir) {
            create_dir(&chapter_language_dir)?;
        }

        Ok(chapter_language_dir)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use fake::faker::name::en::Name;
    use fake::Fake;
    use pretty_assertions::assert_eq;
    use uuid::Uuid;

    use super::*;
    use crate::backend::manga_provider::Languages;

    fn create_tests_directory() -> Result<PathBuf, std::io::Error> {
        let base_directory = Path::new("./test_results/download");

        if !exists!(&base_directory) {
            fs::create_dir_all(base_directory)?;
        }

        Ok(base_directory.to_path_buf())
    }

    fn get_chapter_for_testing() -> DownloadChapter {
        DownloadChapter::new(
            &Uuid::new_v4().to_string(),
            &Uuid::new_v4().to_string(),
            &Name().fake::<String>(),
            &Name().fake::<String>(),
            "1",
            &Name().fake::<String>(),
            &Languages::default().as_human_readable(),
        )
    }

    /// For creating epub or cbz chapter file
    #[test]
    #[ignore]
    fn make_chapter_file_name() {
        let chapter_to_download = get_chapter_for_testing();

        let chapter_name = chapter_to_download.make_chapter_file_name();

        let expected_chapter_name = format!(
            "Ch. {} {} {} {}",
            chapter_to_download.number,
            chapter_to_download.chapter_title,
            chapter_to_download.scanlator,
            chapter_to_download.id_chapter
        );

        assert_eq!(expected_chapter_name, chapter_name)
    }

    #[test]
    #[ignore]
    fn make_manga_directory_name() {
        let chapter = get_chapter_for_testing();

        let expected = format!("{} {}", chapter.manga_title, chapter.manga_id);

        let directory_name = chapter.make_manga_directory_filename();

        assert_eq!(expected, directory_name)
    }

    #[test]
    #[ignore]
    fn make_base_directory_for_manga() -> Result<(), std::io::Error> {
        let chapter_to_download = get_chapter_for_testing();

        let base_directory = create_tests_directory()?;

        let directory_manga_path = chapter_to_download.make_base_manga_directory(&base_directory)?;

        assert!(directory_manga_path.is_dir());

        assert!(directory_manga_path.starts_with(&base_directory));

        let manga_base_directory = base_directory.join(chapter_to_download.make_manga_directory_filename());

        assert!(directory_manga_path.starts_with(&manga_base_directory));

        let language_directory = manga_base_directory.join(Languages::default().as_human_readable());

        assert!(directory_manga_path.starts_with(language_directory));

        Ok(())
    }

    #[test]
    #[ignore]
    fn make_raw_images_directory() -> Result<(), std::io::Error> {
        let chapter = get_chapter_for_testing();
        let base_directory = create_tests_directory()?;

        let path = chapter.make_chapter_directory(&base_directory)?;

        fs::read_dir(&path)?;

        assert_eq!(path, base_directory.join(chapter.make_chapter_file_name()));

        Ok(())
    }

    #[test]
    #[ignore]
    fn make_image_file() -> Result<(), std::io::Error> {
        let chapter = get_chapter_for_testing();
        let base_directory = create_tests_directory()?;

        let image_sample = include_bytes!("../../public/mangadex_support.jpg").to_vec();
        let image_name = format!("{}.jpg", Uuid::new_v4());

        let image_path = chapter.create_image_file(&image_sample, &base_directory, SanitizedFilename::new(image_name))?;

        let file = fs::read(&image_path)?;

        assert_eq!(image_sample, file);

        fs::remove_file(image_path)?;

        Ok(())
    }

    #[test]
    #[ignore]
    fn create_cbz_file() -> Result<(), std::io::Error> {
        let chapter = get_chapter_for_testing();

        let (mut zip, cbz_path) = chapter.create_cbz_file(&create_tests_directory()?)?;

        assert_eq!(format!("{}.cbz", chapter.make_chapter_file_name()).as_str(), cbz_path.file_name().unwrap());

        chapter.insert_into_cbz(&mut zip, "create_cbz1.jpg", include_bytes!("../../data_test/images/1.jpg"));
        chapter.insert_into_cbz(&mut zip, "create_cbz2.jpg", include_bytes!("../../data_test/images/2.jpg"));

        zip.finish()?;

        let zip_file_created = File::open(&cbz_path)?;

        let mut zip_file_created = zip::ZipArchive::new(zip_file_created)?;

        for file_in_cbz_index in 0..zip_file_created.len() {
            let file = zip_file_created.by_index(file_in_cbz_index)?;
            assert_eq!(format!("create_cbz{}.jpg", file_in_cbz_index + 1), file.name());
        }

        Ok(())
    }

    #[test]
    #[ignore]
    fn create_epub_file() -> color_eyre::eyre::Result<()> {
        let chapter = get_chapter_for_testing();

        let base_directory = create_tests_directory()?;

        let (mut epub_builder, mut file, epub_path) = chapter.create_epub_file(&base_directory)?;

        chapter.insert_into_epub(&mut epub_builder, "test.jpg", "jpg", 0, include_bytes!("../../data_test/images/1.jpg"));
        chapter.insert_into_epub(&mut epub_builder, "test2.jpg", "jpg", 1, include_bytes!("../../data_test/images/2.jpg"));

        epub_builder.generate(&mut file)?;

        fs::File::open(&epub_path)?;

        let expected_epub_name = format!("{}.epub", chapter.make_chapter_file_name());

        assert_eq!(expected_epub_name.as_str(), epub_path.file_name().unwrap());

        Ok(())
    }
}
