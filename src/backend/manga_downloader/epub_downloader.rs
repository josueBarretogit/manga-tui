use std::fs::File;

use epub_builder::{EpubBuilder, EpubContent, ZipLibrary};

use super::MangaDownloader;
/// xml template to build epub files
static EPUB_FILE_TEMPLATE: &str = "<?xml version='1.0' encoding='utf-8'?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\">\n  <head>\n    <title>Panel</title>\n    <meta http-equiv=\"Content-Type\" content=\"text/html; charset=utf-8\"/>\n  </head>\n  <body>\n    <div class=\"centered_image\">\n        <img src=\"REPLACE_IMAGE_SOURCE\" alt=\"Panel\" />\n    </div>\n  </body>\n</html>\n";

#[derive(Debug, Clone)]
pub struct EpubDownloader {}

impl EpubDownloader {
    pub fn new() -> Self {
        Self {}
    }
}

impl MangaDownloader for EpubDownloader {
    fn save_chapter_in_file_system(
        &self,
        base_directory: &std::path::Path,
        chapter: super::ChapterToDownloadSanitized,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let base_directory = self.make_manga_base_directory_name(base_directory, &chapter);

        self.create_manga_base_directory(&base_directory)?;

        let epub_path = base_directory.join(format!("{}.epub", self.make_chapter_name(&chapter).display()));

        let mut epub_file = File::create(&epub_path)?;

        let mut epub_builder = EpubBuilder::new(ZipLibrary::new()?)?;

        epub_builder.epub_version(epub_builder::EpubVersion::V30);

        epub_builder.metadata("title", chapter.manga_title.to_string()).ok();

        for (index, chap) in chapter.pages.into_iter().enumerate() {
            let file_name = format!("{:04}.{}", index + 1, chap.extension);
            let image_path = format!("data/{file_name}");

            let mime_type = match chap.extension.to_lowercase().as_str() {
                "jpg" | "jpeg" => "image/jpeg",
                "png" => "image/png",
                "webp" => "image/webp",
                "gif" => "image/gif",
                ext => &format!("image/{ext}"),
            };

            if index == 0 {
                epub_builder.add_cover_image(&image_path, chap.bytes.as_ref(), mime_type)?;
            } else {
                epub_builder.add_resource(&image_path, chap.bytes.as_ref(), mime_type)?;
            }

            let xml_file_path = format!("{index}.xhtml");

            epub_builder.add_content(
                EpubContent::new(xml_file_path, EPUB_FILE_TEMPLATE.replace("REPLACE_IMAGE_SOURCE", &image_path).as_bytes())
                    .title(&file_name),
            )?;
        }

        epub_builder.generate(&mut epub_file)?;

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
            ..Default::default()
        };

        let downloader = EpubDownloader::new();

        downloader.save_chapter_in_file_system(&AppDirectories::MangaDownloads.get_full_path(), chapter)?;

        Ok(())
    }
}
