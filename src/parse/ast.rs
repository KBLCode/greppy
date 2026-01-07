//! AST-aware code parsing using tree-sitter
//!
//! This module provides semantic code chunking by parsing source files
//! into their AST representation and extracting meaningful code units
//! like functions, classes, methods, and structs.

use crate::error::Result;
use crate::parse::languages::Language;
use tree_sitter::{Node, Parser, Tree};

/// Represents a semantic code symbol extracted from the AST
#[derive(Debug, Clone)]
pub struct Symbol {
    /// The symbol name (e.g., function name, class name)
    pub name: String,
    /// The kind of symbol (function, class, method, struct, etc.)
    pub kind: SymbolKind,
    /// The full signature (e.g., "fn authenticate(user: &User) -> Result<Token>")
    pub signature: Option<String>,
    /// Parent symbol name (for methods inside classes)
    pub parent: Option<String>,
    /// Documentation comment if present
    pub doc_comment: Option<String>,
    /// Start byte offset in source
    pub start_byte: usize,
    /// End byte offset in source
    pub end_byte: usize,
    /// Start line (0-indexed)
    pub start_line: usize,
    /// End line (0-indexed)
    pub end_line: usize,
    /// Whether this symbol is exported/public
    pub is_exported: bool,
    /// Whether this is a test function/method
    pub is_test: bool,
}

/// The kind of symbol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Interface,
    Trait,
    Impl,
    Module,
    Constant,
    Variable,
    Type,
    Unknown,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SymbolKind::Function => "function",
            SymbolKind::Method => "method",
            SymbolKind::Class => "class",
            SymbolKind::Struct => "struct",
            SymbolKind::Enum => "enum",
            SymbolKind::Interface => "interface",
            SymbolKind::Trait => "trait",
            SymbolKind::Impl => "impl",
            SymbolKind::Module => "module",
            SymbolKind::Constant => "constant",
            SymbolKind::Variable => "variable",
            SymbolKind::Type => "type",
            SymbolKind::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// AST parser for extracting symbols from source code
pub struct AstParser {
    parser: Parser,
    language: Language,
}

impl AstParser {
    /// Create a new AST parser for the given language
    pub fn new(language: Language) -> Option<Self> {
        let ts_lang = language.tree_sitter_language()?;
        let mut parser = Parser::new();
        parser.set_language(&ts_lang).ok()?;
        Some(Self { parser, language })
    }

    /// Parse source code and extract symbols
    pub fn parse(&mut self, source: &str) -> Result<Vec<Symbol>> {
        let tree = self.parser.parse(source, None)
            .ok_or_else(|| crate::error::GreppyError::Parse("Failed to parse source".into()))?;

        let mut symbols = Vec::new();
        self.extract_symbols(&tree, source, &mut symbols);
        Ok(symbols)
    }

    /// Extract symbols from the AST
    fn extract_symbols(&self, tree: &Tree, source: &str, symbols: &mut Vec<Symbol>) {
        let root = tree.root_node();
        self.visit_node(root, source, None, symbols);
    }

    /// Recursively visit AST nodes and extract symbols
    fn visit_node(
        &self,
        node: Node,
        source: &str,
        parent: Option<&str>,
        symbols: &mut Vec<Symbol>,
    ) {
        let kind = node.kind();

        // Try to extract a symbol from this node
        if let Some(symbol) = self.extract_symbol_from_node(node, source, parent) {
            let symbol_name = symbol.name.clone();
            symbols.push(symbol);

            // For container types (class, struct, impl), visit children with this as parent
            if self.is_container_node(kind) {
                for child in node.children(&mut node.walk()) {
                    self.visit_node(child, source, Some(&symbol_name), symbols);
                }
                return;
            }
        }

        // Visit children
        for child in node.children(&mut node.walk()) {
            self.visit_node(child, source, parent, symbols);
        }
    }

