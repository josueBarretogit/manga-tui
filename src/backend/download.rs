use std::fs::{create_dir, create_dir_all, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use bytes::Bytes;
use epub_builder::{EpubBuilder, EpubContent, ZipLibrary};
use manga_tui::{exists, SanitizedFilename};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

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

#[derive(Debug, Clone)]
pub struct DownloadChapter {
    id_chapter: SanitizedFilename,
    manga_id: SanitizedFilename,
    manga_title: SanitizedFilename,
    chapter_title: SanitizedFilename,
    number: u32,
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
        number: u32,
        scanlator: &'a str,
        lang: &'a str,
    ) -> Self {
        Self {
            id_chapter: SanitizedFilename::new(id_chapter),
            manga_id: SanitizedFilename::new(manga_id),
            manga_title: SanitizedFilename::new(manga_title),
            chapter_title: SanitizedFilename::new(chapter_title),
            number,
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

    pub fn make_raw_images_directory(&'a self, base_directory: &Path) -> Result<PathBuf, std::io::Error> {
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

    pub fn create_cbz(
        &'a self,
        base_directory: &Path,
        images_to_store_in_cbz: Vec<ImageMetada>,
    ) -> Result<PathBuf, std::io::Error> {
        let cbz_filename = format!("{}.cbz", self.make_chapter_file_name());

        let cbz_path = base_directory.join(&cbz_filename);

        let cbz_file = File::create(&cbz_path)?;

        let mut zip = ZipWriter::new(cbz_file);

        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        for (index, image_to_cbz) in images_to_store_in_cbz.into_iter().enumerate() {
            zip.start_file(format!("{}.{}", index, image_to_cbz.extension), options)?;

            zip.write_all(&image_to_cbz.image_bytes)?;
        }

        zip.finish()?;

        Ok(cbz_path)
    }

    pub fn create_epub(
        &'a self,
        base_directory: &Path,
        images_to_store_in_epub: Vec<ImageMetada>,
    ) -> color_eyre::eyre::Result<PathBuf> {
        let epub_path = base_directory.join(format!("{}.epub", self.make_chapter_file_name()));

        let mut epub = File::create(&epub_path)?;

        let mut epub_builder = EpubBuilder::new(ZipLibrary::new()?)?;

        epub_builder.epub_version(epub_builder::EpubVersion::V30);

        epub_builder.metadata("title", self.manga_title.to_string())?;

        for (index, image_data) in images_to_store_in_epub.into_iter().enumerate() {
            let file_name = format!("{}.{}", index, image_data.extension);
            let image_path = format!("data/{}", file_name);

            let mime_type = format!("image/{}", image_data.extension);

            if index == 0 {
                epub_builder.add_cover_image(&image_path, image_data.image_bytes.as_ref(), &mime_type)?;
            }

            epub_builder.add_resource(&image_path, image_data.image_bytes.as_ref(), &mime_type)?;

            let xml_file_path = format!("{}.xhtml", index);

            epub_builder.add_content(EpubContent::new(
                xml_file_path,
                EPUB_FILE_TEMPLATE.replace("REPLACE_IMAGE_SOURCE", &image_path).as_bytes(),
            ))?;
        }

        epub_builder.generate(&mut epub)?;

        Ok(epub_path)
    }

    pub fn get_chapter_id(&self) -> String {
        self.id_chapter.to_string()
    }

    pub fn create_manga_directory(&'a self, base_directory: &Path) -> Result<PathBuf, std::io::Error> {
        let dir_manga_downloads = base_directory.join("mangaDownloads");

        if !exists!(&dir_manga_downloads) {
            create_dir_all(&dir_manga_downloads)?;
        }

        let dir_manga = dir_manga_downloads.join(self.make_manga_directory_filename());

        if !exists!(&dir_manga) {
            create_dir(&dir_manga)?;
        }

        // need directory to store the language the chapter is in
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
    use crate::backend::filter::Languages;

    fn create_tests_directory() -> Result<PathBuf, std::io::Error> {
        let base_directory = Path::new("./test_results");

        if !exists!(&base_directory) {
            fs::create_dir(base_directory)?;
        }

        Ok(base_directory.to_path_buf())
    }

    fn get_chapter_for_testing() -> DownloadChapter {
        DownloadChapter::new(
            &Uuid::new_v4().to_string(),
            &Uuid::new_v4().to_string(),
            &Name().fake::<String>(),
            &Name().fake::<String>(),
            1,
            &Name().fake::<String>(),
            &Languages::default().as_human_readable(),
        )
    }

    /// For creating epub or cbz chapter file
    #[test]
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
    fn make_manga_directory_name() {
        let chapter = get_chapter_for_testing();

        let expected = format!("{} {}", chapter.manga_title, chapter.manga_id);

        let directory_name = chapter.make_manga_directory_filename();

        assert_eq!(expected, directory_name)
    }

    #[test]
    fn make_base_directory_for_manga() -> Result<(), std::io::Error> {
        let chapter_to_download = get_chapter_for_testing();

        let base_directory = create_tests_directory()?;
        let directory_manga_path = chapter_to_download.create_manga_directory(&base_directory)?;

        fs::read_dir(directory_manga_path.as_path())?;

        assert!(directory_manga_path.is_dir());

        let last_folder = directory_manga_path.iter().last().unwrap();

        assert_eq!(Languages::default().as_human_readable().as_str(), last_folder);

        Ok(())
    }

    #[test]
    fn make_raw_images_directory() -> Result<(), std::io::Error> {
        let chapter = get_chapter_for_testing();
        let base_directory = create_tests_directory()?;

        let path = chapter.make_raw_images_directory(&base_directory)?;

        fs::read_dir(&path)?;

        assert_eq!(path, base_directory.join(chapter.make_chapter_file_name()));

        Ok(())
    }

    #[test]
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
    fn make_cbz() -> Result<(), std::io::Error> {
        let chapter = get_chapter_for_testing();

        let base_directory = create_tests_directory()?;

        let images_to_store_in_cbz: Vec<ImageMetada> = vec![
            ImageMetada::new("jpg", include_bytes!("../../data_test/images/1.jpg").to_vec().into()),
            ImageMetada::new("jpg", include_bytes!("../../data_test/images/2.jpg").to_vec().into()),
            ImageMetada::new("jpg", include_bytes!("../../data_test/images/3.jpg").to_vec().into()),
        ];

        let cbz_path = chapter.create_cbz(&base_directory, images_to_store_in_cbz)?;

        assert!(
            cbz_path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .contains(&chapter.make_chapter_file_name())
        );

        let zip_file_created = File::open(&cbz_path)?;

        let mut zip_file_created = zip::ZipArchive::new(zip_file_created)?;

        for file_in_cbz_index in 0..zip_file_created.len() {
            let file = zip_file_created.by_index(file_in_cbz_index)?;
            assert_eq!(format!("{}.jpg", file_in_cbz_index), file.name());
        }

        Ok(())
    }

    #[test]
    fn make_epub() -> color_eyre::eyre::Result<()> {
        let chapter = get_chapter_for_testing();

        let base_directory = create_tests_directory()?;

        let images_to_store_in_epub: Vec<ImageMetada> = vec![
            ImageMetada::new("jpg", include_bytes!("../../data_test/images/1.jpg").to_vec().into()),
            ImageMetada::new("jpg", include_bytes!("../../data_test/images/2.jpg").to_vec().into()),
            ImageMetada::new("jpg", include_bytes!("../../data_test/images/3.jpg").to_vec().into()),
        ];

        let epub_created_path = chapter.create_epub(&base_directory, images_to_store_in_epub)?;

        assert_eq!(epub_created_path.extension().unwrap(), "epub");

        assert!(
            epub_created_path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .contains(&chapter.make_chapter_file_name())
        );

        Ok(())
    }
}
