//! Binary Storage with Memory Mapping
//!
//! Provides fast serialization and deserialization of the SemanticIndex
//! using memory-mapped files for instant loading.
//!
//! File format:
//! - Header (32 bytes): magic, version, counts, offsets
//! - Symbols section
//! - Tokens section
//! - References section
//! - Scopes section
//! - Edges section
//! - Files section (length-prefixed paths)
//! - Strings section (null-terminated)
//!
//! @module trace/storage

use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;

use memmap2::Mmap;

use super::index::{SemanticIndex, StringTable};
use super::types::{Edge, Reference, Scope, Symbol, Token};
use crate::core::error::{Error, Result};

// =============================================================================
// CONSTANTS
// =============================================================================

/// Magic bytes to identify greppy trace index files
const MAGIC: [u8; 8] = *b"GRPTRACE";

/// Current file format version
const VERSION: u32 = 1;

/// Header size in bytes
const HEADER_SIZE: usize = 64;

// =============================================================================
// FILE HEADER
// =============================================================================

/// File header for the binary index format
///
/// Layout (64 bytes):
/// - magic: [u8; 8] = 8 bytes
/// - version: u32 = 4 bytes
/// - _reserved: u32 = 4 bytes
/// - symbol_count: u32 = 4 bytes
/// - token_count: u32 = 4 bytes
/// - reference_count: u32 = 4 bytes
/// - scope_count: u32 = 4 bytes
/// - edge_count: u32 = 4 bytes
/// - file_count: u32 = 4 bytes
/// - string_size: u32 = 4 bytes
/// - _padding: [u8; 20] = 20 bytes
/// Total: 8 + 36 + 20 = 64 bytes
#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct Header {
    /// Magic bytes
    magic: [u8; 8],
    /// Format version
    version: u32,
    /// Reserved for future use
    _reserved: u32,
    /// Number of symbols
    symbol_count: u32,
    /// Number of tokens
    token_count: u32,
    /// Number of references
    reference_count: u32,
    /// Number of scopes
    scope_count: u32,
    /// Number of edges
    edge_count: u32,
    /// Number of files
    file_count: u32,
    /// Size of string table in bytes
    string_size: u32,
    /// Padding to 64 bytes
    _padding: [u8; 20],
}

impl Header {
    fn new(index: &SemanticIndex) -> Self {
        Self {
            magic: MAGIC,
            version: VERSION,
            _reserved: 0,
            symbol_count: index.symbols.len() as u32,
            token_count: index.tokens.len() as u32,
            reference_count: index.references.len() as u32,
            scope_count: index.scopes.len() as u32,
            edge_count: index.edges.len() as u32,
            file_count: index.files.len() as u32,
            string_size: index.strings.byte_size() as u32,
            _padding: [0; 20],
        }
    }

    fn validate(&self) -> Result<()> {
        if self.magic != MAGIC {
            return Err(Error::IndexError {
                message: "Invalid trace index file (bad magic)".into(),
            });
        }
        let version = self.version;
        if version != VERSION {
            return Err(Error::IndexError {
                message: format!(
                    "Unsupported trace index version {} (expected {})",
                    version, VERSION
                ),
            });
        }
        Ok(())
    }

    fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self as *const Self as *const u8,
                std::mem::size_of::<Self>(),
            )
        }
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < std::mem::size_of::<Self>() {
            return Err(Error::IndexError {
                message: "Invalid trace index file (header too small)".into(),
            });
        }

        // Safety: Header is repr(C, packed) with fixed layout
        let header = unsafe { std::ptr::read_unaligned(bytes.as_ptr() as *const Self) };

        header.validate()?;
        Ok(header)
    }
}

// Compile-time size check
const _: () = {
    assert!(std::mem::size_of::<Header>() == HEADER_SIZE);
};

// =============================================================================
// SAVE INDEX
// =============================================================================

/// Save a SemanticIndex to a binary file
pub fn save_index(index: &SemanticIndex, path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    let file = File::create(path)?;
    let mut writer = BufWriter::with_capacity(64 * 1024, file);

    // Write header
    let header = Header::new(index);
    writer.write_all(header.as_bytes())?;

    // Write symbols
    write_slice(&mut writer, &index.symbols)?;

    // Write tokens
    write_slice(&mut writer, &index.tokens)?;

    // Write references
    write_slice(&mut writer, &index.references)?;

    // Write scopes
    write_slice(&mut writer, &index.scopes)?;

    // Write edges
    write_slice(&mut writer, &index.edges)?;

    // Write files (length-prefixed UTF-8 paths)
    for path in &index.files {
        let path_bytes = path.to_string_lossy().as_bytes().to_vec();
        let len = path_bytes.len() as u32;
        writer.write_all(&len.to_le_bytes())?;
        writer.write_all(&path_bytes)?;
    }

    // Write string table
    writer.write_all(index.strings.as_bytes())?;

    writer.flush()?;
    Ok(())
}

/// Write a slice of repr(C) types to the writer
fn write_slice<T, W: Write>(writer: &mut W, slice: &[T]) -> io::Result<()> {
    let bytes = unsafe {
        std::slice::from_raw_parts(
            slice.as_ptr() as *const u8,
            slice.len() * std::mem::size_of::<T>(),
        )
    };
    writer.write_all(bytes)
}

