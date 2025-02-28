use std::fs::File;
use std::io::{BufWriter, Cursor, Write};
use std::path::Path;

use flate2::write::ZlibEncoder;
use flate2::Compression;
use image::{DynamicImage, GenericImageView, ImageFormat};
use lopdf::{dictionary, Document, Object, Stream};

use super::MangaDownloader;

pub struct PdfDownloader {}

impl PdfDownloader {
    pub fn new() -> Self {
        Self {}
    }
}

impl MangaDownloader for PdfDownloader {
    fn save_chapter_in_file_system(
        &self,
        base_directory: &Path,
        chapter: super::ChapterToDownloadSanitized,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let base_directory = self.make_manga_base_directory_name(base_directory, &chapter);
        self.create_manga_base_directory(&base_directory)?;

        let pdf_path = base_directory.join(format!("{}.pdf", self.make_chapter_name(&chapter).display()));

        let mut doc = Document::with_version("1.7");
        let mut pages = Vec::new();
        let page_width = 595.0;

        for (index, page) in chapter.pages.iter().enumerate() {
            let img = image::load_from_memory(&page.bytes)?;
            let (img_width, img_height) = img.dimensions();
            let mut img_data = Vec::new();
            let mut filter = None;
            let mut color_space = "DeviceRGB";

            match page.extension.as_str() {
                "jpg" | "jpeg" => {
                    let mut cursor = Cursor::new(Vec::new());
                    img.write_to(&mut cursor, ImageFormat::Jpeg)?;
                    img_data = cursor.into_inner();
                    filter = Some("DCTDecode");
                },
                "png" => {
                    let raw_img = img.to_rgb8();
                    let uncompressed_data = raw_img.into_raw();
                    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
                    encoder.write_all(&uncompressed_data)?;
                    img_data = encoder.finish()?;
                    filter = Some("FlateDecode");
                },
                _ => return Err(format!("Unsupported image format: {}", page.extension).into()),
            };

            let img_dict = dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => img_width as i64,
                "Height" => img_height as i64,
                "ColorSpace" => color_space,
                "BitsPerComponent" => 8,
                "Filter" => filter.unwrap(),
                "Length" => img_data.len() as i64
            };

            let img_stream = Stream::new(img_dict, img_data);
            let img_id = doc.add_object(img_stream);

            let scale_factor = page_width / img_width as f32;
            let scaled_w = img_width as f32 * scale_factor;
            let scaled_h = img_height as f32 * scale_factor;

            let contents =
                Stream::new(dictionary! {}, format!("q {} 0 0 {} 0 0 cm /Im{} Do Q", scaled_w, scaled_h, index).into_bytes());

            let contents_id = doc.add_object(contents);

            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "MediaBox" => vec![0.0.into(), 0.0.into(), 595.0.into(), 842.0.into()],
                "Resources" => dictionary! {
                    "XObject" => dictionary! {
                        format!("Im{}", index) => img_id,
                    }
                },
                "Contents" => contents_id,
            });

            pages.push(page_id);
        }

        let pages_id = doc.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => pages.iter().map(|&p| p.into()).collect::<Vec<_>>(),
            "Count" => pages.len() as i32,
        });

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });

        doc.trailer.set("Root", catalog_id);

        let mut file = File::create(pdf_path)?;
        doc.save_to(&mut BufWriter::new(file))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fs;
    use std::path::PathBuf;

    use fake::faker::name::en::Name;
    use fake::Fake;
    use lopdf::Document;
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
            download_type: DownloadType::Pdf,
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

        let downloader = PdfDownloader::new();
        let base_path = AppDirectories::MangaDownloads.get_full_path();

        downloader.save_chapter_in_file_system(&base_path, chapter.clone())?;

        Ok(())
    }
}
