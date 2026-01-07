//! Local query expansion without LLM
//!
//! Builds synonym maps from indexed symbols for instant query expansion.
//! Falls back to LLM only for truly ambiguous natural language queries.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use tracing::{debug, info};

use crate::index::IndexSearcher;

/// Common code-related synonyms that don't need LLM
const BUILTIN_SYNONYMS: &[(&str, &[&str])] = &[
    // Authentication
    ("auth", &["authenticate", "authentication", "login", "logout", "session", "token", "credential", "password", "jwt", "oauth"]),
    ("login", &["auth", "authenticate", "signin", "sign_in", "logon"]),
    ("logout", &["signout", "sign_out", "logoff"]),
    
    // Errors
    ("error", &["err", "exception", "failure", "fail", "panic", "crash", "bug"]),
    ("handle", &["handler", "handling", "catch", "process"]),
    ("try", &["catch", "except", "result", "option"]),
    
    // CRUD
    ("create", &["new", "add", "insert", "make", "build", "init", "initialize"]),
    ("read", &["get", "fetch", "load", "find", "query", "select", "retrieve"]),
    ("update", &["edit", "modify", "change", "set", "patch", "put"]),
    ("delete", &["remove", "destroy", "drop", "clear", "purge"]),
    
    // Data
    ("database", &["db", "sql", "postgres", "mysql", "sqlite", "mongo", "redis", "store", "storage"]),
    ("cache", &["cached", "caching", "memoize", "lru"]),
    ("config", &["configuration", "settings", "options", "preferences", "env"]),
    
    // API
    ("api", &["endpoint", "route", "handler", "controller", "rest", "graphql"]),
    ("request", &["req", "http", "fetch", "call"]),
    ("response", &["res", "reply", "result"]),
    
    // Async
    ("async", &["await", "promise", "future", "concurrent", "parallel", "thread"]),
    ("sync", &["synchronous", "blocking", "sequential"]),
    
    // Testing
    ("test", &["spec", "unittest", "integration", "e2e", "mock", "stub", "fixture"]),
    
    // Common patterns
    ("parse", &["parser", "parsing", "deserialize", "decode"]),
    ("serialize", &["encode", "stringify", "marshal"]),
    ("validate", &["validation", "validator", "check", "verify", "sanitize"]),
    ("transform", &["convert", "map", "translate"]),
    
    // UI
    ("component", &["widget", "element", "view"]),
    ("render", &["display", "draw", "paint", "show"]),
    ("style", &["css", "theme", "design"]),
    
    // File operations
    ("file", &["fs", "path", "directory", "folder", "io"]),
    ("read", &["load", "open", "parse"]),
    ("write", &["save", "store", "output"]),
];

/// Intent patterns for local detection
const INTENT_PATTERNS: &[(&str, &[&str], &str)] = &[
    // (intent, trigger words, expansion suffix)
    ("find_definition", &["find", "where", "locate", "show", "get"], "definition declaration impl"),
    ("find_usage", &["usage", "used", "uses", "call", "calls", "reference"], "usage reference call invoke"),
    ("understand_flow", &["how", "flow", "work", "process", "explain"], "flow process logic implementation"),
    ("find_error", &["error", "bug", "fix", "issue", "problem", "fail"], "error exception handle catch"),
];

/// Local query expander using indexed symbols and builtin synonyms
pub struct LocalExpander {
    /// Synonyms from builtin + extracted from index
    synonyms: HashMap<String, HashSet<String>>,
    /// All known symbols from the index
    known_symbols: HashSet<String>,
}

impl LocalExpander {
    /// Create a new local expander with builtin synonyms
    pub fn new() -> Self {
        let mut synonyms: HashMap<String, HashSet<String>> = HashMap::new();
        
        // Load builtin synonyms
        for (key, values) in BUILTIN_SYNONYMS {
            let set = synonyms.entry(key.to_string()).or_default();
            for v in *values {
                set.insert(v.to_string());
            }
            // Also add reverse mappings
            for v in *values {
                let reverse_set = synonyms.entry(v.to_string()).or_default();
                reverse_set.insert(key.to_string());
            }
        }
        
        Self {
            synonyms,
            known_symbols: HashSet::new(),
        }
    }

