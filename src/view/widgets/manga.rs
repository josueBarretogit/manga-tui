#[derive(Clone)]
pub struct ChapterItem {
    id: String,
    title: String,
    chapter_number: String,
    is_read: bool,
    translated_language: String,
}

impl ChapterItem {
    pub fn new(id: String, title: String, chapter_number: String, is_read: bool, translated_language: String) -> Self {
        Self { id, title, chapter_number, is_read, translated_language }
    }
}

#[derive(Clone)]
pub struct ChaptersListWidget {
    chapters: Vec<ChapterItem>,
}

impl ChaptersListWidget {
    pub fn new(chapters: Vec<ChapterItem>) -> Self {
        Self { chapters }
    }
}


