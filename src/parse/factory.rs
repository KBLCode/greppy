use crate::parse::{CodeParser, HeuristicParser, TreeSitterParser};
use std::path::Path;

pub fn get_parser(path: &Path) -> Box<dyn CodeParser + Send + Sync> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "rs" => Box::new(TreeSitterParser::new(tree_sitter_rust::language(), "rust")),
        "py" => Box::new(TreeSitterParser::new(
            tree_sitter_python::language(),
            "python",
        )),
        "ts" | "tsx" => Box::new(TreeSitterParser::new(
            tree_sitter_typescript::language_tsx(),
            "tsx",
        )),
        "js" | "jsx" | "mjs" | "cjs" => Box::new(TreeSitterParser::new(
            tree_sitter_javascript::language(),
            "javascript",
        )),
        "go" => Box::new(TreeSitterParser::new(tree_sitter_go::language(), "go")),
        "java" => Box::new(TreeSitterParser::new(tree_sitter_java::language(), "java")),
        _ => Box::new(HeuristicParser),
    }
}
