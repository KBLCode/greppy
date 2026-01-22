//! Tree-sitter Based Extraction
//!
//! High-accuracy AST-based extraction for supported languages.
//! Uses tree-sitter queries to extract symbols, calls, references, and scopes.
//!
//! Supported languages:
//! - TypeScript/TSX
//! - JavaScript/JSX
//! - Python
//! - Rust
//! - Go
//!
//! @module trace/extract/treesitter

use super::{
    ExtractError, ExtractedCall, ExtractedData, ExtractedRef, ExtractedScope, ExtractedSymbol,
    ExtractedToken, ExtractionMethod, RefKind, ScopeKind, SymbolKind, TokenKind,
};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor, Tree};

// =============================================================================
// LANGUAGE PARSERS
// =============================================================================

/// Get or create a parser for the given language
fn get_parser(language: &str) -> Result<Parser, ExtractError> {
    let lang = get_language(language)?;
    let mut parser = Parser::new();
    parser
        .set_language(&lang)
        .map_err(|e| ExtractError::ParseFailed {
            language: language.to_string(),
            message: e.to_string(),
        })?;
    Ok(parser)
}

/// Get tree-sitter language for the given language name
fn get_language(language: &str) -> Result<Language, ExtractError> {
    match language {
        "typescript" => Ok(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "javascript" => Ok(tree_sitter_javascript::LANGUAGE.into()),
        "python" => Ok(tree_sitter_python::LANGUAGE.into()),
        "rust" => Ok(tree_sitter_rust::LANGUAGE.into()),
        "go" => Ok(tree_sitter_go::LANGUAGE.into()),
        _ => Err(ExtractError::UnsupportedLanguage {
            language: language.to_string(),
        }),
    }
}

// =============================================================================
// TREE-SITTER QUERIES
// =============================================================================

/// Query for extracting function definitions
fn function_query(language: &str) -> &'static str {
    match language {
        "typescript" | "javascript" => {
            "(function_declaration name: (identifier) @name) @function
(lexical_declaration (variable_declarator name: (identifier) @name value: (arrow_function))) @function
(method_definition name: (property_identifier) @name) @method
(export_statement declaration: (function_declaration name: (identifier) @name)) @export_function"
        }
        "python" => "(function_definition name: (identifier) @name) @function",
        "rust" => "(function_item name: (identifier) @name) @function",
        "go" => {
            "(function_declaration name: (identifier) @name) @function
(method_declaration name: (field_identifier) @name) @method"
        }
        _ => "",
    }
}

/// Query for extracting class/struct definitions
fn class_query(language: &str) -> &'static str {
    match language {
        // TypeScript/JavaScript class extraction is complex due to grammar differences
        // We rely on regex fallback for these languages
        "typescript" | "javascript" => "",
        "python" => "(class_definition name: (identifier) @name) @class",
        "rust" => {
            // Note: impl blocks are intentionally NOT extracted as symbols
            // They are implementation details, not standalone semantic entities
            "(struct_item name: (type_identifier) @name) @struct
(enum_item name: (type_identifier) @name) @enum
(trait_item name: (type_identifier) @name) @trait"
        }
        "go" => {
            "(type_declaration (type_spec name: (type_identifier) @name type: (struct_type))) @struct
(type_declaration (type_spec name: (type_identifier) @name type: (interface_type))) @interface"
        }
        _ => "",
    }
}

/// Query for extracting function calls
fn call_query(language: &str) -> &'static str {
    match language {
        "typescript" | "javascript" => {
            "(call_expression function: (identifier) @callee) @call
(call_expression function: (member_expression property: (property_identifier) @callee)) @method_call"
        }
        "python" => {
            "(call function: (identifier) @callee) @call
(call function: (attribute attribute: (identifier) @callee)) @method_call"
        }
        "rust" => {
            "(call_expression function: (identifier) @callee) @call
(call_expression function: (field_expression field: (field_identifier) @callee)) @method_call
(macro_invocation macro: (identifier) @callee) @macro_call"
        }
        "go" => {
            "(call_expression function: (identifier) @callee) @call
(call_expression function: (selector_expression field: (field_identifier) @callee)) @method_call"
        }
        _ => "",
    }
}

