use crate::search::SearchResponse;
use lru::LruCache;
use std::num::NonZeroUsize;

pub struct QueryCache {
    cache: LruCache<String, SearchResponse>,
}

impl QueryCache {
    pub fn new() -> Self {
        Self {
            cache: LruCache::new(NonZeroUsize::new(100).unwrap()),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&SearchResponse> {
        self.cache.get(key)
    }

    pub fn put(&mut self, key: String, value: SearchResponse) {
        self.cache.put(key, value);
    }

    pub fn clear_project(&mut self, _project_path: &str) {
        // Naive implementation: iterate and remove keys starting with project_path
        // LruCache doesn't support retain efficiently, so we might need a better structure
        // or just clear everything for now if it's too complex.
        // For now, let's just clear everything to be safe and simple.
        self.cache.clear();
    }
}