// =============================================================================
// LOAD INDEX
// =============================================================================

/// Load a SemanticIndex from a binary file
///
/// This uses memory mapping for fast access to large indices.
pub fn load_index(path: impl AsRef<Path>) -> Result<SemanticIndex> {
    let path = path.as_ref();
    let file = File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };

    // Parse header
    let header = Header::from_bytes(&mmap)?;
    let mut offset = HEADER_SIZE;

    // Read symbols
    let symbols: Vec<Symbol> = read_vec(&mmap, &mut offset, header.symbol_count as usize)?;

    // Read tokens
    let tokens: Vec<Token> = read_vec(&mmap, &mut offset, header.token_count as usize)?;

    // Read references
    let references: Vec<Reference> = read_vec(&mmap, &mut offset, header.reference_count as usize)?;

    // Read scopes
    let scopes: Vec<Scope> = read_vec(&mmap, &mut offset, header.scope_count as usize)?;

    // Read edges
    let edges: Vec<Edge> = read_vec(&mmap, &mut offset, header.edge_count as usize)?;

    // Read files
    let mut files = Vec::with_capacity(header.file_count as usize);
    for _ in 0..header.file_count {
        if offset + 4 > mmap.len() {
            return Err(Error::IndexError {
                message: "Truncated trace index file (files section)".into(),
            });
        }
        let len = u32::from_le_bytes([
            mmap[offset],
            mmap[offset + 1],
            mmap[offset + 2],
            mmap[offset + 3],
        ]) as usize;
        offset += 4;

        if offset + len > mmap.len() {
            return Err(Error::IndexError {
                message: "Truncated trace index file (file path)".into(),
            });
        }
        let path_str =
            std::str::from_utf8(&mmap[offset..offset + len]).map_err(|e| Error::IndexError {
                message: format!("Invalid UTF-8 in file path: {}", e),
            })?;
        files.push(path_str.into());
        offset += len;
    }

    // Read string table
    let string_bytes = if offset < mmap.len() {
        mmap[offset..].to_vec()
    } else {
        Vec::new()
    };
    let strings = StringTable::from_bytes(string_bytes);

    // Build the index
    let mut index = SemanticIndex {
        symbols,
        tokens,
        references,
        scopes,
        edges,
        symbol_by_name: Default::default(),
        token_by_name: Default::default(),
        incoming_edges: Default::default(),
        outgoing_edges: Default::default(),
        refs_to_symbol: Default::default(),
        files,
        strings,
        entry_points: Default::default(),
    };

    // Rebuild lookup structures
    index.rebuild_lookups();

    Ok(index)
}

/// Read a vector of repr(C) types from memory
fn read_vec<T: Clone>(mmap: &Mmap, offset: &mut usize, count: usize) -> Result<Vec<T>> {
    let size = count * std::mem::size_of::<T>();
    if *offset + size > mmap.len() {
        return Err(Error::IndexError {
            message: format!(
                "Truncated trace index file at offset {} (need {} bytes, have {})",
                offset,
                size,
                mmap.len() - *offset
            ),
        });
    }

    let slice = &mmap[*offset..*offset + size];
    *offset += size;

    // Safety: We're reading repr(C) packed structs with known layout
    let result = unsafe {
        let ptr = slice.as_ptr() as *const T;
        std::slice::from_raw_parts(ptr, count).to_vec()
    };

    Ok(result)
}

// =============================================================================
// LOAD INDEX (STREAMING)
// =============================================================================

/// Load a SemanticIndex from a file using streaming (no mmap)
///
/// Use this for smaller files or when mmap is not available.
pub fn load_index_streaming(path: impl AsRef<Path>) -> Result<SemanticIndex> {
    let path = path.as_ref();
    let file = File::open(path)?;
    let mut reader = BufReader::with_capacity(64 * 1024, file);

    // Read header
    let mut header_bytes = [0u8; HEADER_SIZE];
    reader.read_exact(&mut header_bytes)?;
    let header = Header::from_bytes(&header_bytes)?;

    // Read symbols
    let symbols: Vec<Symbol> = read_vec_streaming(&mut reader, header.symbol_count as usize)?;

    // Read tokens
    let tokens: Vec<Token> = read_vec_streaming(&mut reader, header.token_count as usize)?;

    // Read references
    let references: Vec<Reference> =
        read_vec_streaming(&mut reader, header.reference_count as usize)?;

    // Read scopes
    let scopes: Vec<Scope> = read_vec_streaming(&mut reader, header.scope_count as usize)?;

    // Read edges
    let edges: Vec<Edge> = read_vec_streaming(&mut reader, header.edge_count as usize)?;

    // Read files
    let mut files = Vec::with_capacity(header.file_count as usize);
    for _ in 0..header.file_count {
        let mut len_bytes = [0u8; 4];
        reader.read_exact(&mut len_bytes)?;
        let len = u32::from_le_bytes(len_bytes) as usize;

        let mut path_bytes = vec![0u8; len];
        reader.read_exact(&mut path_bytes)?;
        let path_str = std::str::from_utf8(&path_bytes).map_err(|e| Error::IndexError {
            message: format!("Invalid UTF-8 in file path: {}", e),
        })?;
        files.push(path_str.into());
    }

    // Read string table
    let mut string_bytes = Vec::new();
    reader.read_to_end(&mut string_bytes)?;
    let strings = StringTable::from_bytes(string_bytes);

    // Build the index
    let mut index = SemanticIndex {
        symbols,
        tokens,
        references,
        scopes,
        edges,
        symbol_by_name: Default::default(),
        token_by_name: Default::default(),
        incoming_edges: Default::default(),
        outgoing_edges: Default::default(),
        refs_to_symbol: Default::default(),
        files,
        strings,
        entry_points: Default::default(),
    };

    // Rebuild lookup structures
    index.rebuild_lookups();

    Ok(index)
}

