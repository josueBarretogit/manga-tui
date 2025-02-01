use std::fs::File;
use std::io::Write;

use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use super::MangaDownloader;

#[derive(Debug)]
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
