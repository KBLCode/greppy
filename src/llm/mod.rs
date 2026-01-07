//! LLM-powered query enhancement module
//!
//! Uses Claude Haiku to understand search intent and expand queries
//! for better BM25 matching.

mod client;
mod query;

pub use client::ClaudeClient;
pub use query::{QueryEnhancement, QueryFilters, enhance_query, try_enhance_query};
