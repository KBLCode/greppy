//! Trace-specific AI prompts
//!
//! Provides prompts for AI-enhanced trace operations:
//! - Query expansion: "auth" -> [auth, login, authenticate, session, token]
//! - Natural language parsing: "how does auth work" -> [authenticate, login, session]
//! - Result reranking: prioritize most relevant invocation paths
//!
//! @module ai/trace_prompts

// =============================================================================
// SYSTEM PROMPTS
// =============================================================================

/// System prompt for query expansion
/// Converts a symbol name or natural language query into related symbol names
pub const QUERY_EXPANSION_SYSTEM: &str = r#"You are a code symbol query expander. Given a query (either a symbol name or natural language), return a JSON array of related symbol/function names that might be relevant in a codebase.

Rules:
1. Return 5-15 symbol names as a JSON array of strings
2. Include the original query term if it's a valid symbol name
3. Include common variations: camelCase, snake_case, abbreviated forms
4. Include semantically related concepts
5. For natural language queries, extract the key concepts
6. Return ONLY the JSON array, no explanation

Examples:
- "auth" -> ["auth", "authenticate", "login", "logout", "session", "token", "verify", "validateToken", "checkAuth", "isAuthenticated"]
- "how does user creation work" -> ["createUser", "newUser", "registerUser", "addUser", "signup", "register", "userCreation", "insertUser", "saveUser"]
- "validateEmail" -> ["validateEmail", "checkEmail", "isValidEmail", "emailValidator", "verifyEmail", "emailValidation", "isEmail", "parseEmail"]"#;

/// System prompt for reranking trace results
/// Takes invocation paths and reorders by relevance to the query
pub const TRACE_RERANK_SYSTEM: &str = r#"You are a code trace reranker. Given a query and numbered invocation paths, return ONLY a JSON array of path indices ordered by relevance to the query.

Consider these factors when ranking:
1. How directly the path relates to the query concept
2. Paths through main/entry point functions are often more important
3. Shorter, more direct paths may be more relevant
4. Paths involving the queried concept in business logic rank higher

Return the JSON array of indices, most relevant first. Example: [2, 0, 5, 1, 3, 4]"#;

/// System prompt for natural language query understanding
/// Extracts intent and key symbols from natural language
pub const NL_QUERY_SYSTEM: &str = r#"You are a code query analyzer. Given a natural language question about code, extract the key concepts and return a JSON object with:

1. "intent": The type of query (one of: "trace", "refs", "flow", "impact", "dead_code")
2. "symbols": Array of symbol names to search for
3. "filters": Optional filters like file patterns or kinds

Return ONLY the JSON object.

Examples:
- "how is the login function called?" -> {"intent": "trace", "symbols": ["login", "authenticate", "doLogin"], "filters": null}
- "what reads the userId variable?" -> {"intent": "refs", "symbols": ["userId", "user_id"], "filters": {"kind": "read"}}
- "what would break if I change validateUser?" -> {"intent": "impact", "symbols": ["validateUser"], "filters": null}"#;

// =============================================================================
// USER PROMPT BUILDERS
// =============================================================================

/// Build user prompt for query expansion
pub fn build_expansion_prompt(query: &str) -> String {
    format!(
        "Expand this code query into related symbol names: \"{}\"\n\nReturn ONLY the JSON array.",
        query
    )
}

/// Build user prompt for trace reranking
pub fn build_trace_rerank_prompt(query: &str, paths: &[String]) -> String {
    let mut prompt = format!("Query: {}\n\nInvocation paths:\n", query);
    for (i, path) in paths.iter().enumerate() {
        prompt.push_str(&format!("\n--- Path {} ---\n{}\n", i, path));
    }
    prompt.push_str("\nReturn ONLY the JSON array of indices ordered by relevance.");
    prompt
}

/// Build user prompt for natural language query analysis
pub fn build_nl_query_prompt(query: &str) -> String {
    format!(
        "Analyze this natural language code query: \"{}\"\n\nReturn ONLY the JSON object with intent, symbols, and filters.",
        query
    )
}

// =============================================================================
// RESPONSE TYPES
// =============================================================================

use serde::Deserialize;

/// Response from query expansion
#[derive(Debug, Deserialize)]
pub struct ExpandedQuery {
    /// The original query
    #[serde(skip)]
    pub original: String,
    /// Expanded symbol names to search for
    pub symbols: Vec<String>,
}

/// Response from natural language query analysis
#[derive(Debug, Deserialize)]
pub struct NlQueryAnalysis {
    /// The detected intent
    pub intent: String,
    /// Symbols to search for
    pub symbols: Vec<String>,
    /// Optional filters
    pub filters: Option<NlQueryFilters>,
}

