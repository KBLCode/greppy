//! LLM-powered query enhancement module
//!
//! Uses Claude Haiku to understand search intent and expand queries
//! for better BM25 matching. Includes caching and pre-warming for instant queries.

mod cache;
mod client;
mod local;
mod query;
mod warmup;

pub use cache::LlmCache;
pub use client::ClaudeClient;
pub use local::{LocalExpander, LocalExpansion};
pub use query::{QueryEnhancement, QueryFilters, enhance_query, try_enhance_query, SYSTEM_PROMPT};
pub use warmup::warmup_cache;