/// Query for extracting construction patterns (struct literals, enum variants, new expressions)
fn construction_query(language: &str) -> &'static str {
    match language {
        "typescript" | "javascript" => {
            // new ClassName()
            "(new_expression constructor: (identifier) @constructed_type) @construction"
        }
        "python" => {
            // ClassName() - Python class instantiation looks like a function call
            // We detect it by checking if the callee starts with uppercase
            ""
        }
        "rust" => {
            // Struct literal: MyStruct { field: value }
            // Enum variant: MyEnum::Variant or MyEnum::Variant { field }
            // Tuple struct: MyStruct(value)
            "(struct_expression name: (type_identifier) @constructed_type) @construction
(scoped_identifier path: (identifier) @constructed_type) @enum_usage"
        }
        "go" => {
            // Go struct literal: MyStruct{}
            "(composite_literal type: (type_identifier) @constructed_type) @construction"
        }
        _ => "",
    }
}

// =============================================================================
// MAIN EXTRACTION FUNCTION
// =============================================================================

/// Extract all semantic information from source code using tree-sitter
pub fn extract(content: &str, language: &str) -> Result<ExtractedData, ExtractError> {
    // Parse the content
    let mut parser = get_parser(language)?;
    let tree = parser
        .parse(content, None)
        .ok_or_else(|| ExtractError::ParseFailed {
            language: language.to_string(),
            message: "Parser returned None".to_string(),
        })?;

    let mut data = ExtractedData {
        language: language.to_string(),
        extraction_method: ExtractionMethod::TreeSitter,
        ..Default::default()
    };

    let source = content.as_bytes();

    // Extract functions and methods
    extract_functions(&tree, source, language, &mut data)?;

    // Extract classes/structs (non-fatal if fails)
    if let Err(e) = extract_classes(&tree, source, language, &mut data) {
        tracing::debug!("Class extraction skipped for {}: {}", language, e);
    }

    // Extract function calls
    extract_calls(&tree, source, language, &mut data)?;

    // Extract construction patterns (struct literals, enum variants, etc.)
    if let Err(e) = extract_constructions(&tree, source, language, &mut data) {
        tracing::debug!("Construction extraction skipped for {}: {}", language, e);
    }

    // Build scope tree
    extract_scopes(&tree, &mut data)?;

    // Extract identifiers as tokens
    extract_tokens(&tree, source, &mut data);

    Ok(data)
}

// =============================================================================
// EXTRACTION HELPERS
// =============================================================================

/// Extract function and method definitions
fn extract_functions(
    tree: &Tree,
    source: &[u8],
    language: &str,
    data: &mut ExtractedData,
) -> Result<(), ExtractError> {
    let query_str = function_query(language);
    if query_str.is_empty() {
        return Ok(());
    }

    let lang = get_language(language)?;
    let query = Query::new(&lang, query_str).map_err(|e| ExtractError::ParseFailed {
        language: language.to_string(),
        message: format!("Invalid function query: {}", e),
    })?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source);

    while let Some(m) = matches.next() {
        let mut name: Option<String> = None;
        let mut is_method = false;
        let mut is_exported = false;
        let mut start_line = 0u32;
        let mut end_line = 0u32;
        let mut start_col = 0u16;
        let mut end_col = 0u16;

        for capture in m.captures {
            let node = capture.node;
            let capture_name = query.capture_names()[capture.index as usize];

            match capture_name {
                "name" => {
                    name = node.utf8_text(source).ok().map(|s| s.to_string());
                    start_line = node.start_position().row as u32 + 1;
                    start_col = node.start_position().column as u16;
                }
                "function" => {
                    end_line = node.end_position().row as u32 + 1;
                    end_col = node.end_position().column as u16;
                }
                "method" => {
                    is_method = true;
                    end_line = node.end_position().row as u32 + 1;
                    end_col = node.end_position().column as u16;
                }
                "export_function" => {
                    is_exported = true;
                    end_line = node.end_position().row as u32 + 1;
                    end_col = node.end_position().column as u16;
                }
                _ => {}
            }
        }

        if let Some(name) = name {
            // Skip test/private helpers (simple heuristic)
            if !name.starts_with('_') || language == "python" {
                data.symbols.push(ExtractedSymbol {
                    name,
                    kind: if is_method {
                        SymbolKind::Method
                    } else {
                        SymbolKind::Function
                    },
                    start_line,
                    end_line,
                    start_column: start_col,
                    end_column: end_col,
                    is_exported,
                    is_async: false,
                    parent_symbol: None,
                });
            }
        }
    }

    Ok(())
}

