//! Core Data Structures for Semantic Index
//!
//! All structures use #[repr(C)] for mmap compatibility and predictable memory layout.
//! This enables zero-copy loading via memory-mapped files.
//!
//! @module trace/types

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

// =============================================================================
// SYMBOL KIND ENUM
// =============================================================================

/// Classification of symbol definitions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum SymbolKind {
    /// Function definition (standalone)
    Function = 0,
    /// Method definition (attached to class/struct)
    Method = 1,
    /// Class definition
    Class = 2,
    /// Struct definition
    Struct = 3,
    /// Enum definition
    Enum = 4,
    /// Interface/Trait definition
    Interface = 5,
    /// Type alias
    TypeAlias = 6,
    /// Constant definition
    Constant = 7,
    /// Variable binding
    Variable = 8,
    /// Module/namespace
    Module = 9,
    /// Unknown symbol type
    Unknown = 255,
}

impl From<u8> for SymbolKind {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Function,
            1 => Self::Method,
            2 => Self::Class,
            3 => Self::Struct,
            4 => Self::Enum,
            5 => Self::Interface,
            6 => Self::TypeAlias,
            7 => Self::Constant,
            8 => Self::Variable,
            9 => Self::Module,
            _ => Self::Unknown,
        }
    }
}

// =============================================================================
// SYMBOL FLAGS
// =============================================================================

bitflags! {
    /// Flags for symbol metadata
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    #[repr(transparent)]
    pub struct SymbolFlags: u8 {
        /// Symbol is an entry point (main, test, exported handler)
        const IS_ENTRY_POINT = 0b0000_0001;
        /// Symbol is exported (pub, export)
        const IS_EXPORTED = 0b0000_0010;
        /// Symbol is async
        const IS_ASYNC = 0b0000_0100;
        /// Symbol is a test function
        const IS_TEST = 0b0000_1000;
        /// Symbol is deprecated
        const IS_DEPRECATED = 0b0001_0000;
        /// Symbol is static/class-level
        const IS_STATIC = 0b0010_0000;
        /// Symbol is abstract
        const IS_ABSTRACT = 0b0100_0000;
        /// Symbol is a constructor
        const IS_CONSTRUCTOR = 0b1000_0000;
    }
}

impl Default for SymbolFlags {
    fn default() -> Self {
        Self::empty()
    }
}

// =============================================================================
// SYMBOL
// =============================================================================

/// A code symbol definition (function, class, method, etc.)
///
/// Layout (24 bytes):
/// - id: u32 (4)
/// - name_offset: u32 (4)
/// - file_id: u16 (2)
/// - kind: u8 (1)
/// - flags: u8 (1)
/// - start_line: u32 (4)
/// - end_line: u32 (4)
/// - _padding: u32 (4) - for alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(C)]
pub struct Symbol {
    /// Unique symbol ID (index into symbols vector)
    pub id: u32,
    /// Offset into string table for the symbol name
    pub name_offset: u32,
    /// File ID where this symbol is defined
    pub file_id: u16,
    /// Symbol kind (function, class, method, etc.)
    pub kind: u8,
    /// Symbol flags (entry point, exported, async, etc.)
    pub flags: u8,
    /// Starting line number (1-indexed)
    pub start_line: u32,
    /// Ending line number (1-indexed)
    pub end_line: u32,
    /// Padding for alignment
    _padding: u32,
}

impl Symbol {
    /// Create a new symbol
    #[inline]
    pub const fn new(
        id: u32,
        name_offset: u32,
        file_id: u16,
        kind: SymbolKind,
        flags: SymbolFlags,
        start_line: u32,
        end_line: u32,
    ) -> Self {
        Self {
            id,
            name_offset,
            file_id,
            kind: kind as u8,
            flags: flags.bits(),
            start_line,
            end_line,
            _padding: 0,
        }
    }

    /// Get the symbol kind
    #[inline]
    pub fn symbol_kind(&self) -> SymbolKind {
        SymbolKind::from(self.kind)
    }

    /// Get the symbol flags
    #[inline]
    pub fn symbol_flags(&self) -> SymbolFlags {
        SymbolFlags::from_bits_truncate(self.flags)
    }