    /// Check if a node is a container (class, struct, impl, etc.)
    fn is_container_node(&self, kind: &str) -> bool {
        matches!(
            kind,
            "class_declaration"
                | "class_definition"
                | "struct_item"
                | "impl_item"
                | "trait_item"
                | "interface_declaration"
                | "enum_item"
                | "enum_declaration"
                | "module_declaration"
        )
    }

    /// Extract a symbol from an AST node
    fn extract_symbol_from_node(
        &self,
        node: Node,
        source: &str,
        parent: Option<&str>,
    ) -> Option<Symbol> {
        match self.language {
            Language::Rust => self.extract_rust_symbol(node, source, parent),
            Language::TypeScript | Language::TypeScriptReact => {
                self.extract_typescript_symbol(node, source, parent)
            }
            Language::JavaScript | Language::JavaScriptReact => {
                self.extract_javascript_symbol(node, source, parent)
            }
            Language::Python => self.extract_python_symbol(node, source, parent),
            Language::Go => self.extract_go_symbol(node, source, parent),
            Language::Java => self.extract_java_symbol(node, source, parent),
            Language::C | Language::Cpp => self.extract_c_symbol(node, source, parent),
            Language::Unknown => None,
        }
    }

    /// Extract Rust symbols
    fn extract_rust_symbol(
        &self,
        node: Node,
        source: &str,
        parent: Option<&str>,
    ) -> Option<Symbol> {
        let kind = node.kind();
        let (symbol_kind, name_field) = match kind {
            "function_item" => (SymbolKind::Function, "name"),
            "struct_item" => (SymbolKind::Struct, "name"),
            "enum_item" => (SymbolKind::Enum, "name"),
            "trait_item" => (SymbolKind::Trait, "name"),
            "impl_item" => (SymbolKind::Impl, "type"),
            "mod_item" => (SymbolKind::Module, "name"),
            "const_item" => (SymbolKind::Constant, "name"),
            "static_item" => (SymbolKind::Constant, "name"),
            "type_item" => (SymbolKind::Type, "name"),
            _ => return None,
        };

        let name = self.get_child_text(node, name_field, source)?;
        let signature = self.get_signature(node, source);
        let doc_comment = self.get_preceding_comment(node, source);
        let is_exported = self.is_rust_public(node, source);
        let is_test = self.has_test_attribute(node, source);

        Some(Symbol {
            name,
            kind: if parent.is_some() && symbol_kind == SymbolKind::Function {
                SymbolKind::Method
            } else {
                symbol_kind
            },
            signature,
            parent: parent.map(String::from),
            doc_comment,
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_line: node.start_position().row,
            end_line: node.end_position().row,
            is_exported,
            is_test,
        })
    }

    /// Extract TypeScript symbols
    fn extract_typescript_symbol(
        &self,
        node: Node,
        source: &str,
        parent: Option<&str>,
    ) -> Option<Symbol> {
        let kind = node.kind();
        let (symbol_kind, name_field) = match kind {
            "function_declaration" => (SymbolKind::Function, "name"),
            "method_definition" => (SymbolKind::Method, "name"),
            "class_declaration" => (SymbolKind::Class, "name"),
            "interface_declaration" => (SymbolKind::Interface, "name"),
            "type_alias_declaration" => (SymbolKind::Type, "name"),
            "enum_declaration" => (SymbolKind::Enum, "name"),
            "lexical_declaration" | "variable_declaration" => {
                return self.extract_ts_variable(node, source, parent);
            }
            "arrow_function" => {
                // Arrow functions need special handling - get name from parent
                return self.extract_arrow_function(node, source, parent);
            }
            _ => return None,
        };

        let name = self.get_child_text(node, name_field, source)?;
        let signature = self.get_signature(node, source);
        let doc_comment = self.get_preceding_comment(node, source);
        let is_exported = self.is_ts_exported(node, source);
        let is_test = self.is_test_function(&name, source);

        Some(Symbol {
            name,
            kind: symbol_kind,
            signature,
            parent: parent.map(String::from),
            doc_comment,
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_line: node.start_position().row,
            end_line: node.end_position().row,
            is_exported,
            is_test,
        })
    }

