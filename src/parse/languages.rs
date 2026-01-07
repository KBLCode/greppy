//! Language detection and tree-sitter grammar loading

use std::path::Path;

/// Supported programming languages with tree-sitter grammars
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    Rust,
    TypeScript,
    TypeScriptReact,
    JavaScript,
    JavaScriptReact,
    Python,
    Go,
    Java,
    C,
    Cpp,
    // Languages without tree-sitter support (fallback to line-based)
    Unknown,
}

impl Language {
    /// Detect language from file path
    pub fn from_path(path: &Path) -> Self {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        match ext.to_lowercase().as_str() {
            "rs" => Language::Rust,
            "ts" => Language::TypeScript,
            "tsx" => Language::TypeScriptReact,
            "js" | "mjs" | "cjs" => Language::JavaScript,
            "jsx" => Language::JavaScriptReact,
            "py" | "pyi" | "pyw" => Language::Python,
            "go" => Language::Go,
            "java" => Language::Java,
            "c" | "h" => Language::C,
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => Language::Cpp,
            _ => {
                // Check for special filenames
                match filename {
                    "Cargo.toml" | "Cargo.lock" => Language::Unknown, // TOML
                    "package.json" | "tsconfig.json" => Language::Unknown, // JSON
                    _ => Language::Unknown,
                }
            }
        }
    }

    /// Get the language name as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            Language::Rust => "rust",
            Language::TypeScript => "typescript",
            Language::TypeScriptReact => "typescriptreact",
            Language::JavaScript => "javascript",
            Language::JavaScriptReact => "javascriptreact",
            Language::Python => "python",
            Language::Go => "go",
            Language::Java => "java",
            Language::C => "c",
            Language::Cpp => "cpp",
            Language::Unknown => "unknown",
        }
    }

    /// Check if this language has tree-sitter support
    pub fn has_ast_support(&self) -> bool {
        !matches!(self, Language::Unknown)
    }

    /// Get the tree-sitter language for this language
    pub fn tree_sitter_language(&self) -> Option<tree_sitter::Language> {
        match self {
            Language::Rust => Some(tree_sitter_rust::LANGUAGE.into()),
            Language::TypeScript | Language::TypeScriptReact => {
                Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
            }
            Language::JavaScript | Language::JavaScriptReact => {
                Some(tree_sitter_javascript::LANGUAGE.into())
            }
            Language::Python => Some(tree_sitter_python::LANGUAGE.into()),
            Language::Go => Some(tree_sitter_go::LANGUAGE.into()),
            Language::Java => Some(tree_sitter_java::LANGUAGE.into()),
            Language::C => Some(tree_sitter_c::LANGUAGE.into()),
            Language::Cpp => Some(tree_sitter_cpp::LANGUAGE.into()),
            Language::Unknown => None,
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_detection() {
        assert_eq!(Language::from_path(Path::new("foo.rs")), Language::Rust);
        assert_eq!(Language::from_path(Path::new("bar.ts")), Language::TypeScript);
        assert_eq!(Language::from_path(Path::new("baz.tsx")), Language::TypeScriptReact);
        assert_eq!(Language::from_path(Path::new("qux.py")), Language::Python);
        assert_eq!(Language::from_path(Path::new("main.go")), Language::Go);
        assert_eq!(Language::from_path(Path::new("App.java")), Language::Java);
        assert_eq!(Language::from_path(Path::new("unknown.xyz")), Language::Unknown);
    }
}
