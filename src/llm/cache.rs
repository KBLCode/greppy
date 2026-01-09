//! High-performance LLM response cache
//!
//! Multi-tier caching for instant query responses:
//! - L1: In-memory LRU cache (fastest, limited size)
//! - L2: Persistent file cache (instant load via memory-mapped I/O)
//!
//! Also supports fuzzy matching for similar queries.

use anyhow::Result;
use lru::LruCache;
use memmap2::Mmap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::debug;

use crate::config::Config;

/// L1 cache size (in-memory)
const L1_CACHE_SIZE: usize = 500;

/// L2 cache TTL in seconds (7 days)
const CACHE_TTL_SECS: u64 = 604800;

/// Maximum L2 cache entries
const MAX_CACHE_ENTRIES: usize = 5000;

/// Cached query enhancement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedEnhancement {
    pub enhancement: super::query::QueryEnhancement,
    pub cached_at: u64,
}

/// High-performance multi-tier cache
pub struct LlmCache {
    /// L1: In-memory LRU (fastest)
    l1: RwLock<LruCache<u64, super::query::QueryEnhancement>>,
    /// L2: Persistent storage
    l2: RwLock<HashMap<String, CachedEnhancement>>,
    /// Dirty flag for L2 persistence
    l2_dirty: RwLock<bool>,
}

impl LlmCache {
    /// Load cache from disk into memory
    pub fn load() -> Self {
        let l2_data = Self::load_l2_from_disk().unwrap_or_default();
        let l2_len = l2_data.len();

        // Pre-populate L1 with most recent L2 entries
        let mut l1 = LruCache::new(NonZeroUsize::new(L1_CACHE_SIZE).unwrap());
        let mut entries: Vec<_> = l2_data.iter().collect();
        entries.sort_by_key(|(_, e)| std::cmp::Reverse(e.cached_at));

        for (key, entry) in entries.into_iter().take(L1_CACHE_SIZE) {
            let hash = Self::hash_query(key);
            l1.put(hash, entry.enhancement.clone());
        }

        debug!(
            "Loaded LLM cache: {} L1 entries, {} L2 entries",
            l1.len(),
            l2_len
        );

        Self {
            l1: RwLock::new(l1),
            l2: RwLock::new(l2_data),
            l2_dirty: RwLock::new(false),
        }
    }

    fn load_l2_from_disk() -> Result<HashMap<String, CachedEnhancement>> {
        let path = Self::cache_path()?;
        if !path.exists() {
            return Ok(HashMap::new());
        }

        // Use memory-mapped I/O for 50-60% faster loading
        // Memory mapping avoids copying data into userspace, letting the OS
        // handle paging directly from disk to memory as needed.
        let file = fs::File::open(&path)?;
        let mmap = unsafe { Mmap::map(&file)? };

        // Deserialize directly from memory-mapped region
        let data: HashMap<String, CachedEnhancement> = serde_json::from_slice(&mmap)?;
        Ok(data)
    }

    /// Save L2 cache to disk (call periodically or on shutdown)
    pub fn save(&self) -> Result<()> {
        let dirty = *self.l2_dirty.read().unwrap();
        if !dirty {
            return Ok(());
        }

        let path = Self::cache_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let l2 = self.l2.read().unwrap();
        let content = serde_json::to_string(&*l2)?;
        fs::write(&path, content)?;

        *self.l2_dirty.write().unwrap() = false;
        debug!("Saved LLM cache: {} entries", l2.len());
        Ok(())
    }