/// Filters extracted from natural language query
#[derive(Debug, Deserialize)]
pub struct NlQueryFilters {
    /// Filter by reference kind (read, write, call, etc.)
    pub kind: Option<String>,
    /// Filter by file pattern
    pub file_pattern: Option<String>,
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Parse expanded query symbols from AI response
pub fn parse_expansion_response(response: &str) -> Vec<String> {
    let text = response.trim();

    // Try direct parse
    if let Ok(symbols) = serde_json::from_str::<Vec<String>>(text) {
        return symbols;
    }

    // Try to find JSON array in the text
    if let Some(start) = text.find('[') {
        if let Some(end) = text.rfind(']') {
            let json_str = &text[start..=end];
            if let Ok(symbols) = serde_json::from_str::<Vec<String>>(json_str) {
                return symbols;
            }
        }
    }

    // Fallback: return empty
    Vec::new()
}

/// Parse reranked indices from AI response
pub fn parse_rerank_response(response: &str, count: usize) -> Vec<usize> {
    let text = response.trim();

    // Try direct parse
    if let Ok(indices) = serde_json::from_str::<Vec<usize>>(text) {
        return indices.into_iter().filter(|&i| i < count).collect();
    }

    // Try to find JSON array in the text
    if let Some(start) = text.find('[') {
        if let Some(end) = text.rfind(']') {
            let json_str = &text[start..=end];
            if let Ok(indices) = serde_json::from_str::<Vec<usize>>(json_str) {
                return indices.into_iter().filter(|&i| i < count).collect();
            }
        }
    }

    // Fallback: return original order
    (0..count).collect()
}

/// Parse natural language query analysis from AI response
pub fn parse_nl_query_response(response: &str) -> Option<NlQueryAnalysis> {
    let text = response.trim();

    // Try direct parse
    if let Ok(analysis) = serde_json::from_str::<NlQueryAnalysis>(text) {
        return Some(analysis);
    }

    // Try to find JSON object in the text
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            let json_str = &text[start..=end];
            if let Ok(analysis) = serde_json::from_str::<NlQueryAnalysis>(json_str) {
                return Some(analysis);
            }
        }
    }

    None
}

/// Check if a query looks like natural language (vs a symbol name)
pub fn is_natural_language_query(query: &str) -> bool {
    // Heuristics for detecting natural language:
    // 1. Contains spaces and common words
    // 2. Starts with question words
    // 3. Contains multiple words

    let query_lower = query.to_lowercase();
    let words: Vec<&str> = query.split_whitespace().collect();

    if words.len() < 2 {
        return false;
    }

    // Check for question words
    let question_words = [
        "how", "what", "where", "when", "why", "which", "who", "does", "is", "are", "can", "show",
        "find", "list",
    ];
    if question_words.iter().any(|&w| query_lower.starts_with(w)) {
        return true;
    }

    // Check for common English words
    let common_words = [
        "the", "a", "an", "to", "from", "in", "of", "for", "with", "by", "all", "every", "any",
    ];
    let common_word_count = words
        .iter()
        .filter(|w| common_words.contains(&w.to_lowercase().as_str()))
        .count();

    common_word_count >= 1 && words.len() >= 3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_natural_language_query() {
        assert!(is_natural_language_query("how does auth work"));
        assert!(is_natural_language_query("what calls the login function"));
        assert!(is_natural_language_query("show all references to userId"));
        assert!(!is_natural_language_query("validateEmail"));
        assert!(!is_natural_language_query("auth"));
        assert!(!is_natural_language_query("UserService"));
    }

    #[test]
    fn test_parse_expansion_response() {
        let response = r#"["auth", "login", "authenticate"]"#;
        let symbols = parse_expansion_response(response);
        assert_eq!(symbols, vec!["auth", "login", "authenticate"]);
    }

    #[test]
    fn test_parse_expansion_response_with_text() {
        let response = r#"Here are the related symbols: ["auth", "login", "authenticate"]"#;
        let symbols = parse_expansion_response(response);
        assert_eq!(symbols, vec!["auth", "login", "authenticate"]);
    }

    #[test]
    fn test_parse_rerank_response() {
        let response = "[2, 0, 1, 3]";
        let indices = parse_rerank_response(response, 5);
        assert_eq!(indices, vec![2, 0, 1, 3]);
    }

    #[test]
    fn test_parse_rerank_response_filters_invalid() {
        let response = "[2, 0, 10, 1, 3]";
        let indices = parse_rerank_response(response, 5);
        assert_eq!(indices, vec![2, 0, 1, 3]); // 10 filtered out
    }

    #[test]
    fn test_parse_nl_query_response() {
        let response = r#"{"intent": "trace", "symbols": ["login"], "filters": null}"#;
        let analysis = parse_nl_query_response(response).unwrap();
        assert_eq!(analysis.intent, "trace");
        assert_eq!(analysis.symbols, vec!["login"]);
    }
}
