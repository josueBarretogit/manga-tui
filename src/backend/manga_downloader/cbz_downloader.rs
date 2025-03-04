use std::fs::File;
use std::io::Write;

use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use super::MangaDownloader;

#[derive(Debug, Clone)]
pub struct CbzDownloader {}

impl CbzDownloader {
    pub fn new() -> Self {
        Self {}
    }
}

impl MangaDownloader for CbzDownloader {
    fn save_chapter_in_file_system(
        &self,
        base_directory: &std::path::Path,
        chapter: super::ChapterToDownloadSanitized,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let base_directory = self.make_manga_base_directory_name(base_directory, &chapter);

        self.create_manga_base_directory(&base_directory)?;

        let cbz_filename = format!("{}.cbz", self.make_chapter_name(&chapter).display());

        let cbz_path = base_directory.join(&cbz_filename);

        let cbz_file = File::create(&cbz_path)?;

        let mut zip = ZipWriter::new(cbz_file);

        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);

        for (index, chap) in chapter.pages.into_iter().enumerate() {
            let file_name = format!("{}.{}", index + 1, chap.extension);

            zip.start_file(file_name, options).ok();

            zip.write_all(&chap.bytes).ok();
        }
        zip.finish()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use fake::Fake;
    use fake::faker::name::en::Name;
    use uuid::Uuid;

    use super::*;
    use crate::backend::AppDirectories;
    use crate::backend::manga_downloader::ChapterToDownloadSanitized;
    use crate::backend::manga_provider::{ChapterPage, Languages};
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

        let downloader = CbzDownloader::new();

        downloader.save_chapter_in_file_system(&AppDirectories::MangaDownloads.get_full_path(), chapter)?;

        Ok(())
    }
}
