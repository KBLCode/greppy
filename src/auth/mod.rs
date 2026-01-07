//! OAuth authentication module for Anthropic Claude API
//!
//! This module implements OAuth 2.0 PKCE flow for authenticating with Anthropic's
//! Claude API, enabling LLM-powered semantic search features.

mod oauth;
mod storage;

pub use oauth::{authorize, exchange, refresh, get_access_token, is_authenticated};
pub use storage::{OAuthTokens, load_tokens, store_tokens, clear_tokens};
