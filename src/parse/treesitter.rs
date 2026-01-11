use crate::core::config::{CHUNK_MAX_LINES, CHUNK_OVERLAP};
use crate::parse::{Chunk, CodeParser};
use anyhow::Result;
use tree_sitter::{Language, Node, Parser};

pub struct TreeSitterParser {
    language: Language,
    lang_id: String,
}

impl TreeSitterParser {
    pub fn new(language: Language, lang_id: &str) -> Self {
        Self {
            language,
            lang_id: lang_id.to_string(),
        }
    }
}

impl CodeParser for TreeSitterParser {
    fn chunk(&self, path: &str, content: &str) -> Result<Vec<Chunk>> {
        let mut parser = Parser::new();
        parser.set_language(self.language)?;

        let tree = parser
            .parse(content, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse"))?;
        let root_node = tree.root_node();
        let source_bytes = content.as_bytes();

        let mut chunks = Vec::new();
        let mut cursor = root_node.walk();

        // Simple strategy: Iterate over top-level nodes (functions, classes, impls)
        // If a node is too big, we might need to split it further (TODO)
        // If nodes are too small, we might group them (TODO)

        // For now, let's just try to identify "chunkable" nodes
        for child in root_node.children(&mut cursor) {
            let kind = child.kind();

            // We are interested in functions, classes, impl blocks, etc.
            // This is language specific. For Rust:
            // function_item, impl_item, struct_item, enum_item, mod_item

            let is_chunkable = match self.lang_id.as_str() {
                "rust" => matches!(
                    kind,
                    "function_item" | "impl_item" | "struct_item" | "enum_item" | "mod_item"
                ),
                _ => false, // Fallback for other languages if we add them
            };

            if is_chunkable {
                let start_byte = child.start_byte();
                let end_byte = child.end_byte();
                let start_line = child.start_position().row + 1;
                let end_line = child.end_position().row + 1;

                let chunk_content = &content[start_byte..end_byte];

                // Extract symbol name
                let symbol_name = extract_name_from_node(child, content, self.lang_id.as_str());
                let symbol_type = Some(kind.to_string());

                // Compute hash
                let file_hash = format!("{:016x}", xxhash_rust::xxh3::xxh3_64(content.as_bytes()));

                chunks.push(Chunk {
                    path: path.to_string(),
                    content: chunk_content.to_string(),
                    symbol_name,
                    symbol_type,
                    start_line,
                    end_line,
                    language: self.lang_id.clone(),
                    file_hash,
                });
            }
        }

        // If we didn't find any chunks (e.g. script file or unsupported nodes),
        // or if the file is huge and we missed things, we might want to fallback
        // or handle "gaps".
        // For this MVP, let's just return what we found.
        // If empty, maybe fallback to heuristic?

        if chunks.is_empty() {
            // Fallback to heuristic if tree-sitter didn't find "items"
            // e.g. a script with just statements
            use crate::parse::parser::HeuristicParser;
            return HeuristicParser.chunk(path, content);
        }

        Ok(chunks)
    }
}

fn extract_name_from_node(node: Node, source: &str, lang: &str) -> Option<String> {
    match lang {
        "rust" => {
            // For function_item, name is in 'name' field
            if let Some(name_node) = node.child_by_field_name("name") {
                return Some(name_node.utf8_text(source.as_bytes()).ok()?.to_string());
            }
            // For impl_item, it's more complex (type_identifier)
            if node.kind() == "impl_item" {
                if let Some(type_node) = node.child_by_field_name("type") {
                    return Some(type_node.utf8_text(source.as_bytes()).ok()?.to_string());
                }
            }
            None
        }
        _ => None,
    }
}
