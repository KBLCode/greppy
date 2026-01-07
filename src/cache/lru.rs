use crate::config::CACHE_SIZE;
use crate::search::SearchResponse;
use lru::LruCache;
use std::num::NonZeroUsize;

/// LRU cache for query results
pub struct QueryCache {
    cache: LruCache<String, SearchResponse>,
}

impl QueryCache {
    pub fn new() -> Self {
        Self {
            cache: LruCache::new(NonZeroUsize::new(CACHE_SIZE).unwrap()),
        }
    }

    pub fn get(&self, key: &str) -> Option<SearchResponse> {
        // Note: LruCache::peek doesn't update access order, but we want to
        // For now, we clone and don't update order (simpler)
        self.cache.peek(key).cloned()
    }

    pub fn put(&mut self, key: String, value: SearchResponse) {
        self.cache.put(key, value);
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Invalidate all cache entries for a specific project
    pub fn invalidate_project(&mut self, project_path: &str) {
        // Collect keys to remove (can't modify while iterating)
        let keys_to_remove: Vec<String> = self
            .cache
            .iter()
            .filter(|(k, _)| k.starts_with(project_path))
            .map(|(k, _)| k.clone())
            .collect();

        for key in keys_to_remove {
            self.cache.pop(&key);
        }
    }
}

impl Default for QueryCache {
    fn default() -> Self {
        Self::new()
    }
}
