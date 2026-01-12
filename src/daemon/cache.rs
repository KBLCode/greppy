use crate::search::SearchResponse;
use lru::LruCache;
use std::num::NonZeroUsize;

/// Cache for search query results.
/// Keys are formatted as "project_path:query:limit"
pub struct QueryCache {
    cache: LruCache<String, SearchResponse>,
}

impl QueryCache {
    pub fn new() -> Self {
        Self {
            cache: LruCache::new(NonZeroUsize::new(100).unwrap()),
        }
    }

    /// Get a cached response by key
    pub fn get(&mut self, key: &str) -> Option<&SearchResponse> {
        self.cache.get(key)
    }

    /// Store a response in the cache
    pub fn put(&mut self, key: String, value: SearchResponse) {
        self.cache.put(key, value);
    }

    /// Clear all cached entries for a specific project.
    /// Keys are expected to be in format "project_path:query:limit"
    pub fn clear_project(&mut self, project_path: &str) {
        // Collect keys to remove (can't modify while iterating)
        let keys_to_remove: Vec<String> = self
            .cache
            .iter()
            .filter_map(|(key, _)| {
                if key.starts_with(project_path) && key.chars().nth(project_path.len()) == Some(':')
                {
                    Some(key.clone())
                } else {
                    None
                }
            })
            .collect();

        // Remove matching keys
        for key in keys_to_remove {
            self.cache.pop(&key);
        }
    }

    /// Clear the entire cache
    pub fn clear_all(&mut self) {
        self.cache.clear();
    }

    /// Get the number of cached entries
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

impl Default for QueryCache {
    fn default() -> Self {
        Self::new()
    }
}
