//! Code Context Engine
//!
//! Provides file caching and code context extraction for trace operations.
//! This is the foundation for showing actual code instead of `// line X`.
//!
//! @module trace/context

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

// =============================================================================
// TYPES
// =============================================================================

/// Code context around a specific line
#[derive(Debug, Clone)]
pub struct CodeContext {
    /// The target line of code (trimmed)
    pub line: String,
    /// Lines before the target (in order)
    pub before: Vec<String>,
    /// Lines after the target (in order)
    pub after: Vec<String>,
    /// The line number of the target
    pub line_number: u32,
    /// Column position (for highlighting)
    pub column: Option<u16>,
}

impl CodeContext {
    /// Get formatted output with line numbers
    pub fn format(&self, highlight_column: bool) -> String {
        let mut output = String::new();
        let start_line = self.line_number.saturating_sub(self.before.len() as u32);

        // Before lines
        for (i, line) in self.before.iter().enumerate() {
            let ln = start_line + i as u32;
            output.push_str(&format!("  {:>4}: {}\n", ln, line));
        }

        // Target line with marker
        output.push_str(&format!("> {:>4}: {}\n", self.line_number, self.line));

        // Column indicator if requested
        if highlight_column {
            if let Some(col) = self.column {
                let padding = 8 + col as usize; // "> NNNN: " = 8 chars
                output.push_str(&format!("{}^\n", " ".repeat(padding)));
            }
        }

        // After lines
        let after_start = self.line_number + 1;
        for (i, line) in self.after.iter().enumerate() {
            let ln = after_start + i as u32;
            output.push_str(&format!("  {:>4}: {}\n", ln, line));
        }

        output
    }

    /// Get just the line content (no formatting)
    pub fn line_content(&self) -> &str {
        &self.line
    }
}

// =============================================================================
// FILE CACHE
// =============================================================================

/// LRU cache for file contents
///
/// Caches file contents as line vectors for fast line access.
/// Uses a simple eviction strategy when memory limit is reached.
pub struct FileCache {
    /// Cached file contents (path -> lines)
    cache: HashMap<PathBuf, Vec<String>>,
    /// Total bytes cached (approximate)
    bytes_cached: usize,
    /// Maximum bytes to cache
    max_bytes: usize,
    /// Project root for resolving relative paths
    project_root: PathBuf,
}

impl FileCache {
    /// Create a new file cache with default 16MB limit
    pub fn new(project_root: impl AsRef<Path>) -> Self {
        Self::with_capacity(project_root, 16 * 1024 * 1024)
    }

    /// Create with custom memory limit
    pub fn with_capacity(project_root: impl AsRef<Path>, max_bytes: usize) -> Self {
        Self {
            cache: HashMap::new(),
            bytes_cached: 0,
            max_bytes,
            project_root: project_root.as_ref().to_path_buf(),
        }
    }

