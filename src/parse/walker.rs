use crate::config::MAX_FILE_SIZE;
use crate::error::Result;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

/// Walks project files respecting .gitignore
pub struct FileWalker {
    root: PathBuf,
}

impl FileWalker {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    /// Walk all indexable files in the project
    pub fn walk(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        let walker = WalkBuilder::new(&self.root)
            .hidden(true)           // Skip hidden files
            .git_ignore(true)       // Respect .gitignore
            .git_global(true)       // Respect global gitignore
            .git_exclude(true)      // Respect .git/info/exclude
            .require_git(false)     // Work even without .git
            .build();

        for entry in walker.flatten() {
            let path = entry.path();
            
            // Skip directories
            if path.is_dir() {
                continue;
            }

            // Skip files that are too large
            if let Ok(meta) = path.metadata() {
                if meta.len() > MAX_FILE_SIZE {
                    continue;
                }
            }

            // Only index known code files
            if Self::is_code_file(path) {
                files.push(path.to_path_buf());
            }
        }

        Ok(files)
    }

    /// Check if a file is a code file we should index
    fn is_code_file(path: &Path) -> bool {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        matches!(
            ext.to_lowercase().as_str(),
            // Rust
            "rs" |
            // JavaScript/TypeScript
            "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" |
            // Python
            "py" | "pyi" |
            // Go
            "go" |
            // Java/Kotlin
            "java" | "kt" | "kts" |
            // C/C++
            "c" | "h" | "cpp" | "hpp" | "cc" | "cxx" |
            // C#
            "cs" |
            // Ruby
            "rb" |
            // PHP
            "php" |
            // Swift
            "swift" |
            // Scala
            "scala" |
            // Shell
            "sh" | "bash" | "zsh" |
            // Web
            "html" | "css" | "scss" | "sass" | "less" |
            // Config/Data
            "json" | "yaml" | "yml" | "toml" | "xml" |
            // Docs
            "md" | "mdx" |
            // SQL
            "sql" |
            // Elixir/Erlang
            "ex" | "exs" | "erl" |
            // Haskell
            "hs" |
            // Lua
            "lua" |
            // Zig
            "zig" |
            // Nim
            "nim" |
            // V
            "v" |
            // Dart
            "dart" |
            // Vue/Svelte
            "vue" | "svelte"
        )
    }

    /// Get the language from file extension
    pub fn language_from_path(path: &Path) -> String {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        match ext.to_lowercase().as_str() {
            "rs" => "rust",
            "js" | "mjs" | "cjs" => "javascript",
            "jsx" => "javascriptreact",
            "ts" => "typescript",
            "tsx" => "typescriptreact",
            "py" | "pyi" => "python",
            "go" => "go",
            "java" => "java",
            "kt" | "kts" => "kotlin",
            "c" | "h" => "c",
            "cpp" | "hpp" | "cc" | "cxx" => "cpp",
            "cs" => "csharp",
            "rb" => "ruby",
            "php" => "php",
            "swift" => "swift",
            "scala" => "scala",
            "sh" | "bash" | "zsh" => "shell",
            "html" => "html",
            "css" => "css",
            "scss" | "sass" => "scss",
            "less" => "less",
            "json" => "json",
            "yaml" | "yml" => "yaml",
            "toml" => "toml",
            "xml" => "xml",
            "md" | "mdx" => "markdown",
            "sql" => "sql",
            "ex" | "exs" => "elixir",
            "erl" => "erlang",
            "hs" => "haskell",
            "lua" => "lua",
            "zig" => "zig",
            "nim" => "nim",
            "v" => "vlang",
            "dart" => "dart",
            "vue" => "vue",
            "svelte" => "svelte",
            _ => "unknown",
        }
        .to_string()
    }
}
