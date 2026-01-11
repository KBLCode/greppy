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

        let mut chunks = Vec::new();
        let mut cursor = root_node.walk();

        for child in root_node.children(&mut cursor) {
            let kind = child.kind();

            let is_chunkable = match self.lang_id.as_str() {
                "rust" => matches!(
                    kind,
                    "function_item" | "impl_item" | "struct_item" | "enum_item" | "mod_item"
                ),
                "python" => matches!(kind, "function_definition" | "class_definition"),
                "typescript" | "javascript" | "tsx" => matches!(
                    kind,
                    "function_declaration"
                        | "class_declaration"
                        | "interface_declaration"
                        | "enum_declaration"
                        | "method_definition"
                        | "export_statement"
                ),
                "go" => matches!(
                    kind,
                    "function_declaration" | "method_declaration" | "type_declaration"
                ),
                _ => false,
            };

            if is_chunkable {
                let start_byte = child.start_byte();
                let end_byte = child.end_byte();
                let start_line = child.start_position().row + 1;
                let end_line = child.end_position().row + 1;

                let chunk_content = &content[start_byte..end_byte];

                let symbol_name = extract_name_from_node(child, content, self.lang_id.as_str());
                let symbol_type = Some(kind.to_string());

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

        if chunks.is_empty() {
            use crate::parse::parser::HeuristicParser;
            return HeuristicParser.chunk(path, content);
        }

        Ok(chunks)
    }
}

fn extract_name_from_node(node: Node, source: &str, lang: &str) -> Option<String> {
    match lang {
        "rust" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                return Some(name_node.utf8_text(source.as_bytes()).ok()?.to_string());
            }
            if node.kind() == "impl_item" {
                if let Some(type_node) = node.child_by_field_name("type") {
                    return Some(type_node.utf8_text(source.as_bytes()).ok()?.to_string());
                }
            }
            None
        }
        "python" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                return Some(name_node.utf8_text(source.as_bytes()).ok()?.to_string());
            }
            None
        }
        "typescript" | "javascript" | "tsx" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                return Some(name_node.utf8_text(source.as_bytes()).ok()?.to_string());
            }
            None
        }
        "go" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                return Some(name_node.utf8_text(source.as_bytes()).ok()?.to_string());
            }
            None
        }
        _ => None,
    }
}
