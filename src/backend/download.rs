use super::APP_DATA_DIR;

pub struct DownloadChapter<'a> {
    manga_title: &'a str,
    title: &'a str,
    number: &'a u32,
    scanlator: &'a str,
}

pub fn download_chapter(chapter: DownloadChapter<'_>) {
    // need directory with the manga's title,


    let dir_manga_downloads = APP_DATA_DIR.as_ref().unwrap().join("mangaDownloads");

    // need directory with chapter's title and scanlator
    // create images and store them in the directory
}