    /// Extract JavaScript symbols (similar to TypeScript)
    fn extract_javascript_symbol(
        &self,
        node: Node,
        source: &str,
        parent: Option<&str>,
    ) -> Option<Symbol> {
        // JavaScript uses similar node types to TypeScript
        self.extract_typescript_symbol(node, source, parent)
    }

    /// Extract Python symbols
    fn extract_python_symbol(
        &self,
        node: Node,
        source: &str,
        parent: Option<&str>,
    ) -> Option<Symbol> {
        let kind = node.kind();
        let (symbol_kind, name_field) = match kind {
            "function_definition" => (SymbolKind::Function, "name"),
            "class_definition" => (SymbolKind::Class, "name"),
            "decorated_definition" => {
                // Handle decorated functions/classes
                return self.extract_decorated_python(node, source, parent);
            }
            _ => return None,
        };

        let name = self.get_child_text(node, name_field, source)?;
        let signature = self.get_signature(node, source);
        let doc_comment = self.get_python_docstring(node, source);
        let is_exported = !name.starts_with('_');
        let is_test = name.starts_with("test_") || name.starts_with("test");

        Some(Symbol {
            name,
            kind: if parent.is_some() && symbol_kind == SymbolKind::Function {
                SymbolKind::Method
            } else {
                symbol_kind
            },
            signature,
            parent: parent.map(String::from),
            doc_comment,
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_line: node.start_position().row,
            end_line: node.end_position().row,
            is_exported,
            is_test,
        })
    }

    /// Extract Go symbols
    fn extract_go_symbol(
        &self,
        node: Node,
        source: &str,
        parent: Option<&str>,
    ) -> Option<Symbol> {
        let kind = node.kind();
        let (symbol_kind, name_field) = match kind {
            "function_declaration" => (SymbolKind::Function, "name"),
            "method_declaration" => (SymbolKind::Method, "name"),
            "type_declaration" => (SymbolKind::Type, "name"),
            "type_spec" => {
                // Check if it's a struct or interface
                return self.extract_go_type_spec(node, source, parent);
            }
            _ => return None,
        };

        let name = self.get_child_text(node, name_field, source)?;
        let signature = self.get_signature(node, source);
        let doc_comment = self.get_preceding_comment(node, source);
        let is_exported = name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
        let is_test = name.starts_with("Test") || name.starts_with("Benchmark");

        Some(Symbol {
            name,
            kind: symbol_kind,
            signature,
            parent: parent.map(String::from),
            doc_comment,
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_line: node.start_position().row,
            end_line: node.end_position().row,
            is_exported,
            is_test,
        })
    }

    /// Extract Java symbols
    fn extract_java_symbol(
        &self,
        node: Node,
        source: &str,
        parent: Option<&str>,
    ) -> Option<Symbol> {
        let kind = node.kind();
        let (symbol_kind, name_field) = match kind {
            "method_declaration" => (SymbolKind::Method, "name"),
            "class_declaration" => (SymbolKind::Class, "name"),
            "interface_declaration" => (SymbolKind::Interface, "name"),
            "enum_declaration" => (SymbolKind::Enum, "name"),
            "constructor_declaration" => (SymbolKind::Method, "name"),
            _ => return None,
        };

        let name = self.get_child_text(node, name_field, source)?;
        let signature = self.get_signature(node, source);
        let doc_comment = self.get_preceding_comment(node, source);
        let is_exported = self.is_java_public(node, source);
        let is_test = self.has_java_test_annotation(node, source);

        Some(Symbol {
            name,
            kind: symbol_kind,
            signature,
            parent: parent.map(String::from),
            doc_comment,
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_line: node.start_position().row,
            end_line: node.end_position().row,
            is_exported,
            is_test,
        })
    }

