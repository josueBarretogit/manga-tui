use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::{Cacher, Entry, InsertEntry};

/// Implementation of In-memory cache, the entries stored with this struct will be dropped after the programm's execution
/// and each entry has a time since creation and a duration or `time_to_live` so that older
/// entries are removed and newer ones stay, this cleanup proccess is called Least Recently Used [`LRU`](https://www.geeksforgeeks.org/lru-cache-implementation/)
/// this requires [`Interior mutability`](https://doc.rust-lang.org/book/ch15-05-interior-mutability.html)
/// because the trait `Cacher` requires `Self` to be immutable but we need to persist data
#[derive(Debug)]
pub struct InMemoryCache {
    entries: Mutex<HashMap<String, MemoryEntry>>,
    /// Indicates how much entries should remain in the cache before being cleanup
    capacity: usize,
}

/// Keeping track of `time_since_creation` to know how long the entry has existed and
/// `time_to_live` which indicates how long the entry should exist
#[derive(Debug)]
struct MemoryEntry {
    data: String,
    time_since_creation: Instant,
    time_to_live: Duration,
}

impl MemoryEntry {
    fn new(data: String, time_to_live: Duration) -> Self {
        Self {
            data,
            time_since_creation: Instant::now(),
            time_to_live,
        }
    }

    /// Returns `true` if the time since creation that has elapsed is greatern than the time it
    /// should live
    fn is_expired(&self) -> bool {
        self.time_since_creation.elapsed() > self.time_to_live
    }
}

impl InMemoryCache {
    /// Only way of constructing this struct to make sure when initializing the cleanup task is spawned
    pub fn init(capacity: usize) -> Arc<Self> {
        let cache = Arc::new(Self::new().with_capacity(capacity));

        start_cleanup_task(Arc::clone(&cache));

        cache
    }

    fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            capacity: 5,
        }
    }

    /// Set how many entries to hold before starting the cleanup proccess
    pub fn with_capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }

    /// Utility mainly for tests, should always remain private
    fn with_cached_data(data: HashMap<String, MemoryEntry>) -> Self {
        Self {
            entries: Mutex::new(data),
            capacity: 5,
        }
    }
}

impl Cacher for InMemoryCache {
    fn get(&self, id: &str) -> Result<Option<Entry>, Box<dyn std::error::Error>> {
        let mut cached: Option<Entry> = None;
        {
            let mut entries = self.entries.lock().map_err(|e| "could not get cached data")?;

            let entry = entries.get_mut(id);
            if let Some(en) = entry {
                en.time_since_creation = Instant::now();
                cached = Some(Entry {
                    data: en.data.to_string(),
                })
            }
        }

        Ok(cached)
    }

    fn cache(&self, entry: InsertEntry) -> Result<(), Box<dyn std::error::Error>> {
        {
            let mut entries = self.entries.lock().unwrap();

            entries.insert(entry.id.to_string(), MemoryEntry::new(entry.data.to_string(), entry.duration));
        }

        Ok(())
    }
}