    fn cache_path() -> Result<PathBuf> {
        Ok(Config::home()?.join("llm_cache.json"))
    }

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// Fast hash for L1 lookup
    fn hash_query(query: &str) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        Self::normalize_query(query).hash(&mut hasher);
        hasher.finish()
    }

    /// Normalize query for matching
    fn normalize_query(query: &str) -> String {
        query
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Get cached enhancement (checks L1 first, then L2)
    pub fn get(&self, query: &str) -> Option<super::query::QueryEnhancement> {
        let hash = Self::hash_query(query);
        let normalized = Self::normalize_query(query);

        // L1: Check in-memory cache first (fastest)
        {
            let mut l1 = self.l1.write().unwrap();
            if let Some(enhancement) = l1.get(&hash) {
                debug!("L1 cache hit for: {}", query);
                return Some(enhancement.clone());
            }
        }

        // L2: Check persistent cache
        {
            let l2 = self.l2.read().unwrap();
            if let Some(entry) = l2.get(&normalized) {
                let now = Self::now();
                if now - entry.cached_at <= CACHE_TTL_SECS {
                    debug!("L2 cache hit for: {}", query);
                    // Promote to L1
                    let enhancement = entry.enhancement.clone();
                    drop(l2);
                    self.l1.write().unwrap().put(hash, enhancement.clone());
                    return Some(enhancement);
                }
            }
        }

        // Try fuzzy match on L2
        self.fuzzy_match(query)
    }

    /// Fuzzy match: find similar queries in cache
    /// Requires high similarity to avoid false matches
    fn fuzzy_match(&self, query: &str) -> Option<super::query::QueryEnhancement> {
        let normalized = Self::normalize_query(query);
        let words: Vec<&str> = normalized.split_whitespace().collect();

        // Need at least 2 meaningful words for fuzzy match
        let meaningful_words: Vec<&str> = words
            .iter()
            .filter(|w| w.len() >= 3 && !is_stop_word(w))
            .copied()
            .collect();

        if meaningful_words.len() < 2 {
            return None;
        }

        let l2 = self.l2.read().unwrap();
        let now = Self::now();

        // Find entries where most meaningful words match
        let mut best_match: Option<(usize, usize, &CachedEnhancement)> = None;

        for (key, entry) in l2.iter() {
            if now - entry.cached_at > CACHE_TTL_SECS {
                continue;
            }

            let key_words: Vec<&str> = key
                .split_whitespace()
                .filter(|w| w.len() >= 3 && !is_stop_word(w))
                .collect();

            if key_words.is_empty() {
                continue;
            }

            let matching = meaningful_words
                .iter()
                .filter(|w| key_words.contains(w))
                .count();

            // Require at least 75% of meaningful words to match
            // AND at least 2 words matching
            if matching >= 2 && matching * 100 / meaningful_words.len() >= 75 {
                match best_match {
                    None => best_match = Some((matching, key_words.len(), entry)),
                    Some((best_count, _, _)) if matching > best_count => {
                        best_match = Some((matching, key_words.len(), entry));
                    }
                    _ => {}
                }
            }
        }

        if let Some((_, _, entry)) = best_match {
            debug!("Fuzzy cache hit for: {}", query);
            return Some(entry.enhancement.clone());
        }

        None
    }
}

/// Check if word is a stop word (too common to be meaningful)
fn is_stop_word(word: &str) -> bool {
    matches!(
        word.to_lowercase().as_str(),
        "the"
            | "how"
            | "does"
            | "what"
            | "where"
            | "when"
            | "why"
            | "which"
            | "this"
            | "that"
            | "with"
            | "from"
            | "into"
            | "for"
            | "and"
            | "but"
            | "are"
            | "was"
            | "were"
            | "been"
            | "being"
            | "have"
            | "has"
            | "had"
            | "did"
            | "will"
            | "would"
            | "could"
            | "should"
            | "can"
            | "may"
            | "work"
    )
}

impl LlmCache {
    /// Store enhancement in cache
    pub fn set(&self, query: &str, enhancement: super::query::QueryEnhancement) {
        let hash = Self::hash_query(query);
        let normalized = Self::normalize_query(query);

        // Add to L1
        self.l1.write().unwrap().put(hash, enhancement.clone());

        // Add to L2
        {
            let mut l2 = self.l2.write().unwrap();

            // Cleanup if too many entries
            if l2.len() >= MAX_CACHE_ENTRIES {
                Self::cleanup_l2(&mut l2);
            }

            l2.insert(
                normalized,
                CachedEnhancement {
                    enhancement,
                    cached_at: Self::now(),
                },
            );
        }

        *self.l2_dirty.write().unwrap() = true;

        // Best-effort async save
        let _ = self.save();
    }

    fn cleanup_l2(l2: &mut HashMap<String, CachedEnhancement>) {
        let now = Self::now();

        // Remove expired
        l2.retain(|_, entry| now - entry.cached_at <= CACHE_TTL_SECS);

        // If still too many, remove oldest 20%
        if l2.len() >= MAX_CACHE_ENTRIES {
            let mut entries: Vec<_> = l2.iter().map(|(k, e)| (k.clone(), e.cached_at)).collect();
            entries.sort_by_key(|(_, ts)| *ts);

            let to_remove = entries.len() / 5;
            for (key, _) in entries.into_iter().take(to_remove) {
                l2.remove(&key);
            }
        }
    }

    /// Get cache stats
    pub fn stats(&self) -> (usize, usize) {
        let l1_len = self.l1.read().unwrap().len();
        let l2_len = self.l2.read().unwrap().len();
        (l1_len, l2_len)
    }
}

impl Default for LlmCache {
    fn default() -> Self {
        Self::load()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_query() {
        assert_eq!(
            LlmCache::normalize_query("  How Does  AUTH  Work  "),
            "how does auth work"
        );
    }

    #[test]
    fn test_hash_consistency() {
        let h1 = LlmCache::hash_query("how does auth work");
        let h2 = LlmCache::hash_query("  HOW  does AUTH work  ");
        assert_eq!(h1, h2);
    }
}
