//! Regex-Based Extraction Fallback
//!
//! Best-effort extraction using regular expressions for languages
//! not supported by tree-sitter. Provides lower confidence results
//! but works for any text-based source file.
//!
//! Extracts:
//! - Function/method definitions
//! - Class/struct definitions
//! - Function calls (basic patterns)
//! - Import statements
//!
//! @module trace/extract/regex

use super::{
    ExtractedCall, ExtractedData, ExtractedRef, ExtractedScope, ExtractedSymbol, ExtractedToken,
    ExtractionMethod, RefKind, ScopeKind, SymbolKind, TokenKind,
};
use once_cell::sync::Lazy;
use regex::Regex;

// =============================================================================
// COMPILED REGEX PATTERNS
// =============================================================================

/// Function definition patterns for various languages
static FUNCTION_PATTERNS: Lazy<Vec<FunctionPattern>> = Lazy::new(|| {
    vec![
        // JavaScript/TypeScript: function name(
        FunctionPattern {
            regex: Regex::new(r"(?m)^[\t ]*(?:export\s+)?(?:async\s+)?function\s+([a-zA-Z_$][a-zA-Z0-9_$]*)\s*\(")
                .unwrap(),
            kind: SymbolKind::Function,
            is_exported_group: None,
            is_async: false,
        },
        // JavaScript/TypeScript: const/let/var name = (arrow function)
        FunctionPattern {
            regex: Regex::new(r"(?m)^[\t ]*(?:export\s+)?(?:const|let|var)\s+([a-zA-Z_$][a-zA-Z0-9_$]*)\s*=\s*(?:async\s+)?\([^)]*\)\s*=>")
                .unwrap(),
            kind: SymbolKind::Function,
            is_exported_group: None,
            is_async: false,
        },
        // JavaScript/TypeScript: const/let/var name = async (arrow function)
        FunctionPattern {
            regex: Regex::new(r"(?m)^[\t ]*(?:export\s+)?(?:const|let|var)\s+([a-zA-Z_$][a-zA-Z0-9_$]*)\s*=\s*async\s+\([^)]*\)\s*=>")
                .unwrap(),
            kind: SymbolKind::Function,
            is_exported_group: None,
            is_async: true,
        },
        // Python: def name(
        FunctionPattern {
            regex: Regex::new(r"(?m)^[\t ]*(?:async\s+)?def\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(")
                .unwrap(),
            kind: SymbolKind::Function,
            is_exported_group: None,
            is_async: false,
        },
        // Rust: fn name( or pub fn name(
        FunctionPattern {
            regex: Regex::new(r"(?m)^[\t ]*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*[<(]")
                .unwrap(),
            kind: SymbolKind::Function,
            is_exported_group: None,
            is_async: false,
        },
        // Go: func name( or func (r Receiver) name(
        FunctionPattern {
            regex: Regex::new(r"(?m)^[\t ]*func\s+(?:\([^)]+\)\s+)?([a-zA-Z_][a-zA-Z0-9_]*)\s*\(")
                .unwrap(),
            kind: SymbolKind::Function,
            is_exported_group: None,
            is_async: false,
        },
        // Ruby: def name
        FunctionPattern {
            regex: Regex::new(r"(?m)^[\t ]*def\s+([a-zA-Z_][a-zA-Z0-9_!?]*)")
                .unwrap(),
            kind: SymbolKind::Method,
            is_exported_group: None,
            is_async: false,
        },
        // PHP: function name(
        FunctionPattern {
            regex: Regex::new(r"(?m)^[\t ]*(?:public|private|protected|static|\s)*function\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(")
                .unwrap(),
            kind: SymbolKind::Function,
            is_exported_group: None,
            is_async: false,
        },
        // Java/C#: visibility returnType name(
        FunctionPattern {
            regex: Regex::new(r"(?m)^[\t ]*(?:public|private|protected|static|\s)*(?:async\s+)?(?:[a-zA-Z_<>\[\]]+\s+)+([a-zA-Z_][a-zA-Z0-9_]*)\s*\([^)]*\)\s*(?:throws\s+[^\{]+)?\{")
                .unwrap(),
            kind: SymbolKind::Method,
            is_exported_group: None,
            is_async: false,
        },
        // C/C++: returnType name(
        FunctionPattern {
            regex: Regex::new(r"(?m)^[\t ]*(?:static\s+)?(?:inline\s+)?(?:[a-zA-Z_][a-zA-Z0-9_*& ]+\s+)+([a-zA-Z_][a-zA-Z0-9_]*)\s*\([^)]*\)\s*\{")
                .unwrap(),
            kind: SymbolKind::Function,
            is_exported_group: None,
            is_async: false,
        },
        // Lua: function name(
        FunctionPattern {
            regex: Regex::new(r"(?m)^[\t ]*(?:local\s+)?function\s+([a-zA-Z_][a-zA-Z0-9_.:]*)\s*\(")
                .unwrap(),
            kind: SymbolKind::Function,
            is_exported_group: None,
            is_async: false,
        },
        // Elixir: def name(
        FunctionPattern {
            regex: Regex::new(r"(?m)^[\t ]*def[p]?\s+([a-zA-Z_][a-zA-Z0-9_!?]*)")
                .unwrap(),
            kind: SymbolKind::Function,
            is_exported_group: None,
            is_async: false,
        },
        // Haskell: name :: Type
        FunctionPattern {
            regex: Regex::new(r"(?m)^([a-z_][a-zA-Z0-9_']*)\s*::")
                .unwrap(),
            kind: SymbolKind::Function,
            is_exported_group: None,
            is_async: false,
        },
    ]
});