/// Read a vector of repr(C) types from a reader
fn read_vec_streaming<T: Clone, R: Read>(reader: &mut R, count: usize) -> io::Result<Vec<T>> {
    let size = count * std::mem::size_of::<T>();
    let mut bytes = vec![0u8; size];
    reader.read_exact(&mut bytes)?;

    // Safety: We're reading repr(C) packed structs with known layout
    let result = unsafe {
        let ptr = bytes.as_ptr() as *const T;
        std::slice::from_raw_parts(ptr, count).to_vec()
    };

    Ok(result)
}

// =============================================================================
// UTILITIES
// =============================================================================

/// Get the default trace index path for a project
pub fn trace_index_path(project_root: impl AsRef<Path>) -> std::path::PathBuf {
    project_root.as_ref().join(".greppy").join("trace.idx")
}

/// Check if a trace index exists for a project
pub fn trace_index_exists(project_root: impl AsRef<Path>) -> bool {
    trace_index_path(project_root).exists()
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::types::{RefKind, ScopeKind, SymbolFlags, SymbolKind, TokenKind};
    use tempfile::tempdir;

    fn create_test_index() -> SemanticIndex {
        let mut index = SemanticIndex::new();

        // Add files
        let file_id = index.add_file("src/main.rs".into());

        // Add symbols
        let name1 = index.strings.intern("main");
        let name2 = index.strings.intern("helper");

        index.add_symbol(
            Symbol::new(
                0,
                name1,
                file_id,
                SymbolKind::Function,
                SymbolFlags::IS_ENTRY_POINT,
                1,
                10,
            ),
            "main",
        );
        index.add_symbol(
            Symbol::new(
                1,
                name2,
                file_id,
                SymbolKind::Function,
                SymbolFlags::empty(),
                12,
                20,
            ),
            "helper",
        );

        // Add tokens
        index.add_token(
            Token::new(0, name1, file_id, 1, 4, TokenKind::Identifier, 0),
            "main",
        );
        index.add_token(
            Token::new(1, name2, file_id, 5, 4, TokenKind::Call, 0),
            "helper",
        );

        // Add references
        index.add_reference(Reference::new(1, 1, RefKind::Call));

        // Add scopes
        index.add_scope(Scope::file_scope(0, file_id, 25));
        index.add_scope(Scope::new(1, ScopeKind::Function, file_id, 0, 1, 10, name1));

        // Add edges
        index.add_edge(Edge::new(0, 1, 5));

        index
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("trace.idx");

        let original = create_test_index();
        save_index(&original, &path).unwrap();

        let loaded = load_index(&path).unwrap();

        // Verify counts
        assert_eq!(loaded.symbols.len(), original.symbols.len());
        assert_eq!(loaded.tokens.len(), original.tokens.len());
        assert_eq!(loaded.references.len(), original.references.len());
        assert_eq!(loaded.scopes.len(), original.scopes.len());
        assert_eq!(loaded.edges.len(), original.edges.len());
        assert_eq!(loaded.files.len(), original.files.len());

        // Verify lookups work
        assert!(loaded.symbols_by_name("main").is_some());
        assert!(loaded.symbols_by_name("helper").is_some());
        assert_eq!(loaded.entry_points.len(), 1);

        // Verify call graph
        assert_eq!(loaded.callers(1), &[0]);
        assert_eq!(loaded.callees(0), &[1]);
    }

    #[test]
    fn test_streaming_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("trace.idx");

        let original = create_test_index();
        save_index(&original, &path).unwrap();

        let loaded = load_index_streaming(&path).unwrap();

        assert_eq!(loaded.symbols.len(), original.symbols.len());
        assert_eq!(loaded.tokens.len(), original.tokens.len());
    }

    #[test]
    fn test_header_validation() {
        // Test invalid magic
        let bad_magic = [0u8; HEADER_SIZE];
        assert!(Header::from_bytes(&bad_magic).is_err());

        // Test valid header
        let mut valid = [0u8; HEADER_SIZE];
        valid[..8].copy_from_slice(&MAGIC);
        valid[8..12].copy_from_slice(&VERSION.to_le_bytes());
        assert!(Header::from_bytes(&valid).is_ok());
    }
}
