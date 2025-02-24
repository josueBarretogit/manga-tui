use std::error::Error;
use std::fmt::{Debug, Display};
use std::time::Duration;

pub mod in_memory;

#[derive(Debug, PartialEq, Clone, Default)]
pub struct InsertEntry<'a> {
    pub id: &'a str,
    pub data: &'a str,
    /// How long this entry will last until it is removed
    pub duration: Duration,
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Entry {
    pub data: String,
}

/// Cache which is specially important when scraping sites in order to reduce making requests and
/// thus reduce the chances of being blocked
pub trait Cacher: Send + Sync + Debug {
    fn cache(&self, entry: InsertEntry) -> Result<(), Box<dyn Error>>;
    /// Generally if an entry was found it should be renewed, since it was accessed and is very
    /// likely to be accesed again
    fn get(&self, id: &str) -> Result<Option<Entry>, Box<dyn Error>>;
}