/// Class/struct definition patterns
static CLASS_PATTERNS: Lazy<Vec<ClassPattern>> = Lazy::new(|| {
    vec![
        // JavaScript/TypeScript: class Name
        ClassPattern {
            regex: Regex::new(r"(?m)^[\t ]*(?:export\s+)?(?:abstract\s+)?class\s+([a-zA-Z_$][a-zA-Z0-9_$]*)").unwrap(),
            kind: SymbolKind::Class,
        },
        // TypeScript: interface Name
        ClassPattern {
            regex: Regex::new(r"(?m)^[\t ]*(?:export\s+)?interface\s+([a-zA-Z_$][a-zA-Z0-9_$]*)").unwrap(),
            kind: SymbolKind::Interface,
        },
        // TypeScript: type Name =
        ClassPattern {
            regex: Regex::new(r"(?m)^[\t ]*(?:export\s+)?type\s+([a-zA-Z_$][a-zA-Z0-9_$]*)\s*(?:<[^>]*>)?\s*=").unwrap(),
            kind: SymbolKind::TypeAlias,
        },
        // Python: class Name:
        ClassPattern {
            regex: Regex::new(r"(?m)^[\t ]*class\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap(),
            kind: SymbolKind::Class,
        },
        // Rust: struct Name or pub struct Name
        ClassPattern {
            regex: Regex::new(r"(?m)^[\t ]*(?:pub(?:\([^)]*\))?\s+)?struct\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap(),
            kind: SymbolKind::Struct,
        },
        // Rust: enum Name or pub enum Name
        ClassPattern {
            regex: Regex::new(r"(?m)^[\t ]*(?:pub(?:\([^)]*\))?\s+)?enum\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap(),
            kind: SymbolKind::Enum,
        },
        // Rust: trait Name
        ClassPattern {
            regex: Regex::new(r"(?m)^[\t ]*(?:pub(?:\([^)]*\))?\s+)?trait\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap(),
            kind: SymbolKind::Trait,
        },
        // Rust: impl Name
        ClassPattern {
            regex: Regex::new(r"(?m)^[\t ]*impl(?:<[^>]*>)?\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap(),
            kind: SymbolKind::Impl,
        },
        // Go: type Name struct
        ClassPattern {
            regex: Regex::new(r"(?m)^[\t ]*type\s+([a-zA-Z_][a-zA-Z0-9_]*)\s+struct").unwrap(),
            kind: SymbolKind::Struct,
        },
        // Go: type Name interface
        ClassPattern {
            regex: Regex::new(r"(?m)^[\t ]*type\s+([a-zA-Z_][a-zA-Z0-9_]*)\s+interface").unwrap(),
            kind: SymbolKind::Interface,
        },
        // Ruby: class Name or module Name
        ClassPattern {
            regex: Regex::new(r"(?m)^[\t ]*(?:class|module)\s+([a-zA-Z_][a-zA-Z0-9_:]*)").unwrap(),
            kind: SymbolKind::Class,
        },
        // PHP: class Name
        ClassPattern {
            regex: Regex::new(r"(?m)^[\t ]*(?:abstract\s+)?(?:final\s+)?class\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap(),
            kind: SymbolKind::Class,
        },
        // Java/C#: class Name
        ClassPattern {
            regex: Regex::new(r"(?m)^[\t ]*(?:public\s+)?(?:abstract\s+)?(?:final\s+)?class\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap(),
            kind: SymbolKind::Class,
        },
        // C++: class Name or struct Name
        ClassPattern {
            regex: Regex::new(r"(?m)^[\t ]*(?:class|struct)\s+([a-zA-Z_][a-zA-Z0-9_]*)").unwrap(),
            kind: SymbolKind::Class,
        },
    ]
});

/// Call patterns (function/method invocation)
static CALL_PATTERN: Lazy<Regex> = Lazy::new(|| {
    // Match: identifier( or identifier.method( or identifier->method(
    Regex::new(r"([a-zA-Z_][a-zA-Z0-9_]*)\s*\(").unwrap()
});

/// Method call pattern
static METHOD_CALL_PATTERN: Lazy<Regex> = Lazy::new(|| {
    // Match: receiver.method( or receiver->method(
    Regex::new(r"([a-zA-Z_][a-zA-Z0-9_]*)\s*(?:\.|->)\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*\(").unwrap()
});

/// Import patterns
static IMPORT_PATTERNS: Lazy<Vec<ImportPattern>> = Lazy::new(|| {
    vec![
        // JavaScript/TypeScript: import ... from 'module'
        ImportPattern {
            regex: Regex::new(r#"(?m)^[\t ]*import\s+(?:\{[^}]*\}|[^'"]+)\s+from\s+['"]([@a-zA-Z0-9_./-]+)['"]"#).unwrap(),
            language: "javascript",
        },
        // JavaScript: require('module')
        ImportPattern {
            regex: Regex::new(r#"require\s*\(\s*['"]([@a-zA-Z0-9_./-]+)['"]\s*\)"#).unwrap(),
            language: "javascript",
        },
        // Python: import module or from module import
        ImportPattern {
            regex: Regex::new(r"(?m)^[\t ]*(?:from\s+([a-zA-Z_][a-zA-Z0-9_.]*)\s+)?import\s+([a-zA-Z_][a-zA-Z0-9_.,\s]*)").unwrap(),
            language: "python",
        },
        // Rust: use crate::module
        ImportPattern {
            regex: Regex::new(r"(?m)^[\t ]*use\s+([a-zA-Z_][a-zA-Z0-9_:]*(?:::\{[^}]+\})?)").unwrap(),
            language: "rust",
        },
        // Go: import "package"
        ImportPattern {
            regex: Regex::new(r#"(?m)^[\t ]*import\s+["]([a-zA-Z0-9_./-]+)["]"#).unwrap(),
            language: "go",
        },
        // Ruby: require 'file' or require_relative 'file'
        ImportPattern {
            regex: Regex::new(r#"(?m)^[\t ]*require(?:_relative)?\s+['"]([@a-zA-Z0-9_./-]+)['"]"#).unwrap(),
            language: "ruby",
        },
        // PHP: use Namespace\Class
        ImportPattern {
            regex: Regex::new(r"(?m)^[\t ]*use\s+([a-zA-Z_\\][a-zA-Z0-9_\\]*)").unwrap(),
            language: "php",
        },
        // Java: import package.Class
        ImportPattern {
            regex: Regex::new(r"(?m)^[\t ]*import\s+(?:static\s+)?([a-zA-Z_][a-zA-Z0-9_.]*(?:\.\*)?);").unwrap(),
            language: "java",
        },
        // C/C++: #include <header> or #include "header"
        ImportPattern {
            regex: Regex::new(r#"(?m)^[\t ]*#include\s*[<"]([^>"]+)[>"]"#).unwrap(),
            language: "c",
        },
    ]
});

/// Identifier pattern for token extraction
static IDENTIFIER_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"[a-zA-Z_][a-zA-Z0-9_]*").unwrap());

// =============================================================================
// PATTERN TYPES
// =============================================================================

struct FunctionPattern {
    regex: Regex,
    kind: SymbolKind,
    #[allow(dead_code)]
    is_exported_group: Option<usize>,
    is_async: bool,
}

struct ClassPattern {
    regex: Regex,
    kind: SymbolKind,
}

struct ImportPattern {
    regex: Regex,
    #[allow(dead_code)]
    language: &'static str,
}

// =============================================================================
// MAIN EXTRACTION FUNCTION
// =============================================================================

/// Extract semantic information from source code using regex patterns
pub fn extract(content: &str, language: &str) -> ExtractedData {
    let mut data = ExtractedData {
        language: language.to_string(),
        extraction_method: ExtractionMethod::Regex,
        ..Default::default()
    };

    // Extract function definitions
    extract_functions(content, &mut data);

    // Extract class/struct definitions
    extract_classes(content, &mut data);

    // Extract function calls
    extract_calls(content, language, &mut data);

    // Extract imports
    extract_imports(content, &mut data);

    // Build basic scope tree (file-level only for regex)
    extract_scopes(content, &mut data);

    // Extract identifiers as tokens
    extract_tokens(content, &mut data);

    data
}

// =============================================================================
// EXTRACTION HELPERS
// =============================================================================

/// Extract function definitions using regex patterns
fn extract_functions(content: &str, data: &mut ExtractedData) {
    let lines: Vec<&str> = content.lines().collect();

    for pattern in FUNCTION_PATTERNS.iter() {
        for caps in pattern.regex.captures_iter(content) {
            if let Some(name_match) = caps.get(1) {
                let name = name_match.as_str().to_string();

                // Skip common keywords that might match
                if is_keyword(&name) {
                    continue;
                }

                // Calculate line number
                let byte_offset = name_match.start();
                let line = content[..byte_offset].matches('\n').count() as u32 + 1;
                let line_start = if line > 1 {
                    content[..byte_offset]
                        .rfind('\n')
                        .map(|i| i + 1)
                        .unwrap_or(0)
                } else {
                    0
                };
                let column = (byte_offset - line_start) as u16;

                // Estimate end line (look for closing brace or end of indented block)
                let end_line = estimate_end_line(&lines, line as usize);

                // Check if exported (simple heuristic)
                let full_match = caps.get(0).map(|m| m.as_str()).unwrap_or("");
                let is_exported = full_match.contains("export")
                    || full_match.starts_with("pub ")
                    || name
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false);

                data.symbols.push(ExtractedSymbol {
                    name,
                    kind: pattern.kind,
                    start_line: line,
                    end_line: end_line as u32,
                    start_column: column,
                    end_column: 0,
                    is_exported,
                    is_async: pattern.is_async || full_match.contains("async"),
                    parent_symbol: None,
                });
            }
        }
    }

    // Deduplicate by name and line
    data.symbols
        .sort_by(|a, b| a.name.cmp(&b.name).then(a.start_line.cmp(&b.start_line)));
    data.symbols
        .dedup_by(|a, b| a.name == b.name && a.start_line == b.start_line);
}

/// Extract class/struct definitions using regex patterns
fn extract_classes(content: &str, data: &mut ExtractedData) {
    let lines: Vec<&str> = content.lines().collect();

    for pattern in CLASS_PATTERNS.iter() {
        for caps in pattern.regex.captures_iter(content) {
            if let Some(name_match) = caps.get(1) {
                let name = name_match.as_str().to_string();

                // Skip common keywords
                if is_keyword(&name) {
                    continue;
                }

                // Calculate line number
                let byte_offset = name_match.start();
                let line = content[..byte_offset].matches('\n').count() as u32 + 1;
                let line_start = if line > 1 {
                    content[..byte_offset]
                        .rfind('\n')
                        .map(|i| i + 1)
                        .unwrap_or(0)
                } else {
                    0
                };
                let column = (byte_offset - line_start) as u16;

                // Estimate end line
                let end_line = estimate_end_line(&lines, line as usize);

                // Check if exported
                let full_match = caps.get(0).map(|m| m.as_str()).unwrap_or("");
                let is_exported = full_match.contains("export")
                    || full_match.starts_with("pub ")
                    || name
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false);

                data.symbols.push(ExtractedSymbol {
                    name,
                    kind: pattern.kind,
                    start_line: line,
                    end_line: end_line as u32,
                    start_column: column,
                    end_column: 0,
                    is_exported,
                    is_async: false,
                    parent_symbol: None,
                });
            }
        }
    }
}

/// Extract function calls using regex patterns
fn extract_calls(content: &str, language: &str, data: &mut ExtractedData) {
    // Extract method calls first (more specific pattern)
    for caps in METHOD_CALL_PATTERN.captures_iter(content) {
        if let (Some(receiver_match), Some(method_match)) = (caps.get(1), caps.get(2)) {
            let receiver = receiver_match.as_str().to_string();
            let callee = method_match.as_str().to_string();

            // Skip keywords and builtins
            if is_keyword(&callee) || is_builtin(&callee, language) {
                continue;
            }

            let byte_offset = method_match.start();
            let line = content[..byte_offset].matches('\n').count() as u32 + 1;
            let line_start = if line > 1 {
                content[..byte_offset]
                    .rfind('\n')
                    .map(|i| i + 1)
                    .unwrap_or(0)
            } else {
                0
            };
            let column = (byte_offset - line_start) as u16;

            data.calls.push(ExtractedCall {
                callee_name: callee,
                line,
                column,
                containing_symbol: None,
                is_method_call: true,
                receiver: Some(receiver),
            });
        }
    }

    // Extract direct function calls
    for caps in CALL_PATTERN.captures_iter(content) {
        if let Some(name_match) = caps.get(1) {
            let callee = name_match.as_str().to_string();

            // Skip keywords, builtins, and already captured method calls
            if is_keyword(&callee) || is_builtin(&callee, language) {
                continue;
            }

            let byte_offset = name_match.start();
            let line = content[..byte_offset].matches('\n').count() as u32 + 1;
            let line_start = if line > 1 {
                content[..byte_offset]
                    .rfind('\n')
                    .map(|i| i + 1)
                    .unwrap_or(0)
            } else {
                0
            };
            let column = (byte_offset - line_start) as u16;

            // Check if this is already captured as a method call at this position
            let already_captured = data
                .calls
                .iter()
                .any(|c| c.line == line && c.column == column);

            if !already_captured {
                data.calls.push(ExtractedCall {
                    callee_name: callee,
                    line,
                    column,
                    containing_symbol: None,
                    is_method_call: false,
                    receiver: None,
                });
            }
        }
    }
}

/// Extract import statements
fn extract_imports(content: &str, data: &mut ExtractedData) {
    for pattern in IMPORT_PATTERNS.iter() {
        for caps in pattern.regex.captures_iter(content) {
            // Try to get the module name from capture group 1 or 2
            let module = caps.get(1).or(caps.get(2));

            if let Some(module_match) = module {
                let module_name = module_match.as_str().to_string();

                let byte_offset = module_match.start();
                let line = content[..byte_offset].matches('\n').count() as u32 + 1;
                let line_start = if line > 1 {
                    content[..byte_offset]
                        .rfind('\n')
                        .map(|i| i + 1)
                        .unwrap_or(0)
                } else {
                    0
                };
                let column = (byte_offset - line_start) as u16;

                data.references.push(ExtractedRef {
                    name: module_name,
                    kind: RefKind::Import,
                    line,
                    column,
                    containing_symbol: None,
                });
            }
        }
    }
}

/// Build basic scope tree (file-level only for regex extraction)
fn extract_scopes(content: &str, data: &mut ExtractedData) {
    let total_lines = content.lines().count() as u32;

    // Add file-level scope
    data.scopes.push(ExtractedScope {
        kind: ScopeKind::File,
        name: None,
        start_line: 1,
        end_line: total_lines.max(1),
        parent_index: None,
    });
}

/// Extract identifiers as tokens
fn extract_tokens(content: &str, data: &mut ExtractedData) {
    for mat in IDENTIFIER_PATTERN.find_iter(content) {
        let name = mat.as_str().to_string();

        // Skip very short identifiers and keywords
        if name.len() < 2 || is_keyword(&name) {
            continue;
        }

        let byte_offset = mat.start();
        let line = content[..byte_offset].matches('\n').count() as u32 + 1;
        let line_start = if line > 1 {
            content[..byte_offset]
                .rfind('\n')
                .map(|i| i + 1)
                .unwrap_or(0)
        } else {
            0
        };
        let column = (byte_offset - line_start) as u16;

        data.tokens.push(ExtractedToken {
            name,
            kind: TokenKind::Identifier,
            line,
            column,
        });
    }
}

// =============================================================================
// HELPERS
// =============================================================================

/// Estimate the end line of a function/class definition
fn estimate_end_line(lines: &[&str], start_line: usize) -> usize {
    if start_line == 0 || start_line > lines.len() {
        return start_line;
    }

    let start_idx = start_line - 1;
    let start_indent = lines
        .get(start_idx)
        .map(|l| l.len() - l.trim_start().len())
        .unwrap_or(0);

    // Look for closing brace at same or less indentation, or dedent in Python-style
    let mut brace_count = 0;
    let mut found_opening = false;

    for (i, line) in lines.iter().enumerate().skip(start_idx) {
        let trimmed = line.trim();

        // Count braces
        for ch in trimmed.chars() {
            match ch {
                '{' => {
                    brace_count += 1;
                    found_opening = true;
                }
                '}' => brace_count -= 1,
                _ => {}
            }
        }

        // If we found opening brace and now closed it
        if found_opening && brace_count == 0 {
            return i + 1;
        }

        // For Python-style (no braces): look for dedent
        if !found_opening && i > start_idx {
            let current_indent = line.len() - line.trim_start().len();
            if !trimmed.is_empty() && current_indent <= start_indent && !trimmed.starts_with('#') {
                return i; // Previous line was the end
            }
        }
    }

    // If no end found, assume it goes to end of file or reasonable limit
    (start_line + 50).min(lines.len())
}

/// Check if a name is a common keyword (language-agnostic common keywords only)
fn is_keyword(name: &str) -> bool {
    matches!(
        name,
        // Control flow (universal)
        "if" | "else" | "for" | "while" | "do" | "switch" | "case" | "default"
        | "break" | "continue" | "return" | "throw" | "try" | "catch" | "finally"
        // Type/declaration keywords
        | "var" | "let" | "const" | "function" | "class" | "extends" | "implements"
        | "import" | "export" | "from" | "as"
        | "async" | "await" | "yield" | "static" | "get" | "set"
        | "public" | "private" | "protected" | "readonly"
        | "true" | "false" | "null" | "undefined" | "void" | "never"
        | "this" | "super" | "constructor"
        // Python
        | "def" | "lambda" | "with" | "assert" | "pass" | "raise"
        | "global" | "nonlocal" | "and" | "or" | "not" | "is"
        | "None" | "True" | "False" | "self" | "cls"
        // Rust (excluding 'new' which is a valid function name)
        | "fn" | "pub" | "mod" | "use" | "crate" | "Self"
        | "struct" | "enum" | "trait" | "impl" | "type" | "where"
        | "mut" | "ref" | "move" | "box" | "dyn" | "unsafe"
        | "loop" | "match" | "Some" | "Ok" | "Err"
        // Go
        | "func" | "package" | "go" | "defer" | "chan" | "select"
        | "map" | "range" | "interface" | "nil"
        // Common C/C++ (excluding 'new' and 'delete' which are operators, not function names)
        | "int" | "float" | "double" | "char" | "string" | "bool" | "boolean"
        | "byte" | "short" | "long" | "signed" | "unsigned"
        | "auto" | "register" | "volatile" | "extern"
        // JavaScript operators that look like identifiers
        | "typeof" | "instanceof" | "in" | "of"
    )
}

/// Check if a name is a common builtin function
fn is_builtin(name: &str, language: &str) -> bool {
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
                    | "setTimeout"
                    | "setInterval"
                    | "clearTimeout"
                    | "clearInterval"
                    | "require"
                    | "define"
                    | "module"
                    | "exports"
                    | "JSON"
                    | "Math"
                    | "Date"
                    | "Array"
                    | "Object"
                    | "String"
                    | "Number"
                    | "Boolean"
                    | "Symbol"
                    | "Map"
                    | "Set"
                    | "WeakMap"
                    | "WeakSet"
                    | "Promise"
                    | "Proxy"
                    | "Reflect"
                    | "Error"
                    | "RegExp"
                    | "encodeURI"
                    | "decodeURI"
                    | "encodeURIComponent"
                    | "decodeURIComponent"
                    | "eval"
                    | "fetch"
                    | "alert"
                    | "confirm"
                    | "prompt"
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
                    | "object"
                    | "isinstance"
                    | "issubclass"
                    | "hasattr"
                    | "getattr"
                    | "setattr"
                    | "delattr"
                    | "open"
                    | "input"
                    | "id"
                    | "dir"
                    | "vars"
                    | "globals"
                    | "locals"
                    | "iter"
                    | "next"
                    | "zip"
                    | "map"
                    | "filter"
                    | "reduce"
                    | "sorted"
                    | "reversed"
                    | "enumerate"
                    | "sum"
                    | "min"
                    | "max"
                    | "abs"
                    | "round"
                    | "pow"
                    | "divmod"
                    | "ord"
                    | "chr"
                    | "hex"
                    | "oct"
                    | "bin"
                    | "format"
                    | "repr"
                    | "hash"
                    | "callable"
                    | "super"
                    | "staticmethod"
                    | "classmethod"
                    | "property"
                    | "compile"
                    | "exec"
                    | "eval"
                    | "help"
                    | "quit"
                    | "exit"
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
                    | "write"
                    | "writeln"
                    | "panic"
                    | "assert"
                    | "assert_eq"
                    | "assert_ne"
                    | "debug_assert"
                    | "debug_assert_eq"
                    | "debug_assert_ne"
                    | "vec"
                    | "todo"
                    | "unimplemented"
                    | "unreachable"
                    | "cfg"
                    | "derive"
                    | "include"
                    | "include_str"
                    | "include_bytes"
                    | "env"
                    | "option_env"
                    | "concat"
                    | "stringify"
                    | "line"
                    | "column"
                    | "file"
                    | "module_path"
                    | "Box"
                    | "Vec"
                    | "String"
                    | "Option"
                    | "Result"
                    | "Arc"
                    | "Rc"
                    | "Cell"
                    | "RefCell"
                    | "Mutex"
                    | "RwLock"
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
                    | "Fprintf"
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
                    | "real"
                    | "imag"
                    | "complex"
                    | "error"
                    | "Error"
                    | "string"
                    | "int"
                    | "bool"
                    | "byte"
                    | "rune"
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
    fn test_extract_javascript_functions() {
        let code = r#"
function greet(name) {
    return "Hello, " + name;
}

export function farewell(name) {
    return "Goodbye, " + name;
}

const add = (a, b) => a + b;

async function fetchData() {
    return await fetch('/api');
}
"#;

        let result = extract(code, "javascript");
        assert!(!result.symbols.is_empty(), "Should extract symbols");

        let func_names: Vec<_> = result.symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(func_names.contains(&"greet"), "Should find greet");
        assert!(func_names.contains(&"farewell"), "Should find farewell");
        assert!(func_names.contains(&"fetchData"), "Should find fetchData");
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

        let result = extract(code, "python");
        assert!(!result.symbols.is_empty(), "Should extract symbols");

        let names: Vec<_> = result.symbols.iter().map(|s| s.name.as_str()).collect();

        assert!(names.contains(&"MyClass"), "Should find MyClass");
        assert!(names.contains(&"__init__"), "Should find __init__");
        assert!(names.contains(&"get_value"), "Should find get_value");
        assert!(
            names.contains(&"standalone_function"),
            "Should find standalone_function"
        );
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

        let result = extract(code, "rust");
        assert!(!result.symbols.is_empty(), "Should extract symbols");

        let names: Vec<_> = result.symbols.iter().map(|s| s.name.as_str()).collect();
        eprintln!("Extracted symbols: {:?}", names);

        assert!(names.contains(&"MyStruct"), "Should find MyStruct");
        assert!(
            names.contains(&"new"),
            "Should find new method, got: {:?}",
            names
        );
        assert!(names.contains(&"get_field"), "Should find get_field");
        assert!(
            names.contains(&"helper_function"),
            "Should find helper_function"
        );
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

        let result = extract(code, "javascript");
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
    fn test_extract_imports_javascript() {
        let code = r#"
import { foo, bar } from './module';
import defaultExport from 'package';
const path = require('path');
"#;

        let result = extract(code, "javascript");
        assert!(!result.references.is_empty(), "Should extract imports");

        let imports: Vec<_> = result
            .references
            .iter()
            .filter(|r| r.kind == RefKind::Import)
            .map(|r| r.name.as_str())
            .collect();

        assert!(imports.contains(&"./module"), "Should find ./module import");
        assert!(imports.contains(&"package"), "Should find package import");
        assert!(imports.contains(&"path"), "Should find path require");
    }

    #[test]
    fn test_is_keyword() {
        assert!(is_keyword("function"));
        assert!(is_keyword("class"));
        assert!(is_keyword("if"));
        assert!(is_keyword("return"));
        assert!(!is_keyword("myFunction"));
        assert!(!is_keyword("customClass"));
    }
}
