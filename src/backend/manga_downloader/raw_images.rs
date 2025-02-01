use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use super::MangaDownloader;

#[derive(Debug)]
pub struct RawImagesDownloader {}

impl RawImagesDownloader {
    pub fn new() -> Self {
        Self {}
    }
}

impl MangaDownloader for RawImagesDownloader {
    /// Overwriting the default implementation in order to make the chapter the `base_directory`
    fn make_manga_base_directory_name(
        &self,
        base_directory: &std::path::Path,
        chapter: &super::ChapterToDownloadSanitized,
    ) -> PathBuf {
        let base_directory = base_directory
            .join(format!("{} {}", chapter.manga_title, chapter.manga_id))
            .join(chapter.language.as_human_readable())
            .join(self.make_chapter_name(chapter));

        PathBuf::from(base_directory)
    }

    fn save_chapter_in_file_system(
        &self,
        base_directory: &std::path::Path,
        chapter: super::ChapterToDownloadSanitized,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let base_directory = self.make_manga_base_directory_name(base_directory, &chapter);

        self.create_manga_base_directory(&base_directory)?;

        for (index, chap) in chapter.pages.into_iter().enumerate() {
            let file_name = base_directory.join(&format!("{}.{}", index + 1, chap.extension));
            let mut maybe_file = File::create(file_name);

            if let Ok(file) = maybe_file.as_mut() {
                file.write_all(&chap.bytes).ok();
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::path::Path;

    use super::*;
    use crate::backend::manga_downloader::ChapterToDownloadSanitized;
    use crate::backend::manga_provider::Languages;

    #[test]
    fn it_creates_base_directory_raw_images() -> Result<(), Box<dyn Error>> {
        let raw_images_downloader = RawImagesDownloader {};

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

        let expected = Path::new("./test/some manga title manga id/English/Ch 3 some chapter title chapter id");
        let result = raw_images_downloader.make_manga_base_directory_name(Path::new("./test"), &test_chapter);

        assert_eq!(expected, result);
        Ok(())
    }
}
