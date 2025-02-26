use std::cmp::Ordering;
use std::fmt::{Debug, Display};
use std::fs;
use std::io::Write;
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

    pub fn trimmed(search_term: &str) -> Option<Self> {
        let search_term = search_term.trim();
        if search_term.is_empty() { None } else { Some(Self(search_term.to_string())) }
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

/// This type ensures that a filename will not contain characteres that may throw errors
/// like ":" or "/"
#[derive(Debug, Default, PartialEq, Clone)]
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

impl<T: AsRef<Path>> From<T> for SanitizedFilename {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

/// A `Vec` that is guaranteed to be sorted
/// and with no duplicates
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SortedVec<T: Debug>(Vec<T>);

impl<T: Debug> SortedVec<T> {
    pub fn sorted_by<F>(mut vec: Vec<T>, by: F) -> Self
    where
        F: FnMut(&T, &T) -> Ordering,
        T: PartialEq,
    {
        vec.sort_by(by);

        vec.dedup();

        Self(vec)
    }

    pub fn new(mut vec: Vec<T>) -> Self
    where
        T: Ord,
    {
        vec.sort();

        Self(vec)
    }

    pub fn as_slice(&self) -> &[T] {
        &self.0
    }
}

pub struct Log;

impl Log {
    pub fn debug<T: Debug>(val: &T, _path: &Path) {
        let mut data_file = fs::File::create("debug.txt").expect("creation failed");

        data_file.write_all(format!("{:#?}", val).as_bytes()).expect("write failed");
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

        assert_eq!(Path::new("some name which contains _"), file_name.as_path());

        let file_name = SanitizedFilename::new("some / name / which contains ");

        assert_eq!(Path::new("some _ name _ which contains"), file_name.as_path())
    }

    #[test]
    fn sorted_vec_is_constructed_correctly() {
        let vec: Vec<u32> = [3, 10, 4].to_vec();

        let expected = [3, 4, 10];
        let sorted_vec = SortedVec::new(vec);

        assert_eq!(expected, sorted_vec.as_slice())
    }

    #[derive(PartialEq, PartialOrd, Clone, Debug)]
    struct Sort<'a> {
        number: f64,
        val: &'a str,
    }

    #[test]
    fn sorted_vec_by_closure_and_with_no_duplicates() {
        let vec: Vec<Sort> = [
            Sort {
                number: 2.3,
                val: "",
            },
            Sort {
                number: 2.3,
                val: "",
            },
            Sort {
                number: 1.0,
                val: "",
            },
            Sort {
                number: 5.3,
                val: "",
            },
        ]
        .to_vec();

        let expected = [
            Sort {
                number: 1.0,
                val: "",
            },
            Sort {
                number: 2.3,
                val: "",
            },
            Sort {
                number: 5.3,
                val: "",
            },
        ];

        let sorted_vec = SortedVec::sorted_by(vec, |a, b| a.number.total_cmp(&b.number));

        assert_eq!(expected, sorted_vec.as_slice())
    }
}
