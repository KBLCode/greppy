//! Greppy - Sub-millisecond local semantic code search
//!
//! A fast, local code search engine designed for AI coding tools.
//! No cloud, no config, just `greppy search "query"`.

pub mod ai;
pub mod auth;
pub mod cli;
pub mod core;
pub mod daemon;
pub mod index;
pub mod output;
pub mod parse;
pub mod search;
pub mod trace;

pub use core::config::Config;
pub use core::error::{Error, Result};
pub use core::project::Project;
