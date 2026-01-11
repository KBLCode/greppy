use crate::core::config::{CHUNK_MAX_LINES, CHUNK_OVERLAP};
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

/// Chunk a file into indexable pieces using semantic heuristics
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
        // Determine the best end line for this chunk
        let end = find_smart_break_point(&lines, start, CHUNK_MAX_LINES);
        
        let chunk_lines = &lines[start..end];
        let chunk_content = chunk_lines.join("\n");

        // Try to extract symbol name from the chunk
        let (symbol_name, symbol_type) = extract_symbol(chunk_lines);

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

        // Calculate next start with overlap
        // If we broke at a clean boundary (e.g., end of function), we might not need overlap,
        // but overlap is safer for search context.
        start = end.saturating_sub(CHUNK_OVERLAP).max(start + 1);
    }

    chunks
}

/// Find a "smart" break point for a chunk
/// Tries to keep functions/classes together or break at logical points
fn find_smart_break_point(lines: &[&str], start: usize, max_lines: usize) -> usize {
    let len = lines.len();
    let hard_limit = (start + max_lines).min(len);
    
    // If we reached the end, just return it
    if hard_limit == len {
        return len;
    }

    // Look for a natural break point between (start + min_lines) and hard_limit
    // We prefer breaking at:
    // 1. Empty lines (paragraph breaks)
    // 2. Lines with 0 indentation (top-level boundaries)
    // 3. Closing braces '}' at start of line
    
    let min_lines = max_lines / 2; // Don't create tiny chunks if possible
    let search_start = (start + min_lines).min(hard_limit);
    
    let mut best_break = hard_limit;
    let mut best_score = 0;

    for i in search_start..hard_limit {
        let line = lines[i];
        let trimmed = line.trim();
        
        let mut score = 0;
        
        // Empty lines are great break points
        if trimmed.is_empty() {
            score += 10;
        }
        
        // Closing braces are good (end of block)
        if trimmed == "}" || trimmed == "};" || trimmed == "];" || trimmed == ")" {
            score += 8;
            // Ideally break AFTER the closing brace
            if i + 1 <= hard_limit {
                // Return i+1 to include the brace in the current chunk
                // But we are returning the exclusive end index.
                // So if we return i+1, lines[start..i+1] includes the brace.
                // Let's check the next line too.
                if i + 1 < len && lines[i+1].trim().is_empty() {
                    score += 5; // Even better if followed by empty line
                }
            }
        }

        // Top-level definitions (0 indentation) often start new blocks
        // So breaking BEFORE them is good.
        if !trimmed.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
            // If it looks like a definition
            if trimmed.starts_with("fn ") || trimmed.starts_with("pub ") || trimmed.starts_with("class ") || trimmed.starts_with("def ") {
                score += 5;
            }
        }

        if score > best_score {
            best_score = score;
            // If we found a closing brace, we want to include it, so break at i+1
            if trimmed.starts_with('}') || trimmed.starts_with(']') || trimmed.starts_with(')') {
                best_break = i + 1;
            } else {
                // Otherwise break at i (exclude this line from current chunk, start next chunk with it)
                best_break = i;
            }
        }
    }

    // If we found a good break point, use it. Otherwise use hard limit.
    if best_score > 0 {
        best_break
    } else {
        hard_limit
    }
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
        let rest = rest
            .trim_start_matches(|c: char| c == '<' || c.is_alphanumeric() || c == '_' || c == ',');
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
        return rest.split('(').next().map(|s| s.trim().to_string());
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