/// Extract class, struct, enum, trait definitions
fn extract_classes(
    tree: &Tree,
    source: &[u8],
    language: &str,
    data: &mut ExtractedData,
) -> Result<(), ExtractError> {
    let query_str = class_query(language);
    if query_str.is_empty() {
        return Ok(());
    }

    let lang = get_language(language)?;
    let query = Query::new(&lang, query_str).map_err(|e| ExtractError::ParseFailed {
        language: language.to_string(),
        message: format!("Invalid class query: {}", e),
    })?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source);

    while let Some(m) = matches.next() {
        let mut name: Option<String> = None;
        let mut kind = SymbolKind::Class;
        let mut start_line = 0u32;
        let mut end_line = 0u32;
        let mut start_col = 0u16;
        let mut end_col = 0u16;

        for capture in m.captures {
            let node = capture.node;
            let capture_name = query.capture_names()[capture.index as usize];

            match capture_name {
                "name" => {
                    name = node.utf8_text(source).ok().map(|s| s.to_string());
                    start_line = node.start_position().row as u32 + 1;
                    start_col = node.start_position().column as u16;
                }
                "class" => {
                    kind = SymbolKind::Class;
                    end_line = node.end_position().row as u32 + 1;
                    end_col = node.end_position().column as u16;
                }
                "struct" => {
                    kind = SymbolKind::Struct;
                    end_line = node.end_position().row as u32 + 1;
                    end_col = node.end_position().column as u16;
                }
                "enum" => {
                    kind = SymbolKind::Enum;
                    end_line = node.end_position().row as u32 + 1;
                    end_col = node.end_position().column as u16;
                }
                "interface" => {
                    kind = SymbolKind::Interface;
                    end_line = node.end_position().row as u32 + 1;
                    end_col = node.end_position().column as u16;
                }
                "trait" => {
                    kind = SymbolKind::Trait;
                    end_line = node.end_position().row as u32 + 1;
                    end_col = node.end_position().column as u16;
                }
                // Note: impl blocks are NOT extracted - see comment in class_query()
                _ => {}
            }
        }

        if let Some(name) = name {
            data.symbols.push(ExtractedSymbol {
                name,
                kind,
                start_line,
                end_line,
                start_column: start_col,
                end_column: end_col,
                is_exported: false,
                is_async: false,
                parent_symbol: None,
            });
        }
    }

    Ok(())
}

