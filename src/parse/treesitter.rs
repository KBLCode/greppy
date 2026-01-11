use crate::parse::{Chunk, CodeParser};
use anyhow::Result;
use tree_sitter::{Language, Node, Parser};

pub struct TreeSitterParser {
    language: Language,
    lang_id: String,
    parser: Parser,
}

impl TreeSitterParser {
    pub fn new(language: Language, lang_id: &str) -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(language)
            .expect("Error loading language");
        Self {
            language,
            lang_id: lang_id.to_string(),
            parser,
        }
    }
}

impl CodeParser for TreeSitterParser {
    fn chunk(&mut self, path: &str, content: &str) -> Result<Vec<Chunk>> {
        // Reuse self.parser
        let tree = self
            .parser
            .parse(content, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse"))?;
        let root_node = tree.root_node();

        let mut chunks = Vec::new();
        // Use a recursive function to find chunkable nodes
        self.collect_chunks(root_node, content, path, &mut chunks);

        if chunks.is_empty() {
            use crate::parse::parser::HeuristicParser;
            // HeuristicParser is stateless, so we can just instantiate it
            let mut hp = HeuristicParser;
            return hp.chunk(path, content);
        }

        Ok(chunks)
    }
}

impl TreeSitterParser {
    fn collect_chunks(&self, node: Node, content: &str, path: &str, chunks: &mut Vec<Chunk>) {
        let mut cursor = node.walk();

        // Iterate over all children
        for child in node.children(&mut cursor) {
            let kind = child.kind();

            let is_chunkable = match self.lang_id.as_str() {
                "rust" => matches!(
                    kind,
                    "function_item"
                        | "impl_item"
                        | "struct_item"
                        | "enum_item"
                        | "mod_item"
                        | "trait_item"
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
                "java" => matches!(
                    kind,
                    "method_declaration"
                        | "class_declaration"
                        | "interface_declaration"
                        | "enum_declaration"
                ),
                _ => false,
            };

            if is_chunkable {
                let start_byte = child.start_byte();
                let end_byte = child.end_byte();
                let start_line = child.start_position().row + 1;
                let end_line = child.end_position().row + 1;

                let chunk_content = &content[start_byte..end_byte];

                // If the chunk is massive (e.g. a huge module or class), we might want to recurse INSTEAD of chunking the whole thing.
                // Or do both?
                // For now, let's stick to a simple rule:
                // If it's a "container" type (mod_item, class_definition, impl_item) AND it's large (> 50 lines),
                // we recurse into it to find smaller chunks.
                // If it's small, we keep it as one chunk.
                // Functions are usually kept whole unless huge.

                let line_count = end_line - start_line;
                let is_container = matches!(
                    kind,
                    "mod_item" | "impl_item" | "class_definition" | "class_declaration"
                );

                if is_container && line_count > 50 {
                    // Recurse into container to find smaller pieces
                    self.collect_chunks(child, content, path, chunks);
                } else {
                    // It's a good chunk (function, small class, etc.)
                    let symbol_name = extract_name_from_node(child, content, self.lang_id.as_str());
                    let symbol_type = Some(kind.to_string());
                    let file_hash =
                        format!("{:016x}", xxhash_rust::xxh3::xxh3_64(content.as_bytes()));

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
            } else {
                // If not chunkable (e.g. a block, or just a statement), recurse to find nested chunkables
                // e.g. inside a `mod` that wasn't caught above, or just top level statements
                if child.child_count() > 0 {
                    self.collect_chunks(child, content, path, chunks);
                }
            }
        }
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
        "java" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                return Some(name_node.utf8_text(source.as_bytes()).ok()?.to_string());
            }
            None
        }
        _ => None,
    }
}