    /// Check if this symbol is an entry point
    #[inline]
    pub fn is_entry_point(&self) -> bool {
        self.symbol_flags().contains(SymbolFlags::IS_ENTRY_POINT)
    }

    /// Check if this symbol is exported
    #[inline]
    pub fn is_exported(&self) -> bool {
        self.symbol_flags().contains(SymbolFlags::IS_EXPORTED)
    }

    /// Check if this symbol is async
    #[inline]
    pub fn is_async(&self) -> bool {
        self.symbol_flags().contains(SymbolFlags::IS_ASYNC)
    }

    /// Check if this symbol is a test
    #[inline]
    pub fn is_test(&self) -> bool {
        self.symbol_flags().contains(SymbolFlags::IS_TEST)
    }
}

// =============================================================================
// TOKEN KIND ENUM
// =============================================================================

/// Classification of tokens (identifiers)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum TokenKind {
    /// Variable/identifier reference
    Identifier = 0,
    /// Function/method call
    Call = 1,
    /// Type annotation
    Type = 2,
    /// Import reference
    Import = 3,
    /// Property access (obj.prop)
    Property = 4,
    /// Decorator/attribute
    Decorator = 5,
    /// Label (for goto/break/continue)
    Label = 6,
    /// Unknown token type
    Unknown = 255,
}

impl From<u8> for TokenKind {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Identifier,
            1 => Self::Call,
            2 => Self::Type,
            3 => Self::Import,
            4 => Self::Property,
            5 => Self::Decorator,
            6 => Self::Label,
            _ => Self::Unknown,
        }
    }
}

// =============================================================================
// TOKEN
// =============================================================================

/// A token (identifier) occurrence in the source code
///
/// Layout (24 bytes):
/// - id: u32 (4)
/// - name_offset: u32 (4)
/// - file_id: u16 (2)
/// - column: u16 (2)
/// - line: u32 (4)
/// - kind: u8 (1)
/// - _padding: [u8; 3] (3)
/// - scope_id: u32 (4)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(C)]
pub struct Token {
    /// Unique token ID
    pub id: u32,
    /// Offset into string table for the token name
    pub name_offset: u32,
    /// File ID where this token appears
    pub file_id: u16,
    /// Column number (0-indexed)
    pub column: u16,
    /// Line number (1-indexed)
    pub line: u32,
    /// Token kind
    pub kind: u8,
    /// Padding for alignment
    _padding: [u8; 3],
    /// Scope ID this token belongs to
    pub scope_id: u32,
}

impl Token {
    /// Create a new token
    #[inline]
    pub const fn new(
        id: u32,
        name_offset: u32,
        file_id: u16,
        line: u32,
        column: u16,
        kind: TokenKind,
        scope_id: u32,
    ) -> Self {
        Self {
            id,
            name_offset,
            file_id,
            column,
            line,
            kind: kind as u8,
            _padding: [0; 3],
            scope_id,
        }
    }

    /// Get the token kind
    #[inline]
    pub fn token_kind(&self) -> TokenKind {
        TokenKind::from(self.kind)
    }
}

// =============================================================================
// REFERENCE KIND ENUM
// =============================================================================

/// Classification of symbol references
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum RefKind {
    /// Read access (using the value)
    Read = 0,
    /// Write access (assigning to it)
    Write = 1,
    /// Function/method call
    Call = 2,
    /// Type annotation usage
    TypeAnnotation = 3,
    /// Import statement
    Import = 4,
    /// Export statement
    Export = 5,
    /// Inheritance (extends/implements)
    Inheritance = 6,
    /// Decorator/attribute usage
    Decorator = 7,
    /// Unknown reference type
    Unknown = 255,
}

impl From<u8> for RefKind {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Read,
            1 => Self::Write,
            2 => Self::Call,
            3 => Self::TypeAnnotation,
            4 => Self::Import,
            5 => Self::Export,
            6 => Self::Inheritance,
            7 => Self::Decorator,
            _ => Self::Unknown,
        }
    }
}

// =============================================================================
// REFERENCE
// =============================================================================