/// Extract function calls
fn extract_calls(
    tree: &Tree,
    source: &[u8],
    language: &str,
    data: &mut ExtractedData,
) -> Result<(), ExtractError> {
    let query_str = call_query(language);
    if query_str.is_empty() {
        return Ok(());
    }

    let lang = get_language(language)?;
    let query = Query::new(&lang, query_str).map_err(|e| ExtractError::ParseFailed {
        language: language.to_string(),
        message: format!("Invalid call query: {}", e),
    })?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source);

    while let Some(m) = matches.next() {
        let mut callee: Option<String> = None;
        let mut is_method_call = false;
        let mut line = 0u32;
        let mut column = 0u16;

        for capture in m.captures {
            let node = capture.node;
            let capture_name = query.capture_names()[capture.index as usize];

            match capture_name {
                "callee" => {
                    callee = node.utf8_text(source).ok().map(|s| s.to_string());
                    line = node.start_position().row as u32 + 1;
                    column = node.start_position().column as u16;
                }
                "method_call" | "macro_call" => {
                    is_method_call = capture_name == "method_call";
                }
                _ => {}
            }
        }

        if let Some(callee_name) = callee {
            // Skip common built-ins and very short names that are likely noise
            if !is_common_builtin(&callee_name, language) {
                data.calls.push(ExtractedCall {
                    callee_name,
                    line,
                    column,
                    containing_symbol: None,
                    is_method_call,
                    receiver: None,
                });
            }
        }
    }

    Ok(())
}

/// Extract construction patterns (struct literals, enum variants, new expressions)
fn extract_constructions(
    tree: &Tree,
    source: &[u8],
    language: &str,
    data: &mut ExtractedData,
) -> Result<(), ExtractError> {
    let query_str = construction_query(language);
    if query_str.is_empty() {
        return Ok(());
    }

    let lang = get_language(language)?;
    let query = Query::new(&lang, query_str).map_err(|e| ExtractError::ParseFailed {
        language: language.to_string(),
        message: format!("Invalid construction query: {}", e),
    })?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source);

    while let Some(m) = matches.next() {
        let mut constructed_type: Option<String> = None;
        let mut line = 0u32;
        let mut column = 0u16;

        for capture in m.captures {
            let node = capture.node;
            let capture_name = query.capture_names()[capture.index as usize];

            match capture_name {
                "constructed_type" => {
                    constructed_type = node.utf8_text(source).ok().map(|s| s.to_string());
                    line = node.start_position().row as u32 + 1;
                    column = node.start_position().column as u16;
                }
                _ => {}
            }
        }

        if let Some(type_name) = constructed_type {
            // Skip primitive types and very short names
            if type_name.len() >= 2 && !is_primitive_type(&type_name, language) {
                data.references.push(ExtractedRef {
                    name: type_name,
                    kind: RefKind::Construction,
                    line,
                    column,
                    containing_symbol: None,
                });
            }
        }
    }

    Ok(())
}

/// Check if a type name is a primitive type
fn is_primitive_type(name: &str, language: &str) -> bool {
    match language {
        "rust" => {
            matches!(
                name,
                "i8" | "i16"
                    | "i32"
                    | "i64"
                    | "i128"
                    | "isize"
                    | "u8"
                    | "u16"
                    | "u32"
                    | "u64"
                    | "u128"
                    | "usize"
                    | "f32"
                    | "f64"
                    | "bool"
                    | "char"
                    | "str"
                    | "Self"
            )
        }
        "typescript" | "javascript" => {
            matches!(
                name,
                "string"
                    | "number"
                    | "boolean"
                    | "null"
                    | "undefined"
                    | "any"
                    | "void"
                    | "never"
                    | "object"
                    | "Array"
                    | "Object"
                    | "String"
                    | "Number"
                    | "Boolean"
            )
        }
        "python" => {
            matches!(
                name,
                "int" | "float" | "str" | "bool" | "list" | "dict" | "set" | "tuple" | "None"
            )
        }
        "go" => {
            matches!(
                name,
                "int"
                    | "int8"
                    | "int16"
                    | "int32"
                    | "int64"
                    | "uint"
                    | "uint8"
                    | "uint16"
                    | "uint32"
                    | "uint64"
                    | "float32"
                    | "float64"
                    | "bool"
                    | "string"
                    | "byte"
                    | "rune"
            )
        }
        _ => false,
    }
}