    /// Extract C/C++ symbols
    fn extract_c_symbol(
        &self,
        node: Node,
        source: &str,
        parent: Option<&str>,
    ) -> Option<Symbol> {
        let kind = node.kind();
        let (symbol_kind, name_field) = match kind {
            "function_definition" => (SymbolKind::Function, "declarator"),
            "struct_specifier" => (SymbolKind::Struct, "name"),
            "enum_specifier" => (SymbolKind::Enum, "name"),
            "class_specifier" => (SymbolKind::Class, "name"),
            _ => return None,
        };

        let name = self.get_child_text(node, name_field, source)
            .or_else(|| self.get_function_name_from_declarator(node, source))?;
        let signature = self.get_signature(node, source);
        let doc_comment = self.get_preceding_comment(node, source);

        Some(Symbol {
            name,
            kind: symbol_kind,
            signature,
            parent: parent.map(String::from),
            doc_comment,
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_line: node.start_position().row,
            end_line: node.end_position().row,
            is_exported: true, // C doesn't have export concept in the same way
            is_test: false,
        })
    }

    // Helper methods

    /// Get text of a named child node
    fn get_child_text(&self, node: Node, field_name: &str, source: &str) -> Option<String> {
        node.child_by_field_name(field_name)
            .map(|n| source[n.start_byte()..n.end_byte()].to_string())
    }

    /// Get the signature (first line) of a node
    fn get_signature(&self, node: Node, source: &str) -> Option<String> {
        let text = &source[node.start_byte()..node.end_byte()];
        let first_line = text.lines().next()?;
        Some(first_line.trim().to_string())
    }

    /// Get preceding comment (doc comment)
    fn get_preceding_comment(&self, node: Node, source: &str) -> Option<String> {
        let mut prev = node.prev_sibling();
        let mut comments = Vec::new();

        while let Some(sibling) = prev {
            let kind = sibling.kind();
            if kind == "comment" || kind == "line_comment" || kind == "block_comment" {
                let text = &source[sibling.start_byte()..sibling.end_byte()];
                comments.push(text.to_string());
                prev = sibling.prev_sibling();
            } else if kind.contains("attribute") || kind == "decorator" {
                // Skip attributes/decorators
                prev = sibling.prev_sibling();
            } else {
                break;
            }
        }

        if comments.is_empty() {
            None
        } else {
            comments.reverse();
            Some(comments.join("\n"))
        }
    }

    /// Check if a Rust item is public
    fn is_rust_public(&self, node: Node, source: &str) -> bool {
        for child in node.children(&mut node.walk()) {
            if child.kind() == "visibility_modifier" {
                let text = &source[child.start_byte()..child.end_byte()];
                return text.starts_with("pub");
            }
        }
        false
    }

    /// Check if a Rust function has #[test] attribute
    fn has_test_attribute(&self, node: Node, source: &str) -> bool {
        let mut prev = node.prev_sibling();
        while let Some(sibling) = prev {
            if sibling.kind() == "attribute_item" {
                let text = &source[sibling.start_byte()..sibling.end_byte()];
                if text.contains("test") || text.contains("tokio::test") {
                    return true;
                }
            } else if sibling.kind() != "line_comment" && sibling.kind() != "block_comment" {
                break;
            }
            prev = sibling.prev_sibling();
        }
        false
    }

    /// Check if a TypeScript/JavaScript item is exported
    fn is_ts_exported(&self, node: Node, source: &str) -> bool {
        if let Some(parent) = node.parent() {
            if parent.kind() == "export_statement" {
                return true;
            }
        }
        // Check for export keyword in the node itself
        let text = &source[node.start_byte()..node.end_byte()];
        text.starts_with("export ")
    }

    /// Check if a function name suggests it's a test
    fn is_test_function(&self, name: &str, _source: &str) -> bool {
        name.starts_with("test")
            || name.ends_with("Test")
            || name.ends_with("_test")
            || name.contains("spec")
    }