    /// Load symbols from an indexed project to enhance expansion
    pub fn load_from_index(&mut self, project_path: &Path) -> Result<(), crate::error::GreppyError> {
        let searcher = IndexSearcher::open(project_path)?;
        
        // Search for common terms to extract symbols
        let common_queries = ["", "a", "e", "i", "o", "u", "s", "t", "n", "r"];
        
        for query in common_queries {
            if let Ok(results) = searcher.search(query, 100) {
                for result in results {
                    if let Some(ref name) = result.symbol_name {
                        // Add symbol name
                        self.known_symbols.insert(name.to_lowercase());
                        
                        // Extract words from camelCase/snake_case
                        let words = split_identifier(name);
                        for word in &words {
                            if word.len() >= 3 {
                                self.known_symbols.insert(word.clone());
                            }
                        }
                        
                        // Build synonym relationships between words in same symbol
                        if words.len() > 1 {
                            for word in &words {
                                let set = self.synonyms.entry(word.clone()).or_default();
                                for other in &words {
                                    if word != other && other.len() >= 3 {
                                        set.insert(other.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        info!("Loaded {} symbols, {} synonym groups from index", 
              self.known_symbols.len(), self.synonyms.len());
        
        Ok(())
    }

    /// Check if we can expand locally without LLM
    pub fn can_expand_locally(&self, query: &str) -> bool {
        let words: Vec<&str> = query.split_whitespace().collect();
        
        // If query contains known symbols, we can expand locally
        for word in &words {
            let lower = word.to_lowercase();
            if self.known_symbols.contains(&lower) {
                return true;
            }
            if self.synonyms.contains_key(&lower) {
                return true;
            }
        }
        
        // Check for intent patterns
        let lower_query = query.to_lowercase();
        for (_, triggers, _) in INTENT_PATTERNS {
            for trigger in *triggers {
                if lower_query.contains(trigger) {
                    return true;
                }
            }
        }
        
        false
    }

    /// Expand query locally without LLM
    pub fn expand(&self, query: &str) -> LocalExpansion {
        let words: Vec<String> = query
            .split_whitespace()
            .map(|s| s.to_lowercase())
            .collect();
        
        let mut expanded_terms: HashSet<String> = HashSet::new();
        let mut detected_intent = "general".to_string();
        
        // Add original words
        for word in &words {
            expanded_terms.insert(word.clone());
        }
        
        // Detect intent from patterns
        let lower_query = query.to_lowercase();
        for (intent, triggers, expansion) in INTENT_PATTERNS {
            for trigger in *triggers {
                if lower_query.contains(trigger) {
                    detected_intent = intent.to_string();
                    for term in expansion.split_whitespace() {
                        expanded_terms.insert(term.to_string());
                    }
                    break;
                }
            }
        }
        
        // Expand each word using synonyms
        for word in &words {
            if let Some(syns) = self.synonyms.get(word) {
                for syn in syns {
                    expanded_terms.insert(syn.clone());
                }
            }
            
            // Also check if word is part of a known symbol
            for symbol in &self.known_symbols {
                if symbol.contains(word) && symbol != word {
                    expanded_terms.insert(symbol.clone());
                }
            }
        }
        
        // Build expanded query string
        let expanded_query: Vec<String> = expanded_terms.into_iter().collect();
        
        debug!("Local expansion: '{}' -> '{}'", query, expanded_query.join(" "));
        
        LocalExpansion {
            intent: detected_intent,
            expanded_query: expanded_query.join(" "),
            used_llm: false,
        }
    }
}

impl Default for LocalExpander {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of local query expansion
#[derive(Debug, Clone)]
pub struct LocalExpansion {
    pub intent: String,
    pub expanded_query: String,
    pub used_llm: bool,
}

/// Split camelCase or snake_case identifier into words
fn split_identifier(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    
    for ch in name.chars() {
        if ch == '_' || ch == '-' {
            if !current.is_empty() {
                words.push(current.to_lowercase());
                current.clear();
            }
        } else if ch.is_uppercase() && !current.is_empty() {
            words.push(current.to_lowercase());
            current.clear();
            current.push(ch);
        } else {
            current.push(ch);
        }
    }
    
    if !current.is_empty() {
        words.push(current.to_lowercase());
    }
    
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_identifier() {
        assert_eq!(split_identifier("getUserById"), vec!["get", "user", "by", "id"]);
        assert_eq!(split_identifier("get_user_by_id"), vec!["get", "user", "by", "id"]);
        assert_eq!(split_identifier("HTTPRequest"), vec!["h", "t", "t", "p", "request"]);
        assert_eq!(split_identifier("parseJSON"), vec!["parse", "j", "s", "o", "n"]);
    }

    #[test]
    fn test_builtin_synonyms() {
        let expander = LocalExpander::new();
        
        // Auth should expand
        let result = expander.expand("auth");
        assert!(result.expanded_query.contains("login"));
        assert!(result.expanded_query.contains("token"));
        
        // Error should expand
        let result = expander.expand("error handling");
        assert!(result.expanded_query.contains("exception"));
        assert!(result.expanded_query.contains("catch"));
    }

    #[test]
    fn test_intent_detection() {
        let expander = LocalExpander::new();
        
        let result = expander.expand("how does auth work");
        assert_eq!(result.intent, "understand_flow");
        
        let result = expander.expand("find the user class");
        assert_eq!(result.intent, "find_definition");
    }

    #[test]
    fn test_can_expand_locally() {
        let expander = LocalExpander::new();
        
        assert!(expander.can_expand_locally("auth login"));
        assert!(expander.can_expand_locally("how does it work"));
        assert!(expander.can_expand_locally("error handling"));
    }
}
