/// Shortcut for: Path::new($path).try_exists().is_ok_and(|is_true| is_true)
#[macro_export]
macro_rules! exists {
    ($path:expr) => {
        Path::new($path).try_exists().is_ok_and(|is_true| is_true)
    };
}

#[derive(Debug)]
pub struct SearchTerm(String);

impl SearchTerm {
    pub fn trimmed_lowercased(search_term: &str) -> Option<Self> {
        let search_term = search_term.trim();
        if search_term.is_empty() { None } else { Some(Self(search_term.to_lowercase())) }
    }

    pub fn get(&self) -> &str {
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
}
