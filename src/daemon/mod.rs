//! Background daemon for fast queries
//!
//! The daemon provides:
//! - Sub-millisecond search (indexes kept in memory)
//! - File watching for incremental index updates
//! - Query caching

pub mod cache;
pub mod client;
pub mod process;
pub mod protocol;
pub mod server;
pub mod watcher;