    /// Resolve a path (handles relative paths from index)
    fn resolve_path(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.project_root.join(path)
        }
    }

    /// Load file into cache if not already present
    fn ensure_loaded(&mut self, path: &Path) -> Option<&Vec<String>> {
        let resolved = self.resolve_path(path);

        if !self.cache.contains_key(&resolved) {
            // Try to load the file
            let content = fs::read_to_string(&resolved).ok()?;
            let bytes = content.len();

            // Evict if needed
            while self.bytes_cached + bytes > self.max_bytes && !self.cache.is_empty() {
                // Simple eviction: remove first entry
                if let Some(key) = self.cache.keys().next().cloned() {
                    if let Some(lines) = self.cache.remove(&key) {
                        self.bytes_cached -= lines.iter().map(|l| l.len()).sum::<usize>();
                    }
                }
            }

            // Parse into lines
            let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
            self.bytes_cached += bytes;
            self.cache.insert(resolved.clone(), lines);
        }

        self.cache.get(&resolved)
    }

    /// Get a single line from a file (1-indexed)
    pub fn get_line(&mut self, path: &Path, line: u32) -> Option<String> {
        let lines = self.ensure_loaded(path)?;
        let idx = line.saturating_sub(1) as usize;
        lines.get(idx).cloned()
    }

    /// Get multiple lines as a range (1-indexed, inclusive)
    pub fn get_range(&mut self, path: &Path, start: u32, end: u32) -> Option<Vec<String>> {
        let lines = self.ensure_loaded(path)?;
        let start_idx = start.saturating_sub(1) as usize;
        let end_idx = end.min(lines.len() as u32) as usize;

        if start_idx >= lines.len() {
            return None;
        }

        Some(lines[start_idx..end_idx].to_vec())
    }

    /// Get code context around a line
    pub fn get_context(
        &mut self,
        path: &Path,
        line: u32,
        before: u32,
        after: u32,
    ) -> Option<CodeContext> {
        self.get_context_with_column(path, line, None, before, after)
    }

    /// Get code context with column highlighting
    pub fn get_context_with_column(
        &mut self,
        path: &Path,
        line: u32,
        column: Option<u16>,
        before: u32,
        after: u32,
    ) -> Option<CodeContext> {
        let lines = self.ensure_loaded(path)?;
        let idx = line.saturating_sub(1) as usize;

        if idx >= lines.len() {
            return None;
        }

        // Get the target line
        let target_line = lines[idx].clone();

        // Get before lines
        let before_start = idx.saturating_sub(before as usize);
        let before_lines: Vec<String> = lines[before_start..idx].to_vec();

        // Get after lines
        let after_end = (idx + 1 + after as usize).min(lines.len());
        let after_lines: Vec<String> = lines[idx + 1..after_end].to_vec();

        Some(CodeContext {
            line: target_line,
            before: before_lines,
            after: after_lines,
            line_number: line,
            column,
        })
    }

    /// Get the full function/block containing a line
    pub fn get_enclosing_block(
        &mut self,
        path: &Path,
        line: u32,
        max_lines: u32,
    ) -> Option<Vec<String>> {
        let lines = self.ensure_loaded(path)?;
        let idx = line.saturating_sub(1) as usize;

        if idx >= lines.len() {
            return None;
        }

        // Simple heuristic: find enclosing braces
        // Look backwards for function start
        let mut start = idx;
        let mut brace_depth = 0;

        for i in (0..=idx).rev() {
            let l = &lines[i];
            brace_depth += l.matches('}').count() as i32;
            brace_depth -= l.matches('{').count() as i32;

            // Found opening brace at same or lower level
            if brace_depth <= 0 && l.contains('{') {
                start = i;
                break;
            }

            // Don't go too far back
            if idx - i > max_lines as usize / 2 {
                start = i;
                break;
            }
        }

        // Look forwards for function end
        let mut end = idx;
        brace_depth = 0;

        for i in idx..lines.len() {
            let l = &lines[i];
            brace_depth += l.matches('{').count() as i32;
            brace_depth -= l.matches('}').count() as i32;

            // Found closing brace
            if brace_depth <= 0 && l.contains('}') {
                end = i;
                break;
            }

            // Don't go too far forward
            if i - idx > max_lines as usize / 2 {
                end = i;
                break;
            }
        }

        Some(lines[start..=end.min(lines.len() - 1)].to_vec())
    }

    /// Check if a file exists and is readable
    pub fn file_exists(&self, path: &Path) -> bool {
        let resolved = self.resolve_path(path);
        resolved.exists()
    }

    /// Get total lines in a file
    pub fn line_count(&mut self, path: &Path) -> Option<usize> {
        self.ensure_loaded(path).map(|lines| lines.len())
    }

    /// Clear the cache
    pub fn clear(&mut self) {
        self.cache.clear();
        self.bytes_cached = 0;
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            files_cached: self.cache.len(),
            bytes_cached: self.bytes_cached,
            max_bytes: self.max_bytes,
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub files_cached: usize,
    pub bytes_cached: usize,
    pub max_bytes: usize,
}

// =============================================================================
// CONTEXT BUILDER
// =============================================================================

/// Builder for creating code contexts with various options
pub struct ContextBuilder<'a> {
    cache: &'a mut FileCache,
    before_lines: u32,
    after_lines: u32,
    trim_whitespace: bool,
    max_line_length: Option<usize>,
}

impl<'a> ContextBuilder<'a> {
    /// Create a new context builder
    pub fn new(cache: &'a mut FileCache) -> Self {
        Self {
            cache,
            before_lines: 0,
            after_lines: 0,
            trim_whitespace: false,
            max_line_length: None,
        }
    }

    /// Set lines of context before the target
    pub fn before(mut self, lines: u32) -> Self {
        self.before_lines = lines;
        self
    }

    /// Set lines of context after the target
    pub fn after(mut self, lines: u32) -> Self {
        self.after_lines = lines;
        self
    }

