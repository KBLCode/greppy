//! Code chunking for indexing
//!
//! This module provides two chunking strategies:
//! 1. AST-aware chunking: Uses tree-sitter to extract semantic code units
//! 2. Line-based chunking: Fallback for unsupported languages

use crate::config::{CHUNK_MAX_LINES, CHUNK_OVERLAP};
use crate::error::Result;
use crate::parse::ast::AstParser;
use crate::parse::languages::Language;
use std::path::Path;

/// A chunk of code to be indexed
#[derive(Debug, Clone)]
pub struct Chunk {
    /// Relative path to the file
    pub path: String,
    /// The code content
    pub content: String,
    /// Symbol name if this chunk represents a symbol
    pub symbol_name: Option<String>,
    /// Symbol type (function, class, method, etc.)
    pub symbol_type: Option<String>,
    /// Start line number (1-indexed)
    pub start_line: usize,
    /// End line number (1-indexed)
    pub end_line: usize,
    /// Programming language
    pub language: String,
    /// File content hash for change detection
    pub file_hash: String,
    /// Function/method signature
    pub signature: Option<String>,
    /// Parent symbol (for methods in classes)
    pub parent_symbol: Option<String>,
    /// Documentation comment
    pub doc_comment: Option<String>,
    /// Whether this symbol is exported/public
    pub is_exported: bool,
    /// Whether this is a test function
    pub is_test: bool,
}

impl Chunk {
    /// Generate a unique ID for this chunk
    pub fn id(&self) -> String {
        format!("{}:{}:{}", self.path, self.start_line, self.end_line)
    }
}

/// Splits source files into indexable chunks
pub struct Chunker;

