use crate::config::{CHUNK_MAX_LINES, CHUNK_OVERLAP};
use crate::error::Result;
use crate::parse::walker::FileWalker;
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
    /// Generate a unique ID for this chunk
    pub fn id(&self) -> String {
        format!("{}:{}:{}", self.path, self.start_line, self.end_line)
    }
}

/// Splits source files into indexable chunks
pub struct Chunker;

impl Chunker {
    /// Chunk a single file into indexable pieces
    pub fn chunk_file(path: &Path, project_root: &Path) -> Result<Vec<Chunk>> {
        let content = std::fs::read_to_string(path)?;
        let relative_path = path
            .strip_prefix(project_root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        
        let language = FileWalker::language_from_path(path);
        let file_hash = Self::hash_content(&content);
        
        let lines: Vec<&str> = content.lines().collect();
        
        if lines.is_empty() {
            return Ok(Vec::new());
        }

        // For small files, just create one chunk
        if lines.len() <= CHUNK_MAX_LINES {
            let symbol_name = Self::extract_primary_symbol(&lines, &language);
            let end_line = lines.len();
            return Ok(vec![Chunk {
                path: relative_path,
                content,
                symbol_name,
                symbol_type: None,
                start_line: 1,
                end_line,
                language,
                file_hash,
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
                path: relative_path.clone(),
                content: chunk_content.clone(),
                symbol_name: Self::extract_primary_symbol(chunk_lines, &language),
                symbol_type: None,
                start_line: start + 1, // 1-indexed
                end_line: end,
                language: language.clone(),
                file_hash: file_hash.clone(),
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

    /// Extract the primary symbol name from a chunk (simple heuristic)
    fn extract_primary_symbol(lines: &[&str], language: &str) -> Option<String> {
        for line in lines {
            let trimmed = line.trim();
            
            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
                continue;
            }

            // Language-specific patterns
            match language {
                "rust" => {
                    if let Some(name) = Self::extract_rust_symbol(trimmed) {
                        return Some(name);
                    }
                }
                "javascript" | "typescript" | "javascriptreact" | "typescriptreact" => {
                    if let Some(name) = Self::extract_js_symbol(trimmed) {
                        return Some(name);
                    }
                }
                "python" => {
                    if let Some(name) = Self::extract_python_symbol(trimmed) {
                        return Some(name);
                    }
                }
                "go" => {
                    if let Some(name) = Self::extract_go_symbol(trimmed) {
                        return Some(name);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn extract_rust_symbol(line: &str) -> Option<String> {
        // fn name, struct Name, enum Name, impl Name, trait Name, mod name
        let patterns = [
            ("fn ", '('),
            ("struct ", ' '),
            ("struct ", '{'),
            ("struct ", ';'),
            ("enum ", ' '),
            ("enum ", '{'),
            ("trait ", ' '),
            ("trait ", '{'),
            ("impl ", ' '),
            ("impl ", '{'),
            ("mod ", ' '),
            ("mod ", '{'),
            ("const ", ':'),
            ("static ", ':'),
            ("type ", ' '),
            ("type ", '='),
        ];

        for (prefix, delimiter) in patterns {
            if let Some(rest) = line.strip_prefix("pub ").or(Some(line)) {
                if let Some(rest) = rest.strip_prefix("pub(crate) ").or(Some(rest)) {
                    if let Some(rest) = rest.strip_prefix("async ").or(Some(rest)) {
                        if let Some(rest) = rest.strip_prefix("unsafe ").or(Some(rest)) {
                            if let Some(after) = rest.strip_prefix(prefix) {
                                let name = after.split(delimiter).next()?.trim();
                                let name = name.split('<').next()?.trim();
                                if !name.is_empty() && name.chars().next()?.is_alphabetic() {
                                    return Some(name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn extract_js_symbol(line: &str) -> Option<String> {
        // function name, class Name, const name =, export function, etc.
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
            let name = rest.split(|c| c == ' ' || c == '{' || c == '<').next()?.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }

        if let Some(rest) = line.strip_prefix("const ").or_else(|| line.strip_prefix("let ")).or_else(|| line.strip_prefix("var ")) {
            let name = rest.split(|c| c == ' ' || c == '=' || c == ':').next()?.trim();
            if !name.is_empty() && name.chars().next()?.is_alphabetic() {
                return Some(name.to_string());
            }
        }

        if let Some(rest) = line.strip_prefix("interface ").or_else(|| line.strip_prefix("type ")) {
            let name = rest.split(|c| c == ' ' || c == '{' || c == '=' || c == '<').next()?.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }

        None
    }

    fn extract_python_symbol(line: &str) -> Option<String> {
        // def name, class Name, async def name
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

    fn extract_go_symbol(line: &str) -> Option<String> {
        // func name, func (r Receiver) name, type Name struct
        if let Some(rest) = line.strip_prefix("func ") {
            // Method with receiver: func (r *Receiver) Name(
            if rest.starts_with('(') {
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