fn start_cleanup_task(cache: Arc<InMemoryCache>) -> JoinHandle<()> {
    std::thread::spawn(move || {
        loop {
            let mut entries = cache.entries.lock().unwrap();
            if entries.len() > cache.capacity {
                entries.retain(|_, entry| !entry.is_expired());
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::backend::cache::{Entry, InsertEntry};

    #[test]
    fn it_saves_and_retrieves_data() -> Result<(), Box<dyn Error>> {
        let in_memory = InMemoryCache::new();

        let data: InsertEntry = InsertEntry {
            id: "entry1",
            data: "some data",
            duration: Duration::from_secs(3),
        };

        let data2: InsertEntry = InsertEntry {
            id: "entry2",
            data: "some data entry2",
            duration: Duration::from_secs(3),
        };

        in_memory.cache(data.clone())?;
        in_memory.cache(data2.clone())?;

        let cache_found1 = in_memory.get("entry1")?;
        let cache_found2 = in_memory.get("entry2")?;

        assert_eq!(
            Some(Entry {
                data: data.data.to_string()
            }),
            cache_found1
        );

        assert_eq!(
            Some(Entry {
                data: data2.data.to_string()
            }),
            cache_found2
        );

        Ok(())
    }

    #[test]
    fn cached_entry_know_when_they_are_expired() -> Result<(), Box<dyn Error>> {
        let in_memory = InMemoryCache::with_cached_data(HashMap::from([
            /// 3 seconds have passed and the time to live is only 2 seconds so it should be
            ///   expired
            ("id".to_string(), MemoryEntry {
                time_since_creation: Instant::now() - Duration::from_secs(3),
                time_to_live: Duration::from_secs(2),
                data: "some data".to_string(),
            }),
            ("id_not_expired".to_string(), MemoryEntry {
                time_since_creation: Instant::now(),
                time_to_live: Duration::from_secs(5),
                data: "some data 2".to_string(),
            }),
        ]));

        {
            let entries = in_memory.entries.lock().unwrap();

            let inserted_entry = entries.get("id").unwrap();

            assert!(inserted_entry.is_expired());
        }

        {
            let entries = in_memory.entries.lock().unwrap();

            let inserted_entry = entries.get("id_not_expired").unwrap();

            assert!(!inserted_entry.is_expired());
        }

        Ok(())
    }

    #[test]
    fn background_task_removes_expired_entries() -> Result<(), Box<dyn Error>> {
        let in_memory = Arc::new(
            InMemoryCache::with_cached_data(HashMap::from([
                /// 3 seconds have passed and the time to live is only 2 seconds so it should be
                ///   expired
                ("id".to_string(), MemoryEntry {
                    time_since_creation: Instant::now() - Duration::from_secs(3),
                    time_to_live: Duration::from_secs(2),
                    data: "some data".to_string(),
                }),
                ("id_should_not_exist".to_string(), MemoryEntry {
                    time_since_creation: Instant::now() - Duration::from_secs(10),
                    time_to_live: Duration::from_secs(2),
                    data: "some data".to_string(),
                }),
                ("id_should_live".to_string(), MemoryEntry {
                    time_since_creation: Instant::now(),
                    time_to_live: Duration::from_secs(10),
                    data: "some data 2".to_string(),
                }),
                ("id_should_also_live".to_string(), MemoryEntry {
                    time_since_creation: Instant::now(),
                    time_to_live: Duration::from_secs(15),
                    data: "some data 3".to_string(),
                }),
            ]))
            .with_capacity(3),
        );

        let handle = start_cleanup_task(Arc::clone(&in_memory));

        sleep(Duration::from_millis(800));

        let should_not_exist = in_memory.get("id")?.is_none();
        let should_not_exist2 = in_memory.get("id_should_not_exist")?.is_none();
        let should_exist = in_memory.get("id_should_live")?.is_some();
        let should_exist2 = in_memory.get("id_should_also_live")?.is_some();

        assert!(should_not_exist);
        assert!(should_exist);
        assert!(should_not_exist2);
        assert!(should_exist2);

        Ok(())
    }

    #[test]
    fn background_cleanup_task_doesnt_remove_entries_if_cache_capacity_is_not_exceeded() -> Result<(), Box<dyn Error>> {
        let in_memory = Arc::new(
            InMemoryCache::with_cached_data(HashMap::from([
                /// 3 seconds have passed and the time to live is only 2 seconds so it should be
                ///   expired
                ("expired".to_string(), MemoryEntry {
                    time_since_creation: Instant::now() - Duration::from_secs(3),
                    time_to_live: Duration::from_secs(2),
                    data: "some data".to_string(),
                }),
                ("expired2".to_string(), MemoryEntry {
                    time_since_creation: Instant::now() - Duration::from_secs(10),
                    time_to_live: Duration::from_secs(2),
                    data: "some data".to_string(),
                }),
                ("expired3".to_string(), MemoryEntry {
                    time_since_creation: Instant::now() - Duration::from_secs(10),
                    time_to_live: Duration::from_secs(1),
                    data: "some data 2".to_string(),
                }),
            ]))
            .with_capacity(5),
        );

        let handle = start_cleanup_task(Arc::clone(&in_memory));

        sleep(Duration::from_millis(800));

        let should_exist = in_memory.get("expired")?.is_some();
        let should_exist2 = in_memory.get("expired2")?.is_some();

        assert!(should_exist);

        assert!(should_exist2);

        Ok(())
    }

    #[test]
    fn if_the_entry_is_retrieved_then_time_to_live_should_be_renewed() -> Result<(), Box<dyn Error>> {
        let in_memory = InMemoryCache::with_cached_data(HashMap::from([("exists".to_string(), MemoryEntry {
            time_since_creation: Instant::now() - Duration::from_secs(10),
            time_to_live: Duration::from_secs(5),
            data: "some data".to_string(),
        })]));

        in_memory.get("exists")?;

        {
            let entries = in_memory.entries.lock().unwrap();

            assert_eq!(entries.get("exists").unwrap().time_since_creation.elapsed().as_secs(), Instant::now().elapsed().as_secs());
        }

        Ok(())
    }
}