    /// Set context on both sides
    pub fn context(mut self, lines: u32) -> Self {
        self.before_lines = lines;
        self.after_lines = lines;
        self
    }

    /// Trim leading/trailing whitespace from lines
    pub fn trim(mut self) -> Self {
        self.trim_whitespace = true;
        self
    }

    /// Truncate lines longer than max
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_line_length = Some(max);
        self
    }

    /// Build context for a specific location
    pub fn build(self, path: &Path, line: u32, column: Option<u16>) -> Option<CodeContext> {
        let mut ctx = self.cache.get_context_with_column(
            path,
            line,
            column,
            self.before_lines,
            self.after_lines,
        )?;

        // Apply transformations
        if self.trim_whitespace {
            ctx.line = ctx.line.trim().to_string();
            ctx.before = ctx.before.iter().map(|s| s.trim().to_string()).collect();
            ctx.after = ctx.after.iter().map(|s| s.trim().to_string()).collect();
        }

        if let Some(max) = self.max_line_length {
            if ctx.line.len() > max {
                ctx.line = format!("{}...", &ctx.line[..max]);
            }
            ctx.before = ctx
                .before
                .iter()
                .map(|s| {
                    if s.len() > max {
                        format!("{}...", &s[..max])
                    } else {
                        s.clone()
                    }
                })
                .collect();
            ctx.after = ctx
                .after
                .iter()
                .map(|s| {
                    if s.len() > max {
                        format!("{}...", &s[..max])
                    } else {
                        s.clone()
                    }
                })
                .collect();
        }

        Some(ctx)
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_file(dir: &TempDir, name: &str, content: &str) -> PathBuf {
        let path = dir.path().join(name);
        let mut file = fs::File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn test_get_line() {
        let dir = TempDir::new().unwrap();
        let path = create_test_file(&dir, "test.rs", "line 1\nline 2\nline 3\nline 4\nline 5\n");

        let mut cache = FileCache::new(dir.path());

        assert_eq!(cache.get_line(&path, 1), Some("line 1".to_string()));
        assert_eq!(cache.get_line(&path, 3), Some("line 3".to_string()));
        assert_eq!(cache.get_line(&path, 5), Some("line 5".to_string()));
        assert_eq!(cache.get_line(&path, 6), None);
    }

    #[test]
    fn test_get_context() {
        let dir = TempDir::new().unwrap();
        let path = create_test_file(
            &dir,
            "test.rs",
            "fn main() {\n    let x = 1;\n    let y = 2;\n    let z = 3;\n}\n",
        );

        let mut cache = FileCache::new(dir.path());

        let ctx = cache.get_context(&path, 3, 1, 1).unwrap();
        assert_eq!(ctx.line, "    let y = 2;");
        assert_eq!(ctx.before.len(), 1);
        assert_eq!(ctx.after.len(), 1);
        assert_eq!(ctx.before[0], "    let x = 1;");
        assert_eq!(ctx.after[0], "    let z = 3;");
    }

    #[test]
    fn test_get_range() {
        let dir = TempDir::new().unwrap();
        let path = create_test_file(&dir, "test.rs", "a\nb\nc\nd\ne\n");

        let mut cache = FileCache::new(dir.path());

        let range = cache.get_range(&path, 2, 4).unwrap();
        assert_eq!(range, vec!["b", "c", "d"]);
    }

    #[test]
    fn test_context_format() {
        let ctx = CodeContext {
            line: "let x = 42;".to_string(),
            before: vec!["fn main() {".to_string()],
            after: vec!["}".to_string()],
            line_number: 2,
            column: Some(4),
        };

        let formatted = ctx.format(true);
        assert!(formatted.contains("> "));
        assert!(formatted.contains("let x = 42;"));
    }

    #[test]
    fn test_cache_eviction() {
        let dir = TempDir::new().unwrap();
        let path1 = create_test_file(&dir, "big1.txt", &"x".repeat(1000));
        let path2 = create_test_file(&dir, "big2.txt", &"y".repeat(1000));

        // Small cache that can only hold one file
        let mut cache = FileCache::with_capacity(dir.path(), 1500);

        // Load first file
        cache.get_line(&path1, 1);
        assert_eq!(cache.stats().files_cached, 1);

        // Load second file - should evict first
        cache.get_line(&path2, 1);
        assert_eq!(cache.stats().files_cached, 1);
    }
}
