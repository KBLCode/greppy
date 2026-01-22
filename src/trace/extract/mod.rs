//! Trace Extract Module
//!
//! Unified extraction interface for parsing source files and extracting
//! symbols, calls, references, scopes, and tokens using tree-sitter (primary)
//! with regex fallback for unsupported languages.
//!
//! @module trace/extract

pub mod regex;
pub mod treesitter;

use std::path::Path;

// =============================================================================
// EXTRACTED TYPES
// =============================================================================

/// Kind of symbol extracted from source code
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Interface,
    TypeAlias,
    Constant,
    Variable,
    Module,
    Trait,
    Impl,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Class => "class",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Interface => "interface",
            Self::TypeAlias => "type_alias",
            Self::Constant => "constant",
            Self::Variable => "variable",
            Self::Module => "module",
            Self::Trait => "trait",
            Self::Impl => "impl",
        }
    }
}

/// Kind of reference extracted from source code
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    Read,
    Write,
    Call,
    TypeAnnotation,
    Import,
    Export,
    Construction,
}

impl RefKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Call => "call",
            Self::TypeAnnotation => "type_annotation",
            Self::Import => "import",
            Self::Export => "export",
            Self::Construction => "construction",
        }
    }
}

/// Kind of scope in the AST
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    File,
    Module,
    Class,
    Function,
    Block,
    Loop,
    Conditional,
}

impl ScopeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Module => "module",
            Self::Class => "class",
            Self::Function => "function",
            Self::Block => "block",
            Self::Loop => "loop",
            Self::Conditional => "conditional",
        }
    }
}

/// Kind of token extracted
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Identifier,
    Keyword,
    Operator,
    Literal,
    Comment,
    Unknown,
}

impl TokenKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Identifier => "identifier",
            Self::Keyword => "keyword",
            Self::Operator => "operator",
            Self::Literal => "literal",
            Self::Comment => "comment",
            Self::Unknown => "unknown",
        }
    }
}

// =============================================================================
// EXTRACTED STRUCTURES
// =============================================================================

/// A symbol definition extracted from source code
#[derive(Debug, Clone)]
pub struct ExtractedSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub start_line: u32,
    pub end_line: u32,
    pub start_column: u16,
    pub end_column: u16,
    pub is_exported: bool,
    pub is_async: bool,
    pub parent_symbol: Option<String>,
}

/// A function/method call extracted from source code
#[derive(Debug, Clone)]
pub struct ExtractedCall {
    pub callee_name: String,
    pub line: u32,
    pub column: u16,
    pub containing_symbol: Option<String>,
    pub is_method_call: bool,
    pub receiver: Option<String>,
}

/// A reference to a symbol (variable read/write, type annotation, import)
#[derive(Debug, Clone)]
pub struct ExtractedRef {
    pub name: String,
    pub kind: RefKind,
    pub line: u32,
    pub column: u16,
    pub containing_symbol: Option<String>,
}

/// A scope in the AST hierarchy
#[derive(Debug, Clone)]
pub struct ExtractedScope {
    pub kind: ScopeKind,
    pub name: Option<String>,
    pub start_line: u32,
    pub end_line: u32,
    pub parent_index: Option<usize>,
}

/// A token (identifier) extracted from source code
#[derive(Debug, Clone)]
pub struct ExtractedToken {
    pub name: String,
    pub kind: TokenKind,
    pub line: u32,
    pub column: u16,
}

// =============================================================================
// EXTRACTED DATA CONTAINER
// =============================================================================

/// Complete extraction results from a source file
#[derive(Debug, Clone, Default)]
pub struct ExtractedData {
    pub symbols: Vec<ExtractedSymbol>,
    pub calls: Vec<ExtractedCall>,
    pub references: Vec<ExtractedRef>,
    pub scopes: Vec<ExtractedScope>,
    pub tokens: Vec<ExtractedToken>,
    pub language: String,
    pub extraction_method: ExtractionMethod,
}

/// Method used for extraction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExtractionMethod {
    TreeSitter,
    #[default]
    Regex,
}

impl ExtractionMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TreeSitter => "tree-sitter",
            Self::Regex => "regex",
        }
    }
}

impl ExtractedData {
    /// Create empty extraction result
    pub fn empty(language: &str) -> Self {
        Self {
            language: language.to_string(),
            ..Default::default()
        }
    }

    /// Check if any data was extracted
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
            && self.calls.is_empty()
            && self.references.is_empty()
            && self.tokens.is_empty()
    }

    /// Total number of items extracted
    pub fn total_items(&self) -> usize {
        self.symbols.len()
            + self.calls.len()
            + self.references.len()
            + self.scopes.len()
            + self.tokens.len()
    }
}

// =============================================================================
// EXTRACTION ERROR
// =============================================================================

/// Errors that can occur during extraction
#[derive(Debug, Clone)]
pub enum ExtractError {
    /// Tree-sitter parsing failed
    ParseFailed { language: String, message: String },
    /// Language not supported by tree-sitter
    UnsupportedLanguage { language: String },
    /// IO error reading file
    IoError { message: String },
}