    /// Extract TypeScript variable declarations (const, let, var)
    fn extract_ts_variable(
        &self,
        node: Node,
        source: &str,
        parent: Option<&str>,
    ) -> Option<Symbol> {
        // Look for variable_declarator children
        for child in node.children(&mut node.walk()) {
            if child.kind() == "variable_declarator" {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = source[name_node.start_byte()..name_node.end_byte()].to_string();
                    
                    // Check if it's a function (arrow function or function expression)
                    if let Some(value) = child.child_by_field_name("value") {
                        let value_kind = value.kind();
                        if value_kind == "arrow_function" || value_kind == "function" {
                            let is_test = self.is_test_function(&name, source);
                            return Some(Symbol {
                                name,
                                kind: SymbolKind::Function,
                                signature: self.get_signature(node, source),
                                parent: parent.map(String::from),
                                doc_comment: self.get_preceding_comment(node, source),
                                start_byte: node.start_byte(),
                                end_byte: node.end_byte(),
                                start_line: node.start_position().row,
                                end_line: node.end_position().row,
                                is_exported: self.is_ts_exported(node, source),
                                is_test,
                            });
                        }
                    }

                    // Regular variable/constant
                    return Some(Symbol {
                        name,
                        kind: SymbolKind::Variable,
                        signature: self.get_signature(node, source),
                        parent: parent.map(String::from),
                        doc_comment: self.get_preceding_comment(node, source),
                        start_byte: node.start_byte(),
                        end_byte: node.end_byte(),
                        start_line: node.start_position().row,
                        end_line: node.end_position().row,
                        is_exported: self.is_ts_exported(node, source),
                        is_test: false,
                    });
                }
            }
        }
        None
    }

    /// Extract arrow function (needs to get name from parent variable declaration)
    fn extract_arrow_function(
        &self,
        node: Node,
        source: &str,
        parent: Option<&str>,
    ) -> Option<Symbol> {
        // Arrow functions are usually assigned to variables
        // The name comes from the variable_declarator parent
        if let Some(declarator) = node.parent() {
            if declarator.kind() == "variable_declarator" {
                if let Some(name_node) = declarator.child_by_field_name("name") {
                    let name = source[name_node.start_byte()..name_node.end_byte()].to_string();
                    
                    // Get the full declaration for signature
                    let decl_node = declarator.parent().unwrap_or(declarator);
                    
                    return Some(Symbol {
                        name: name.clone(),
                        kind: SymbolKind::Function,
                        signature: self.get_signature(decl_node, source),
                        parent: parent.map(String::from),
                        doc_comment: self.get_preceding_comment(decl_node, source),
                        start_byte: decl_node.start_byte(),
                        end_byte: decl_node.end_byte(),
                        start_line: decl_node.start_position().row,
                        end_line: decl_node.end_position().row,
                        is_exported: self.is_ts_exported(decl_node, source),
                        is_test: self.is_test_function(&name, source),
                    });
                }
            }
        }
        None
    }

    /// Extract decorated Python definitions
    fn extract_decorated_python(
        &self,
        node: Node,
        source: &str,
        parent: Option<&str>,
    ) -> Option<Symbol> {
        // Find the actual definition inside the decorated_definition
        for child in node.children(&mut node.walk()) {
            if child.kind() == "function_definition" || child.kind() == "class_definition" {
                let mut symbol = self.extract_python_symbol(child, source, parent)?;
                // Update byte ranges to include decorators
                symbol.start_byte = node.start_byte();
                symbol.start_line = node.start_position().row;
                // Check for test decorators
                symbol.is_test = symbol.is_test || self.has_python_test_decorator(node, source);
                return Some(symbol);
            }
        }
        None
    }

    /// Check for Python test decorators
    fn has_python_test_decorator(&self, node: Node, source: &str) -> bool {
        for child in node.children(&mut node.walk()) {
            if child.kind() == "decorator" {
                let text = &source[child.start_byte()..child.end_byte()];
                if text.contains("pytest") || text.contains("test") {
                    return true;
                }
            }
        }
        false
    }

    /// Get Python docstring
    fn get_python_docstring(&self, node: Node, source: &str) -> Option<String> {
        // Look for expression_statement with string as first child in body
        for child in node.children(&mut node.walk()) {
            if child.kind() == "block" {
                for block_child in child.children(&mut child.walk()) {
                    if block_child.kind() == "expression_statement" {
                        if let Some(string_node) = block_child.child(0) {
                            if string_node.kind() == "string" {
                                let text = &source[string_node.start_byte()..string_node.end_byte()];
                                return Some(text.to_string());
                            }
                        }
                    }
                    break; // Only check first statement
                }
            }
        }
        None
    }

    /// Extract Go type spec (struct or interface)
    fn extract_go_type_spec(
        &self,
        node: Node,
        source: &str,
        parent: Option<&str>,
    ) -> Option<Symbol> {
        let name = self.get_child_text(node, "name", source)?;
        
        // Determine if it's a struct or interface
        let mut kind = SymbolKind::Type;
        for child in node.children(&mut node.walk()) {
            match child.kind() {
                "struct_type" => kind = SymbolKind::Struct,
                "interface_type" => kind = SymbolKind::Interface,
                _ => {}
            }
        }

        let is_exported = name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);

        Some(Symbol {
            name,
            kind,
            signature: self.get_signature(node, source),
            parent: parent.map(String::from),
            doc_comment: self.get_preceding_comment(node, source),
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_line: node.start_position().row,
            end_line: node.end_position().row,
            is_exported,
            is_test: false,
        })
    }

    /// Check if Java method/class is public
    fn is_java_public(&self, node: Node, source: &str) -> bool {
        for child in node.children(&mut node.walk()) {
            if child.kind() == "modifiers" {
                let text = &source[child.start_byte()..child.end_byte()];
                return text.contains("public");
            }
        }
        false
    }

    /// Check for Java @Test annotation
    fn has_java_test_annotation(&self, node: Node, source: &str) -> bool {
        for child in node.children(&mut node.walk()) {
            if child.kind() == "modifiers" {
                for modifier_child in child.children(&mut child.walk()) {
                    if modifier_child.kind() == "marker_annotation" || modifier_child.kind() == "annotation" {
                        let text = &source[modifier_child.start_byte()..modifier_child.end_byte()];
                        if text.contains("Test") {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Get function name from C/C++ declarator
    fn get_function_name_from_declarator(&self, node: Node, source: &str) -> Option<String> {
        fn find_identifier(node: Node, source: &str) -> Option<String> {
            if node.kind() == "identifier" {
                return Some(source[node.start_byte()..node.end_byte()].to_string());
            }
            for child in node.children(&mut node.walk()) {
                if let Some(name) = find_identifier(child, source) {
                    return Some(name);
                }
            }
            None
        }

        if let Some(declarator) = node.child_by_field_name("declarator") {
            return find_identifier(declarator, source);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rust_parsing() {
        let source = r#"
/// This is a doc comment
pub fn authenticate(user: &User) -> Result<Token> {
    validate_token(user.token)
}

struct User {
    name: String,
    token: String,
}
"#;
        let mut parser = AstParser::new(Language::Rust).unwrap();
        let symbols = parser.parse(source).unwrap();
        
        assert!(symbols.iter().any(|s| s.name == "authenticate" && s.kind == SymbolKind::Function));
        assert!(symbols.iter().any(|s| s.name == "User" && s.kind == SymbolKind::Struct));
    }

    #[test]
    fn test_typescript_parsing() {
        let source = r#"
export function authenticate(user: User): Token {
    return validateToken(user.token);
}

export class AuthService {
    validate(token: string): boolean {
        return true;
    }
}
"#;
        let mut parser = AstParser::new(Language::TypeScript).unwrap();
        let symbols = parser.parse(source).unwrap();
        
        assert!(symbols.iter().any(|s| s.name == "authenticate" && s.kind == SymbolKind::Function));
        assert!(symbols.iter().any(|s| s.name == "AuthService" && s.kind == SymbolKind::Class));
    }
}
