use std::error::Error;
use std::fmt::Debug;

pub mod in_memory;

/// Gives a hint to `Cacher` implementations as to how long entries should last,
/// for in-memory cache it should lasts seconds, 40 seconds or more for the `Long` variant, 20-25
/// seconds for `Medium` and so on
/// for file-based or database cache impĺementations it can last anywhere from minutes to days even
/// so each implementation must know how long the cache should live
#[derive(Debug, PartialEq, Clone)]
pub enum CacheDuration {
    LongLong, // longest
    Long,
    Medium,
    Short,
    VeryShort, //shortest
}

#[derive(Debug, PartialEq, Clone)]
pub struct InsertEntry<'a> {
    pub id: &'a str,
    pub data: &'a [u8],
    /// How long this entry will last until it is removed
    pub duration: CacheDuration,
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Entry {
    pub data: Vec<u8>,
}

/// Cache which is specially important when scraping sites in order to reduce making requests and
/// thus reduce the chances of being blocked
pub trait Cacher: Send + Sync + Debug {
    fn cache(&self, entry: InsertEntry) -> Result<(), Box<dyn Error>>;
    /// Generally if an entry was found it should be renewed, since it was accessed and is very
    /// likely to be accesed again
    fn get(&self, id: &str) -> Result<Option<Entry>, Box<dyn Error>>;
}

#[cfg(test)]
pub mod mock {
    use std::sync::Arc;

    use super::Cacher;

    #[derive(Debug)]
    pub struct EmptyCache;
    impl EmptyCache {
        pub fn new_arc() -> Arc<Self> {
            Arc::new(EmptyCache)
        }
    }

    impl Cacher for EmptyCache {
        fn get(&self, id: &str) -> Result<Option<super::Entry>, Box<dyn std::error::Error>> {
            Ok(None)
        }

        fn cache(&self, entry: super::InsertEntry) -> Result<(), Box<dyn std::error::Error>> {
            Ok(())
        }
    }
}
