use std::fmt::Display;
use std::path::{Path, PathBuf};

/// Shortcut for: Path::new($path).try_exists().is_ok_and(|is_true| is_true)
#[macro_export]
macro_rules! exists {
    ($path:expr) => {
        Path::new($path).try_exists().is_ok_and(|is_true| is_true)
    };
}

/// This type ensures that the inner `String` is never an empty string, it is also lowercased and
/// trimmed to be used in searches
#[derive(Debug, Default)]
pub struct SearchTerm(String);

impl Display for SearchTerm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.get())
    }
}

impl SearchTerm {
    pub fn trimmed_lowercased(search_term: &str) -> Option<Self> {
        let search_term = search_term.trim();
        if search_term.is_empty() { None } else { Some(Self(search_term.to_lowercase())) }
    }

    pub fn get(&self) -> &str {
        &self.0
    }
}

/// Remove special characteres that may cause errors when creating directories or files
fn remove_conflicting_characteres<T: AsRef<Path>>(title: T) -> PathBuf {
    let invalid_chars = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

    let title: &Path = title.as_ref();
    let title = title.to_str().unwrap().trim();

    let sanitized_title: String = title.chars().map(|c| if invalid_chars.contains(&c) { '_' } else { c }).collect();

    sanitized_title.into()
}

/// This type ensures that the inner `PathBuf` doesnt contain characteres tha may throw errors
/// like ":" or "/"
#[derive(Debug, Default, PartialEq)]
pub struct SanitizedFilename(PathBuf);

impl Display for SanitizedFilename {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

impl SanitizedFilename {
    pub fn new<T: AsRef<Path>>(name: T) -> Self {
        Self(remove_conflicting_characteres(name))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn none_when_search_term_is_empty() {
        let search_term = "";

        let search = SearchTerm::trimmed_lowercased(search_term);

        assert!(search.is_none());
    }

    #[test]
    fn search_term_is_lowercased() {
        let sample = "Some Example";

        assert_eq!("some example", SearchTerm::trimmed_lowercased(sample).unwrap().get());
    }

    #[test]
    fn filename_does_not_contain_conflicting_characteres() {
        let example = "a good example";

        let to_correct_filename = remove_conflicting_characteres(example);
        assert_eq!(example, to_correct_filename.to_str().unwrap());

        let bad_example = "a / wrong example";

        let to_correct_filename = remove_conflicting_characteres(bad_example);
        assert_eq!("a _ wrong example", to_correct_filename.to_str().unwrap());
    }

    #[test]
    fn filename_is_constructed_correctly() {
        let file_name = SanitizedFilename::new("some name which contains :");

        assert_eq!(Path::new("some name which contains _"), file_name.as_path())
    }
}