/// A reference from a token to a symbol
///
/// Layout (12 bytes):
/// - token_id: u32 (4)
/// - symbol_id: u32 (4)
/// - kind: u8 (1)
/// - _padding: [u8; 3] (3)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(C)]
pub struct Reference {
    /// Token ID that references the symbol
    pub token_id: u32,
    /// Symbol ID being referenced
    pub symbol_id: u32,
    /// Kind of reference
    pub kind: u8,
    /// Padding for alignment
    _padding: [u8; 3],
}

impl Reference {
    /// Create a new reference
    #[inline]
    pub const fn new(token_id: u32, symbol_id: u32, kind: RefKind) -> Self {
        Self {
            token_id,
            symbol_id,
            kind: kind as u8,
            _padding: [0; 3],
        }
    }

    /// Get the reference kind
    #[inline]
    pub fn ref_kind(&self) -> RefKind {
        RefKind::from(self.kind)
    }

    /// Check if this is a call reference
    #[inline]
    pub fn is_call(&self) -> bool {
        self.ref_kind() == RefKind::Call
    }

    /// Check if this is a read reference
    #[inline]
    pub fn is_read(&self) -> bool {
        self.ref_kind() == RefKind::Read
    }

    /// Check if this is a write reference
    #[inline]
    pub fn is_write(&self) -> bool {
        self.ref_kind() == RefKind::Write
    }
}

// =============================================================================
// SCOPE KIND ENUM
// =============================================================================

/// Classification of scope types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ScopeKind {
    /// File/module level scope
    File = 0,
    /// Module/namespace scope
    Module = 1,
    /// Class body scope
    Class = 2,
    /// Function/method body scope
    Function = 3,
    /// Block scope (if, for, while, etc.)
    Block = 4,
    /// Lambda/closure scope
    Lambda = 5,
    /// Comprehension scope (list comp, etc.)
    Comprehension = 6,
    /// Unknown scope type
    Unknown = 255,
}

impl From<u8> for ScopeKind {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::File,
            1 => Self::Module,
            2 => Self::Class,
            3 => Self::Function,
            4 => Self::Block,
            5 => Self::Lambda,
            6 => Self::Comprehension,
            _ => Self::Unknown,
        }
    }
}

// =============================================================================
// SCOPE
// =============================================================================

/// A scope node in the scope tree
///
/// Layout (24 bytes):
/// - id: u32 (4)
/// - kind: u8 (1)
/// - _padding1: u8 (1)
/// - file_id: u16 (2)
/// - parent_id: u32 (4)
/// - start_line: u32 (4)
/// - end_line: u32 (4)
/// - name_offset: u32 (4)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(C)]
pub struct Scope {
    /// Unique scope ID
    pub id: u32,
    /// Scope kind
    pub kind: u8,
    /// Padding for alignment
    _padding1: u8,
    /// File ID where this scope exists
    pub file_id: u16,
    /// Parent scope ID (u32::MAX for root/file scope)
    pub parent_id: u32,
    /// Starting line number (1-indexed)
    pub start_line: u32,
    /// Ending line number (1-indexed)
    pub end_line: u32,
    /// Offset into string table for scope name (if any)
    pub name_offset: u32,
}

/// Sentinel value for root scope (no parent)
pub const NO_PARENT_SCOPE: u32 = u32::MAX;

impl Scope {
    /// Create a new scope
    #[inline]
    pub const fn new(
        id: u32,
        kind: ScopeKind,
        file_id: u16,
        parent_id: u32,
        start_line: u32,
        end_line: u32,
        name_offset: u32,
    ) -> Self {
        Self {
            id,
            kind: kind as u8,
            _padding1: 0,
            file_id,
            parent_id,
            start_line,
            end_line,
            name_offset,
        }
    }

    /// Create a file-level (root) scope
    #[inline]
    pub const fn file_scope(id: u32, file_id: u16, end_line: u32) -> Self {
        Self::new(
            id,
            ScopeKind::File,
            file_id,
            NO_PARENT_SCOPE,
            1,
            end_line,
            0,
        )
    }

    /// Get the scope kind
    #[inline]
    pub fn scope_kind(&self) -> ScopeKind {
        ScopeKind::from(self.kind)
    }