/// Build scope tree from AST
fn extract_scopes(tree: &Tree, data: &mut ExtractedData) -> Result<(), ExtractError> {
    // Add file-level scope
    let root = tree.root_node();
    data.scopes.push(ExtractedScope {
        kind: ScopeKind::File,
        name: None,
        start_line: root.start_position().row as u32 + 1,
        end_line: root.end_position().row as u32 + 1,
        parent_index: None,
    });

    // Walk tree to find scope-creating nodes
    fn walk_scopes(node: tree_sitter::Node, parent_idx: usize, scopes: &mut Vec<ExtractedScope>) {
        let kind_str = node.kind();

        // Determine if this node creates a new scope
        let scope_kind = match kind_str {
            "function_declaration"
            | "function_definition"
            | "function_item"
            | "method_definition"
            | "method_declaration"
            | "arrow_function" => Some(ScopeKind::Function),
            "class_declaration" | "class_definition" | "class_body" | "impl_item" => {
                Some(ScopeKind::Class)
            }
            "block" | "block_statement" | "compound_statement" => Some(ScopeKind::Block),
            "for_statement" | "while_statement" | "loop_expression" | "for_expression" => {
                Some(ScopeKind::Loop)
            }
            "if_statement" | "if_expression" | "match_expression" | "switch_statement" => {
                Some(ScopeKind::Conditional)
            }
            "module" | "module_declaration" => Some(ScopeKind::Module),
            _ => None,
        };

        let current_idx = if let Some(kind) = scope_kind {
            let idx = scopes.len();
            scopes.push(ExtractedScope {
                kind,
                name: None,
                start_line: node.start_position().row as u32 + 1,
                end_line: node.end_position().row as u32 + 1,
                parent_index: Some(parent_idx),
            });
            idx
        } else {
            parent_idx
        };

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            walk_scopes(child, current_idx, scopes);
        }
    }

    let root = tree.root_node();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        walk_scopes(child, 0, &mut data.scopes);
    }

    Ok(())
}

/// Extract all identifiers as tokens
fn extract_tokens(tree: &Tree, source: &[u8], data: &mut ExtractedData) {
    fn walk_tokens(node: tree_sitter::Node, source: &[u8], tokens: &mut Vec<ExtractedToken>) {
        let kind_str = node.kind();

        // Determine token kind based on node type
        let token_kind = match kind_str {
            "identifier" | "property_identifier" | "field_identifier" | "type_identifier" => {
                Some(TokenKind::Identifier)
            }
            "comment" | "line_comment" | "block_comment" => Some(TokenKind::Comment),
            "string" | "string_literal" | "number" | "integer" | "float" | "true" | "false"
            | "null" | "nil" | "none" => Some(TokenKind::Literal),
            _ => None,
        };

        if let Some(kind) = token_kind {
            if let Ok(text) = node.utf8_text(source) {
                // Skip very short identifiers and common noise
                if (kind == TokenKind::Identifier && text.len() >= 2)
                    || kind != TokenKind::Identifier
                {
                    tokens.push(ExtractedToken {
                        name: text.to_string(),
                        kind,
                        line: node.start_position().row as u32 + 1,
                        column: node.start_position().column as u16,
                    });
                }
            }
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            walk_tokens(child, source, tokens);
        }
    }

    walk_tokens(tree.root_node(), source, &mut data.tokens);
}

// =============================================================================
// HELPERS
// =============================================================================

