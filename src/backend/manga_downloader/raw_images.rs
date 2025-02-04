use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use super::MangaDownloader;

#[derive(Debug, Clone)]
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

    use fake::faker::name::en::Name;
    use fake::Fake;
    use uuid::Uuid;

    use super::*;
    use crate::backend::manga_downloader::ChapterToDownloadSanitized;
    use crate::backend::manga_provider::{ChapterPage, Languages};
    use crate::backend::AppDirectories;
    use crate::config::DownloadType;

    #[test]
    #[ignore]
    fn it_downloads_a_chapter() -> Result<(), Box<dyn Error>> {
        let chapter: ChapterToDownloadSanitized = ChapterToDownloadSanitized {
            chapter_id: Uuid::new_v4().to_string(),
            manga_id: Uuid::new_v4().to_string(),
            manga_title: Name().fake::<String>().into(),
            chapter_title: Name().fake::<String>().into(),
            chapter_number: "2".to_string(),
            volume_number: Some("3".to_string()),
            language: Languages::default(),
            scanlator: Name().fake::<String>().into(),
            download_type: DownloadType::Cbz,
            pages: vec![
                ChapterPage {
                    bytes: include_bytes!("../../../data_test/images/1.jpg").to_vec().into(),
                    extension: "jpg".to_string(),
                },
                ChapterPage {
                    bytes: include_bytes!("../../../data_test/images/2.jpg").to_vec().into(),
                    extension: "jpg".to_string(),
                },
                ChapterPage {
                    bytes: include_bytes!("../../../data_test/images/3.jpg").to_vec().into(),
                    extension: "jpg".to_string(),
                },
            ],
        };

        let downloader = RawImagesDownloader::new();

        downloader.save_chapter_in_file_system(&AppDirectories::MangaDownloads.get_full_path(), chapter)?;

        Ok(())
    }
}
