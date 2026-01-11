use crate::config::{CHUNK_MAX_LINES, CHUNK_OVERLAP};
use crate::parse::walker::detect_language;
use std::path::Path;

/// A chunk of code to be indexed
#[derive(Debug, Clone)]
pub struct Chunk {
    pub path: String,
    pub content: String,
    pub symbol_name: Option<String>,
    pub symbol_type: Option<String>,
    pub start_line: usize,
    pub end_line: usize,
    pub language: String,
    pub file_hash: String,
}

impl Chunk {
    /// Generate unique ID for this chunk
    pub fn id(&self) -> String {
        format!("{}:{}:{}", self.path, self.start_line, self.end_line)
    }
}

/// Chunk a file into indexable pieces
pub fn chunk_file(path: &Path, content: &str) -> Vec<Chunk> {
    let language = detect_language(path);
    let file_hash = compute_hash(content);
    let path_str = path.to_string_lossy().to_string();

    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < lines.len() {
        let end = (start + CHUNK_MAX_LINES).min(lines.len());
        let chunk_content = lines[start..end].join("\n");

        // Try to extract symbol name from first non-empty line
        let (symbol_name, symbol_type) = extract_symbol(&lines[start..end]);

        chunks.push(Chunk {
            path: path_str.clone(),
            content: chunk_content,
            symbol_name,
            symbol_type,
            start_line: start + 1, // 1-indexed
            end_line: end,
            language: language.clone(),
            file_hash: file_hash.clone(),
        });

        if end >= lines.len() {
            break;
        }

        start = end.saturating_sub(CHUNK_OVERLAP);
    }

    chunks
}

/// Compute hash of content
fn compute_hash(content: &str) -> String {
    let hash = xxhash_rust::xxh3::xxh3_64(content.as_bytes());
    format!("{:016x}", hash)
}

/// Extract symbol name and type from code lines (simple heuristic)
fn extract_symbol(lines: &[&str]) -> (Option<String>, Option<String>) {
    for line in lines {
        let trimmed = line.trim();

        // Function patterns
        if let Some(name) = extract_function_name(trimmed) {
            return (Some(name), Some("function".to_string()));
        }

        // Class patterns
        if let Some(name) = extract_class_name(trimmed) {
            return (Some(name), Some("class".to_string()));
        }

        // Method patterns
        if let Some(name) = extract_method_name(trimmed) {
            return (Some(name), Some("method".to_string()));
        }
    }

    (None, None)
}

fn extract_function_name(line: &str) -> Option<String> {
    // fn name(
    if line.starts_with("fn ") {
        return line
            .strip_prefix("fn ")?
            .split('(')
            .next()
            .map(|s| s.trim().to_string());
    }

    // function name(
    if line.starts_with("function ") {
        return line
            .strip_prefix("function ")?
            .split('(')
            .next()
            .map(|s| s.trim().to_string());
    }

    // def name(
    if line.starts_with("def ") {
        return line
            .strip_prefix("def ")?
            .split('(')
            .next()
            .map(|s| s.trim().to_string());
    }

    // func name(
    if line.starts_with("func ") {
        return line
            .strip_prefix("func ")?
            .split('(')
            .next()
            .map(|s| s.trim().to_string());
    }

    // const name = (  or  const name = function
    if line.starts_with("const ") || line.starts_with("let ") || line.starts_with("var ") {
        let rest = line.split_whitespace().nth(1)?;
        if line.contains("=>") || line.contains("function") {
            return Some(rest.trim_end_matches(|c| c == '=' || c == ' ').to_string());
        }
    }

    // export function name(
    if line.starts_with("export function ") {
        return line
            .strip_prefix("export function ")?
            .split('(')
            .next()
            .map(|s| s.trim().to_string());
    }

    // export const name =
    if line.starts_with("export const ") && (line.contains("=>") || line.contains("function")) {
        return line
            .strip_prefix("export const ")?
            .split('=')
            .next()
            .map(|s| s.trim().to_string());
    }

    None
}

fn extract_class_name(line: &str) -> Option<String> {
    // class Name
    if line.starts_with("class ") {
        return line
            .strip_prefix("class ")?
            .split(|c| c == ' ' || c == '{' || c == '(' || c == ':')
            .next()
            .map(|s| s.trim().to_string());
    }

    // struct Name
    if line.starts_with("struct ") || line.starts_with("pub struct ") {
        let rest = if line.starts_with("pub ") {
            line.strip_prefix("pub struct ")?
        } else {
            line.strip_prefix("struct ")?
        };
        return rest
            .split(|c| c == ' ' || c == '{' || c == '(' || c == '<')
            .next()
            .map(|s| s.trim().to_string());
    }

    // impl Name
    if line.starts_with("impl ") || line.starts_with("impl<") {
        let rest = line.strip_prefix("impl")?;
        let rest = rest.trim_start_matches(|c: char| c == '<' || c.is_alphanumeric() || c == '_' || c == ',');
        let rest = rest.trim_start_matches('>').trim();
        return rest
            .split(|c| c == ' ' || c == '{' || c == '<')
            .next()
            .map(|s| s.trim().to_string());
    }

    None
}

fn extract_method_name(line: &str) -> Option<String> {
    // pub fn name( or pub async fn name(
    if line.contains("pub ") && line.contains("fn ") {
        let idx = line.find("fn ")?;
        let rest = &line[idx + 3..];
        return rest
            .split('(')
            .next()
            .map(|s| s.trim().to_string());
    }

    // async name( in class context
    if line.trim().starts_with("async ") {
        return line
            .trim()
            .strip_prefix("async ")?
            .split('(')
            .next()
            .map(|s| s.trim().to_string());
    }

    None
}
