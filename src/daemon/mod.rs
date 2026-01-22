//! Background daemon for fast queries
//!
//! The daemon provides:
//! - Sub-millisecond search (indexes kept in memory)
//! - File watching for incremental index updates
//! - Query caching
//! - Event broadcasting for real-time updates

pub mod cache;
pub mod client;
pub mod events;
pub mod process;
pub mod protocol;
pub mod server;
pub mod watcher;