impl std::fmt::Display for ExtractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParseFailed { language, message } => {
                write!(f, "Failed to parse {} code: {}", language, message)
            }
            Self::UnsupportedLanguage { language } => {
                write!(f, "Language '{}' not supported by tree-sitter", language)
            }
            Self::IoError { message } => {
                write!(f, "IO error: {}", message)
            }
        }
    }
}

impl std::error::Error for ExtractError {}

// =============================================================================
// LANGUAGE DETECTION
// =============================================================================

/// Detect language from file path extension
pub fn detect_language(path: &Path) -> &'static str {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| match ext.to_lowercase().as_str() {
            "ts" | "tsx" | "mts" | "cts" => "typescript",
            "js" | "jsx" | "mjs" | "cjs" => "javascript",
            "py" | "pyi" => "python",
            "rs" => "rust",
            "go" => "go",
            "c" | "h" => "c",
            "cpp" | "hpp" | "cc" | "cxx" => "cpp",
            "java" => "java",
            "rb" => "ruby",
            "php" => "php",
            "swift" => "swift",
            "kt" | "kts" => "kotlin",
            "cs" => "csharp",
            "lua" => "lua",
            "sh" | "bash" | "zsh" => "bash",
            "json" => "json",
            "yaml" | "yml" => "yaml",
            "toml" => "toml",
            "md" | "markdown" => "markdown",
            "html" | "htm" => "html",
            "css" | "scss" | "sass" | "less" => "css",
            "sql" => "sql",
            "zig" => "zig",
            "ex" | "exs" => "elixir",
            "erl" | "hrl" => "erlang",
            "hs" | "lhs" => "haskell",
            "ml" | "mli" => "ocaml",
            "scala" | "sc" => "scala",
            "clj" | "cljs" | "cljc" => "clojure",
            "v" | "vh" => "verilog",
            "svelte" => "svelte",
            "vue" => "vue",
            _ => "unknown",
        })
        .unwrap_or("unknown")
}

/// Check if language is supported by tree-sitter
pub fn is_treesitter_supported(language: &str) -> bool {
    matches!(
        language,
        "typescript" | "javascript" | "python" | "rust" | "go"
    )
}

// =============================================================================
// MAIN EXTRACTION FUNCTION
// =============================================================================

/// Extract symbols, calls, references, scopes, and tokens from a source file.
///
/// Uses tree-sitter for supported languages, falls back to regex for others.
/// Tree-sitter provides more accurate extraction with full AST access.
/// Regex extraction is a best-effort fallback with lower confidence.
///
/// # Arguments
/// * `path` - Path to the source file (used for language detection)
/// * `content` - Source code content
/// * `language` - Optional language override (if None, detected from path)
///
/// # Returns
/// ExtractedData containing all extracted information
pub fn extract_file(path: &Path, content: &str, language: Option<&str>) -> ExtractedData {
    let detected_lang = language.unwrap_or_else(|| detect_language(path));

    // Skip empty files
    if content.trim().is_empty() {
        return ExtractedData::empty(detected_lang);
    }

    // Try tree-sitter first for supported languages
    if is_treesitter_supported(detected_lang) {
        match treesitter::extract(content, detected_lang) {
            Ok(mut data) => {
                data.language = detected_lang.to_string();
                data.extraction_method = ExtractionMethod::TreeSitter;
                return data;
            }
            Err(e) => {
                // Log warning and fall back to regex
                tracing::warn!(
                    "Tree-sitter extraction failed for {}: {}, falling back to regex",
                    path.display(),
                    e
                );
            }
        }
    }

    // Fall back to regex extraction
    let mut data = regex::extract(content, detected_lang);
    data.language = detected_lang.to_string();
    data.extraction_method = ExtractionMethod::Regex;
    data
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language(Path::new("foo.ts")), "typescript");
        assert_eq!(detect_language(Path::new("foo.tsx")), "typescript");
        assert_eq!(detect_language(Path::new("foo.js")), "javascript");
        assert_eq!(detect_language(Path::new("foo.py")), "python");
        assert_eq!(detect_language(Path::new("foo.rs")), "rust");
        assert_eq!(detect_language(Path::new("foo.go")), "go");
        assert_eq!(detect_language(Path::new("foo.xyz")), "unknown");
    }

    #[test]
    fn test_is_treesitter_supported() {
        assert!(is_treesitter_supported("typescript"));
        assert!(is_treesitter_supported("javascript"));
        assert!(is_treesitter_supported("python"));
        assert!(is_treesitter_supported("rust"));
        assert!(is_treesitter_supported("go"));
        assert!(!is_treesitter_supported("ruby"));
        assert!(!is_treesitter_supported("unknown"));
    }

    #[test]
    fn test_extracted_data_empty() {
        let data = ExtractedData::empty("rust");
        assert!(data.is_empty());
        assert_eq!(data.total_items(), 0);
        assert_eq!(data.language, "rust");
    }
}
