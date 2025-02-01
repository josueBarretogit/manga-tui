use std::fs::File;

use epub_builder::{EpubBuilder, EpubContent, ZipLibrary};

use super::MangaDownloader;
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

#[derive(Debug)]
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
            let file_name = format!("{}.{}", index + 1, chap.extension);
            let image_path = format!("data/{}", file_name);

            let mime_type = format!("image/{}", chap.extension);

            if index == 0 {
                epub_builder.add_cover_image(&image_path, chap.bytes.as_ref(), &mime_type).ok();
            }

            epub_builder.add_resource(&image_path, chap.bytes.as_ref(), &mime_type).ok();

            let xml_file_path = format!("{}.xhtml", index);

            epub_builder
                .add_content(
                    EpubContent::new(xml_file_path, EPUB_FILE_TEMPLATE.replace("REPLACE_IMAGE_SOURCE", &image_path).as_bytes())
                        .title(&file_name),
                )
                .ok();
        }

        epub_builder.generate(&mut epub_file)?;

        Ok(())
    }
}