impl Chunker {
    /// Chunk a single file into indexable pieces
    ///
    /// Uses AST-aware chunking for supported languages, falls back to
    /// line-based chunking for unsupported languages.
    pub fn chunk_file(path: &Path, project_root: &Path) -> Result<Vec<Chunk>> {
        // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path
        // Path comes from our own FileWalker (ignore crate), not user input
        let content = std::fs::read_to_string(path)?;
        let relative_path = path
            .strip_prefix(project_root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let language = Language::from_path(path);
        let file_hash = Self::hash_content(&content);

        // Try AST-aware chunking first
        if language.has_ast_support() {
            if let Ok(chunks) = Self::chunk_with_ast(&content, &relative_path, language, &file_hash)
            {
                if !chunks.is_empty() {
                    return Ok(chunks);
                }
            }
        }

        // Fall back to line-based chunking
        Self::chunk_by_lines(&content, &relative_path, language, &file_hash)
    }

    /// Chunk using AST parsing
    fn chunk_with_ast(
        content: &str,
        relative_path: &str,
        language: Language,
        file_hash: &str,
    ) -> Result<Vec<Chunk>> {
        let mut parser = AstParser::new(language).ok_or_else(|| {
            crate::error::GreppyError::Parse(format!("No parser for language: {}", language))
        })?;

        let symbols = parser.parse(content)?;

        if symbols.is_empty() {
            return Ok(Vec::new());
        }

        let mut chunks = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        for symbol in symbols {
            // Get the content for this symbol
            let symbol_content = &content[symbol.start_byte..symbol.end_byte];

            // Skip very small symbols (likely just declarations)
            if symbol_content.lines().count() < 2 {
                continue;
            }

            chunks.push(Chunk {
                path: relative_path.to_string(),
                content: symbol_content.to_string(),
                symbol_name: Some(symbol.name.clone()),
                symbol_type: Some(symbol.kind.as_str().to_string()),
                start_line: symbol.start_line + 1, // Convert to 1-indexed
                end_line: symbol.end_line + 1,
                language: language.as_str().to_string(),
                file_hash: file_hash.to_string(),
                signature: symbol.signature,
                parent_symbol: symbol.parent,
                doc_comment: symbol.doc_comment,
                is_exported: symbol.is_exported,
                is_test: symbol.is_test,
            });
        }

        // If we found symbols, also add a file-level chunk for imports/top-level code
        if !chunks.is_empty() {
            // Find the first symbol's start line
            let first_symbol_line = chunks.iter().map(|c| c.start_line).min().unwrap_or(1);

            // If there's significant content before the first symbol, add it as a chunk
            if first_symbol_line > 3 {
                let header_lines: Vec<&str> =
                    lines.iter().take(first_symbol_line - 1).copied().collect();
                let header_content = header_lines.join("\n");

                if !header_content.trim().is_empty() {
                    chunks.insert(
                        0,
                        Chunk {
                            path: relative_path.to_string(),
                            content: header_content,
                            symbol_name: Some("imports".to_string()),
                            symbol_type: Some("module".to_string()),
                            start_line: 1,
                            end_line: first_symbol_line - 1,
                            language: language.as_str().to_string(),
                            file_hash: file_hash.to_string(),
                            signature: None,
                            parent_symbol: None,
                            doc_comment: None,
                            is_exported: false,
                            is_test: false,
                        },
                    );
                }
            }
        }

        Ok(chunks)
    }

    /// Chunk by lines (fallback for unsupported languages)
    fn chunk_by_lines(
        content: &str,
        relative_path: &str,
        language: Language,
        file_hash: &str,
    ) -> Result<Vec<Chunk>> {
        let lines: Vec<&str> = content.lines().collect();

        if lines.is_empty() {
            return Ok(Vec::new());
        }

        // For small files, just create one chunk
        if lines.len() <= CHUNK_MAX_LINES {
            let symbol_name = Self::extract_primary_symbol_simple(&lines, &language);
            return Ok(vec![Chunk {
                path: relative_path.to_string(),
                content: content.to_string(),
                symbol_name,
                symbol_type: None,
                start_line: 1,
                end_line: lines.len(),
                language: language.as_str().to_string(),
                file_hash: file_hash.to_string(),
                signature: None,
                parent_symbol: None,
                doc_comment: None,
                is_exported: false,
                is_test: false,
            }]);
        }

        // For larger files, create overlapping chunks
        let mut chunks = Vec::new();
        let mut start = 0;

        while start < lines.len() {
            let end = (start + CHUNK_MAX_LINES).min(lines.len());
            let chunk_lines = &lines[start..end];
            let chunk_content = chunk_lines.join("\n");

            chunks.push(Chunk {
                path: relative_path.to_string(),
                content: chunk_content,
                symbol_name: Self::extract_primary_symbol_simple(chunk_lines, &language),
                symbol_type: None,
                start_line: start + 1, // 1-indexed
                end_line: end,
                language: language.as_str().to_string(),
                file_hash: file_hash.to_string(),
                signature: None,
                parent_symbol: None,
                doc_comment: None,
                is_exported: false,
                is_test: false,
            });

            // Move forward, but overlap with previous chunk
            start = end.saturating_sub(CHUNK_OVERLAP);
            if start >= lines.len() - CHUNK_OVERLAP {
                break;
            }
        }

        Ok(chunks)
    }

    /// Hash file content for change detection
    fn hash_content(content: &str) -> String {
        format!("{:016x}", xxhash_rust::xxh3::xxh3_64(content.as_bytes()))
    }

    /// Simple symbol extraction for line-based chunking (fallback)
    fn extract_primary_symbol_simple(lines: &[&str], language: &Language) -> Option<String> {
        for line in lines {
            let trimmed = line.trim();

            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
                continue;
            }

            match language {
                Language::Rust => {
                    if let Some(name) = Self::extract_rust_symbol_simple(trimmed) {
                        return Some(name);
                    }
                }
                Language::JavaScript
                | Language::JavaScriptReact
                | Language::TypeScript
                | Language::TypeScriptReact => {
                    if let Some(name) = Self::extract_js_symbol_simple(trimmed) {
                        return Some(name);
                    }
                }
                Language::Python => {
                    if let Some(name) = Self::extract_python_symbol_simple(trimmed) {
                        return Some(name);
                    }
                }
                Language::Go => {
                    if let Some(name) = Self::extract_go_symbol_simple(trimmed) {
                        return Some(name);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn extract_rust_symbol_simple(line: &str) -> Option<String> {
        let patterns = [
            ("fn ", '('),
            ("struct ", ' '),
            ("struct ", '{'),
            ("enum ", ' '),
            ("enum ", '{'),
            ("trait ", ' '),
            ("impl ", ' '),
            ("mod ", ' '),
        ];

        let line = line.strip_prefix("pub ").unwrap_or(line);
        let line = line.strip_prefix("pub(crate) ").unwrap_or(line);
        let line = line.strip_prefix("async ").unwrap_or(line);

        for (prefix, delimiter) in patterns {
            if let Some(after) = line.strip_prefix(prefix) {
                let name = after.split(delimiter).next()?.trim();
                let name = name.split('<').next()?.trim();
                if !name.is_empty() && name.chars().next()?.is_alphabetic() {
                    return Some(name.to_string());
                }
            }
        }
        None
    }

    fn extract_js_symbol_simple(line: &str) -> Option<String> {
        let line = line.strip_prefix("export ").unwrap_or(line);
        let line = line.strip_prefix("default ").unwrap_or(line);
        let line = line.strip_prefix("async ").unwrap_or(line);

        if let Some(rest) = line.strip_prefix("function ") {
            let name = rest.split('(').next()?.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }

        if let Some(rest) = line.strip_prefix("class ") {
            let name = rest
                .split(|c| c == ' ' || c == '{' || c == '<')
                .next()?
                .trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }

        if let Some(rest) = line
            .strip_prefix("const ")
            .or_else(|| line.strip_prefix("let "))
            .or_else(|| line.strip_prefix("var "))
        {
            let name = rest
                .split(|c| c == ' ' || c == '=' || c == ':')
                .next()?
                .trim();
            if !name.is_empty() && name.chars().next()?.is_alphabetic() {
                return Some(name.to_string());
            }
        }

        None
    }

    fn extract_python_symbol_simple(line: &str) -> Option<String> {
        let line = line.strip_prefix("async ").unwrap_or(line);

        if let Some(rest) = line.strip_prefix("def ") {
            let name = rest.split('(').next()?.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }

        if let Some(rest) = line.strip_prefix("class ") {
            let name = rest.split(|c| c == '(' || c == ':').next()?.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }

        None
    }

    fn extract_go_symbol_simple(line: &str) -> Option<String> {
        if let Some(rest) = line.strip_prefix("func ") {
            if rest.starts_with('(') {
                // Method with receiver
                if let Some(after_receiver) = rest.split(')').nth(1) {
                    let name = after_receiver.trim().split('(').next()?.trim();
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
            } else {
                let name = rest.split('(').next()?.trim();
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }

        if let Some(rest) = line.strip_prefix("type ") {
            let name = rest.split(|c| c == ' ' || c == '{').next()?.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_rust_ast_chunking() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");

        let content = r#"
use std::io;

/// Authenticates a user
pub fn authenticate(user: &User) -> Result<Token, Error> {
    validate_token(user.token)
}

pub struct User {
    name: String,
    token: String,
}

impl User {
    pub fn new(name: String) -> Self {
        Self { name, token: String::new() }
    }
}
"#;
        std::fs::write(&file_path, content).unwrap();

        let chunks = Chunker::chunk_file(&file_path, temp_dir.path()).unwrap();

        // Should have chunks for: imports, authenticate, User, impl User, new
        assert!(chunks.len() >= 3);

        // Check that we found the authenticate function
        let auth_chunk = chunks
            .iter()
            .find(|c| c.symbol_name.as_deref() == Some("authenticate"));
        assert!(auth_chunk.is_some());
        let auth = auth_chunk.unwrap();
        assert_eq!(auth.symbol_type.as_deref(), Some("function"));
        assert!(auth.is_exported);
    }

    #[test]
    fn test_typescript_ast_chunking() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.ts");

        let content = r#"
import { User } from './types';

export function authenticate(user: User): Token {
    return validateToken(user.token);
}

export class AuthService {
    private secret: string;

    constructor(secret: string) {
        this.secret = secret;
    }

    validate(token: string): boolean {
        return true;
    }
}
"#;
        std::fs::write(&file_path, content).unwrap();

        let chunks = Chunker::chunk_file(&file_path, temp_dir.path()).unwrap();

        assert!(chunks.len() >= 2);

        // Check for authenticate function
        let auth_chunk = chunks
            .iter()
            .find(|c| c.symbol_name.as_deref() == Some("authenticate"));
        assert!(auth_chunk.is_some());

        // Check for AuthService class
        let class_chunk = chunks
            .iter()
            .find(|c| c.symbol_name.as_deref() == Some("AuthService"));
        assert!(class_chunk.is_some());
    }

    #[test]
    fn test_line_based_fallback() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.xyz"); // Unknown extension

        let content = "line1\nline2\nline3\nline4\nline5";
        std::fs::write(&file_path, content).unwrap();

        let chunks = Chunker::chunk_file(&file_path, temp_dir.path()).unwrap();

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].language, "unknown");
    }
}
