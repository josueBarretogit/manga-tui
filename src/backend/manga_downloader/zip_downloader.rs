use std::fs::File;
use std::io::Write;

use super::MangaDownloader;

pub struct ZipDownloader {}

impl MangaDownloader for ZipDownloader {
    fn save_chapter_in_file_system(
        self,
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