/// Check if a name is a common built-in that should be filtered
fn is_common_builtin(name: &str, language: &str) -> bool {
    match language {
        "javascript" | "typescript" => {
            matches!(
                name,
                "console"
                    | "log"
                    | "error"
                    | "warn"
                    | "info"
                    | "debug"
                    | "parseInt"
                    | "parseFloat"
                    | "isNaN"
                    | "isFinite"
                    | "JSON"
                    | "Math"
                    | "Date"
                    | "Array"
                    | "Object"
                    | "String"
                    | "Number"
                    | "Boolean"
                    | "Promise"
                    | "setTimeout"
                    | "setInterval"
                    | "clearTimeout"
                    | "clearInterval"
                    | "require"
            )
        }
        "python" => {
            matches!(
                name,
                "print"
                    | "len"
                    | "range"
                    | "str"
                    | "int"
                    | "float"
                    | "bool"
                    | "list"
                    | "dict"
                    | "set"
                    | "tuple"
                    | "type"
                    | "isinstance"
                    | "hasattr"
                    | "getattr"
                    | "setattr"
                    | "open"
                    | "input"
                    | "super"
            )
        }
        "rust" => {
            matches!(
                name,
                "println"
                    | "print"
                    | "eprintln"
                    | "eprint"
                    | "format"
                    | "panic"
                    | "assert"
                    | "assert_eq"
                    | "assert_ne"
                    | "vec"
                    | "todo"
                    | "unimplemented"
                    | "unreachable"
            )
        }
        "go" => {
            matches!(
                name,
                "fmt"
                    | "Println"
                    | "Printf"
                    | "Print"
                    | "Sprintf"
                    | "Errorf"
                    | "len"
                    | "cap"
                    | "make"
                    | "new"
                    | "append"
                    | "copy"
                    | "delete"
                    | "close"
                    | "panic"
                    | "recover"
            )
        }
        _ => false,
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_typescript_function() {
        let code = r#"
function greet(name: string): string {
    return `Hello, ${name}!`;
}

export function farewell(name: string): string {
    return `Goodbye, ${name}!`;
}

const add = (a: number, b: number) => a + b;
"#;

        let result = extract(code, "typescript").unwrap();
        assert!(!result.symbols.is_empty(), "Should extract symbols");

        let func_names: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function)
            .map(|s| s.name.as_str())
            .collect();

        assert!(func_names.contains(&"greet"), "Should find greet function");
        assert!(
            func_names.contains(&"farewell"),
            "Should find farewell function"
        );
    }

    #[test]
    fn test_extract_python_class() {
        let code = r#"
class MyClass:
    def __init__(self, value):
        self.value = value

    def get_value(self):
        return self.value

def standalone_function():
    pass
"#;

        let result = extract(code, "python").unwrap();
        assert!(!result.symbols.is_empty(), "Should extract symbols");

        let class_names: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Class)
            .map(|s| s.name.as_str())
            .collect();

        assert!(class_names.contains(&"MyClass"), "Should find MyClass");
    }

    #[test]
    fn test_extract_rust_struct() {
        let code = r#"
pub struct MyStruct {
    pub field: String,
}

impl MyStruct {
    pub fn new(field: String) -> Self {
        Self { field }
    }

    pub fn get_field(&self) -> &str {
        &self.field
    }
}

fn helper_function() -> i32 {
    42
}
"#;

        let result = extract(code, "rust").unwrap();
        assert!(!result.symbols.is_empty(), "Should extract symbols");

        let struct_names: Vec<_> = result
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Struct)
            .map(|s| s.name.as_str())
            .collect();

        assert!(struct_names.contains(&"MyStruct"), "Should find MyStruct");
    }

    #[test]
    fn test_extract_calls() {
        let code = r#"
function main() {
    greet("World");
    obj.method();
    nested.deep.call();
}
"#;

        let result = extract(code, "javascript").unwrap();
        assert!(!result.calls.is_empty(), "Should extract calls");

        let call_names: Vec<_> = result
            .calls
            .iter()
            .map(|c| c.callee_name.as_str())
            .collect();
        assert!(call_names.contains(&"greet"), "Should find greet call");
        assert!(call_names.contains(&"method"), "Should find method call");
    }

    #[test]
    fn test_unsupported_language() {
        let result = extract("code", "ruby");
        assert!(
            result.is_err(),
            "Should return error for unsupported language"
        );
    }
}