    /// Check if this is a root/file scope
    #[inline]
    pub fn is_root(&self) -> bool {
        self.parent_id == NO_PARENT_SCOPE
    }

    /// Check if this is a function scope
    #[inline]
    pub fn is_function(&self) -> bool {
        self.scope_kind() == ScopeKind::Function
    }
}

// =============================================================================
// EDGE
// =============================================================================

/// A call graph edge from one symbol to another
///
/// Layout (12 bytes):
/// - from_symbol: u32 (4)
/// - to_symbol: u32 (4)
/// - line: u32 (4)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(C)]
pub struct Edge {
    /// Symbol ID of the caller
    pub from_symbol: u32,
    /// Symbol ID of the callee
    pub to_symbol: u32,
    /// Line number where the call occurs
    pub line: u32,
}

impl Edge {
    /// Create a new edge
    #[inline]
    pub const fn new(from_symbol: u32, to_symbol: u32, line: u32) -> Self {
        Self {
            from_symbol,
            to_symbol,
            line,
        }
    }
}

// =============================================================================
// SIZE ASSERTIONS
// =============================================================================

// Compile-time size checks for mmap compatibility
const _: () = {
    assert!(std::mem::size_of::<Symbol>() == 24);
    assert!(std::mem::size_of::<Token>() == 24);
    assert!(std::mem::size_of::<Reference>() == 12);
    assert!(std::mem::size_of::<Scope>() == 24);
    assert!(std::mem::size_of::<Edge>() == 12);
};

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_kind_roundtrip() {
        for kind in [
            SymbolKind::Function,
            SymbolKind::Method,
            SymbolKind::Class,
            SymbolKind::Struct,
            SymbolKind::Enum,
            SymbolKind::Interface,
            SymbolKind::TypeAlias,
            SymbolKind::Constant,
            SymbolKind::Variable,
            SymbolKind::Module,
        ] {
            assert_eq!(SymbolKind::from(kind as u8), kind);
        }
    }

    #[test]
    fn test_symbol_flags() {
        let flags = SymbolFlags::IS_ENTRY_POINT | SymbolFlags::IS_ASYNC;
        assert!(flags.contains(SymbolFlags::IS_ENTRY_POINT));
        assert!(flags.contains(SymbolFlags::IS_ASYNC));
        assert!(!flags.contains(SymbolFlags::IS_EXPORTED));
    }

    #[test]
    fn test_symbol_creation() {
        let sym = Symbol::new(
            0,
            100,
            1,
            SymbolKind::Function,
            SymbolFlags::IS_ENTRY_POINT | SymbolFlags::IS_EXPORTED,
            10,
            50,
        );
        assert_eq!(sym.id, 0);
        assert_eq!(sym.name_offset, 100);
        assert_eq!(sym.file_id, 1);
        assert_eq!(sym.symbol_kind(), SymbolKind::Function);
        assert!(sym.is_entry_point());
        assert!(sym.is_exported());
        assert!(!sym.is_async());
        assert_eq!(sym.start_line, 10);
        assert_eq!(sym.end_line, 50);
    }

    #[test]
    fn test_reference_kind_roundtrip() {
        for kind in [
            RefKind::Read,
            RefKind::Write,
            RefKind::Call,
            RefKind::TypeAnnotation,
            RefKind::Import,
            RefKind::Export,
            RefKind::Inheritance,
            RefKind::Decorator,
        ] {
            assert_eq!(RefKind::from(kind as u8), kind);
        }
    }

    #[test]
    fn test_scope_hierarchy() {
        let file_scope = Scope::file_scope(0, 0, 100);
        assert!(file_scope.is_root());
        assert_eq!(file_scope.scope_kind(), ScopeKind::File);

        let fn_scope = Scope::new(1, ScopeKind::Function, 0, 0, 10, 20, 50);
        assert!(!fn_scope.is_root());
        assert!(fn_scope.is_function());
        assert_eq!(fn_scope.parent_id, 0);
    }

    #[test]
    fn test_edge_creation() {
        let edge = Edge::new(1, 2, 42);
        assert_eq!(edge.from_symbol, 1);
        assert_eq!(edge.to_symbol, 2);
        assert_eq!(edge.line, 42);
    }
}
