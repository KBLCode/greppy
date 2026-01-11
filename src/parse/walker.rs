use crate::core::config::MAX_FILE_SIZE;
use crate::core::error::Result;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

/// File info for indexing
pub struct FileInfo {
    pub path: PathBuf,
    pub content: String,
}

/// Walk a project directory, respecting .gitignore
pub fn walk_project(root: &Path) -> Result<Vec<FileInfo>> {
    let mut files = Vec::new();

    let walker = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .max_filesize(Some(MAX_FILE_SIZE))
        .build();

    for entry in walker.flatten() {
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        if !is_code_file(path) {
            continue;
        }

        if let Ok(content) = std::fs::read_to_string(path) {
            files.push(FileInfo {
                path: path.to_path_buf(),
                content,
            });
        }
    }

    Ok(files)
}

/// Check if file is a code file worth indexing
fn is_code_file(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    matches!(
        ext.as_str(),
        "ts" | "tsx"
            | "js"
            | "jsx"
            | "mjs"
            | "cjs"
            | "py"
            | "pyi"
            | "rs"
            | "go"
            | "java"
            | "kt"
            | "kts"
            | "scala"
            | "rb"
            | "php"
            | "c"
            | "h"
            | "cpp"
            | "cc"
            | "cxx"
            | "hpp"
            | "cs"
            | "swift"
            | "ex"
            | "exs"
            | "erl"
            | "hrl"
            | "hs"
            | "ml"
            | "mli"
            | "lua"
            | "sh"
            | "bash"
            | "zsh"
            | "sql"
            | "vue"
            | "svelte"
            | "md"
            | "yaml"
            | "yml"
            | "toml"
            | "json"
    )
}

/// Detect language from file extension
pub fn detect_language(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "ts" | "tsx" => "typescript",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "py" | "pyi" => "python",
        "rs" => "rust",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "scala" => "scala",
        "rb" => "ruby",
        "php" => "php",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        "cs" => "csharp",
        "swift" => "swift",
        "ex" | "exs" => "elixir",
        "erl" | "hrl" => "erlang",
        "hs" => "haskell",
        "ml" | "mli" => "ocaml",
        "lua" => "lua",
        "sh" | "bash" | "zsh" => "shell",
        "sql" => "sql",
        "vue" => "vue",
        "svelte" => "svelte",
        "md" => "markdown",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "json" => "json",
        _ => "unknown",
    }
    .to_string()
}
