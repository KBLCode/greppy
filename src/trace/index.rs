//! Semantic Index - The core in-memory data structure
//!
//! SemanticIndex holds all symbols, tokens, references, scopes, and edges
//! with fast lookup structures for instant queries.
//!
//! @module trace/index

use std::collections::HashMap;
use std::path::PathBuf;

use compact_str::CompactString;
use smallvec::SmallVec;

use super::types::{Edge, RefKind, Reference, Scope, Symbol, SymbolKind, Token};

// =============================================================================
// STRING TABLE
// =============================================================================

/// Interned string storage for memory efficiency
///
/// All symbol and token names are stored once in this table,
/// and structures reference them by offset.
#[derive(Debug, Clone, Default)]
pub struct StringTable {
    /// Raw bytes of all interned strings (null-terminated)
    data: Vec<u8>,
    /// Map from string to offset for deduplication
    lookup: HashMap<CompactString, u32>,
}

impl StringTable {
    /// Create a new empty string table
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with pre-allocated capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            lookup: HashMap::with_capacity(capacity / 16), // Assume ~16 bytes per string
        }
    }

    /// Intern a string, returning its offset
    ///
    /// If the string already exists, returns the existing offset.
    pub fn intern(&mut self, s: &str) -> u32 {
        // Check if already interned
        let compact = CompactString::new(s);
        if let Some(&offset) = self.lookup.get(&compact) {
            return offset;
        }

        // Add new string
        let offset = self.data.len() as u32;
        self.data.extend_from_slice(s.as_bytes());
        self.data.push(0); // Null terminator
        self.lookup.insert(compact, offset);
        offset
    }

    /// Get a string by offset
    ///
    /// # Safety
    /// The offset must be valid and point to a null-terminated string
    pub fn get(&self, offset: u32) -> Option<&str> {
        let start = offset as usize;
        if start >= self.data.len() {
            return None;
        }

        // Find the null terminator
        let end = self.data[start..]
            .iter()
            .position(|&b| b == 0)
            .map(|pos| start + pos)?;

        std::str::from_utf8(&self.data[start..end]).ok()
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Load from raw bytes (for mmap)
    pub fn from_bytes(data: Vec<u8>) -> Self {
        // Rebuild lookup table
        let mut lookup = HashMap::new();
        let mut offset = 0usize;
        while offset < data.len() {
            // Find null terminator
            let end = data[offset..]
                .iter()
                .position(|&b| b == 0)
                .map(|pos| offset + pos);

            if let Some(end) = end {
                if let Ok(s) = std::str::from_utf8(&data[offset..end]) {
                    lookup.insert(CompactString::new(s), offset as u32);
                }
                offset = end + 1;
            } else {
                break;
            }
        }

        Self { data, lookup }
    }

    /// Number of interned strings
    pub fn len(&self) -> usize {
        self.lookup.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.lookup.is_empty()
    }

    /// Total bytes used
    pub fn byte_size(&self) -> usize {
        self.data.len()
    }
}

// =============================================================================
// SEMANTIC INDEX
// =============================================================================

/// The main semantic index containing all code structure information
///
/// This structure is designed for:
/// - Fast lookup by name (HashMap)
/// - Fast traversal (adjacency lists with SmallVec)
/// - Memory efficiency (interned strings, compact types)
/// - Mmap compatibility (repr(C) data types)
#[derive(Debug)]
pub struct SemanticIndex {
    // -------------------------------------------------------------------------
    // Primary Data (mmap'd)
    // -------------------------------------------------------------------------
    /// All symbol definitions
    pub symbols: Vec<Symbol>,

    /// All token occurrences
    pub tokens: Vec<Token>,

    /// All symbol references
    pub references: Vec<Reference>,

    /// All scopes
    pub scopes: Vec<Scope>,

    /// All call graph edges
    pub edges: Vec<Edge>,

    // -------------------------------------------------------------------------
    // Fast Lookups (built at load time)
    // -------------------------------------------------------------------------
    /// Symbol lookup by name -> list of symbol IDs with that name
    /// SmallVec<[u32; 4]> because most names have 1-4 definitions
    pub symbol_by_name: HashMap<CompactString, SmallVec<[u32; 4]>>,

    /// Token lookup by name -> list of token IDs with that name
    pub token_by_name: HashMap<CompactString, Vec<u32>>,

    /// Incoming edges per symbol (who calls this symbol)
    /// SmallVec<[u32; 8]> because most functions have <8 callers
    pub incoming_edges: Vec<SmallVec<[u32; 8]>>,

    /// Outgoing edges per symbol (what does this symbol call)
    /// SmallVec<[u32; 8]> because most functions call <8 others
    pub outgoing_edges: Vec<SmallVec<[u32; 8]>>,

    /// References to each symbol (index by symbol_id)
    pub refs_to_symbol: Vec<Vec<u32>>,

    // -------------------------------------------------------------------------
    // Metadata
    // -------------------------------------------------------------------------
    /// List of indexed files (index = file_id)
    pub files: Vec<PathBuf>,

    /// Interned string storage
    pub strings: StringTable,

    /// Entry point symbol IDs (for fast traversal starting points)
    pub entry_points: Vec<u32>,
}

impl SemanticIndex {
    /// Create a new empty semantic index
    pub fn new() -> Self {
        Self {
            symbols: Vec::new(),
            tokens: Vec::new(),
            references: Vec::new(),
            scopes: Vec::new(),
            edges: Vec::new(),
            symbol_by_name: HashMap::new(),
            token_by_name: HashMap::new(),
            incoming_edges: Vec::new(),
            outgoing_edges: Vec::new(),
            refs_to_symbol: Vec::new(),
            files: Vec::new(),
            strings: StringTable::new(),
            entry_points: Vec::new(),
        }
    }

    /// Create with estimated capacity for better allocation
    pub fn with_capacity(
        symbols: usize,
        tokens: usize,
        references: usize,
        scopes: usize,
        edges: usize,
        files: usize,
    ) -> Self {
        Self {
            symbols: Vec::with_capacity(symbols),
            tokens: Vec::with_capacity(tokens),
            references: Vec::with_capacity(references),
            scopes: Vec::with_capacity(scopes),
            edges: Vec::with_capacity(edges),
            symbol_by_name: HashMap::with_capacity(symbols),
            token_by_name: HashMap::with_capacity(tokens / 4), // Many tokens share names
            incoming_edges: Vec::with_capacity(symbols),
            outgoing_edges: Vec::with_capacity(symbols),
            refs_to_symbol: Vec::with_capacity(symbols),
            files: Vec::with_capacity(files),
            strings: StringTable::with_capacity(symbols * 20), // ~20 bytes per symbol name
            entry_points: Vec::with_capacity(files),           // ~1 entry point per file
        }
    }

    // -------------------------------------------------------------------------
    // Building Methods
    // -------------------------------------------------------------------------

    /// Add a file to the index, returning its file_id
    pub fn add_file(&mut self, path: PathBuf) -> u16 {
        let id = self.files.len() as u16;
        self.files.push(path);
        id
    }

    /// Add a symbol to the index
    pub fn add_symbol(&mut self, symbol: Symbol, name: &str) {
        let id = symbol.id as usize;

        // Store the symbol
        if id >= self.symbols.len() {
            self.symbols.resize(
                id + 1,
                Symbol::new(0, 0, 0, SymbolKind::Unknown, Default::default(), 0, 0),
            );
        }
        self.symbols[id] = symbol;

        // Update name lookup
        let compact_name = CompactString::new(name);
        self.symbol_by_name
            .entry(compact_name)
            .or_insert_with(SmallVec::new)
            .push(symbol.id);

        // Track entry points
        if symbol.is_entry_point() {
            self.entry_points.push(symbol.id);
        }

        // Ensure adjacency lists are sized
        while self.incoming_edges.len() <= id {
            self.incoming_edges.push(SmallVec::new());
        }
        while self.outgoing_edges.len() <= id {
            self.outgoing_edges.push(SmallVec::new());
        }
        while self.refs_to_symbol.len() <= id {
            self.refs_to_symbol.push(Vec::new());
        }
    }

    /// Add a token to the index
    pub fn add_token(&mut self, token: Token, name: &str) {
        let id = token.id as usize;

        // Store the token
        if id >= self.tokens.len() {
            self.tokens.resize(
                id + 1,
                Token::new(0, 0, 0, 0, 0, super::types::TokenKind::Unknown, 0),
            );
        }
        self.tokens[id] = token;

        // Update name lookup
        let compact_name = CompactString::new(name);
        self.token_by_name
            .entry(compact_name)
            .or_insert_with(Vec::new)
            .push(token.id);
    }

    /// Add a reference to the index
    pub fn add_reference(&mut self, reference: Reference) {
        self.references.push(reference);

        // Update refs_to_symbol lookup
        let sym_id = reference.symbol_id as usize;
        if sym_id < self.refs_to_symbol.len() {
            self.refs_to_symbol[sym_id].push(reference.token_id);
        }
    }

    /// Add a scope to the index
    pub fn add_scope(&mut self, scope: Scope) {
        let id = scope.id as usize;
        if id >= self.scopes.len() {
            self.scopes.resize(id + 1, Scope::file_scope(0, 0, 0));
        }
        self.scopes[id] = scope;
    }

    /// Add an edge to the index
    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);

        // Update adjacency lists
        let from = edge.from_symbol as usize;
        let to = edge.to_symbol as usize;

        if from < self.outgoing_edges.len() {
            self.outgoing_edges[from].push(edge.to_symbol);
        }
        if to < self.incoming_edges.len() {
            self.incoming_edges[to].push(edge.from_symbol);
        }
    }

    /// Rebuild all lookup structures from raw data
    ///
    /// Call this after loading from storage to populate HashMaps and adjacency lists.
    pub fn rebuild_lookups(&mut self) {
        // Clear existing lookups
        self.symbol_by_name.clear();
        self.token_by_name.clear();
        self.incoming_edges.clear();
        self.outgoing_edges.clear();
        self.refs_to_symbol.clear();
        self.entry_points.clear();

        // Resize adjacency lists
        self.incoming_edges
            .resize(self.symbols.len(), SmallVec::new());
        self.outgoing_edges
            .resize(self.symbols.len(), SmallVec::new());
        self.refs_to_symbol.resize(self.symbols.len(), Vec::new());

        // Rebuild symbol_by_name and entry_points
        for symbol in &self.symbols {
            if let Some(name) = self.strings.get(symbol.name_offset) {
                self.symbol_by_name
                    .entry(CompactString::new(name))
                    .or_insert_with(SmallVec::new)
                    .push(symbol.id);
            }
            if symbol.is_entry_point() {
                self.entry_points.push(symbol.id);
            }
        }

        // Rebuild token_by_name
        for token in &self.tokens {
            if let Some(name) = self.strings.get(token.name_offset) {
                self.token_by_name
                    .entry(CompactString::new(name))
                    .or_insert_with(Vec::new)
                    .push(token.id);
            }
        }

        // Rebuild adjacency lists from edges
        for edge in &self.edges {
            let from = edge.from_symbol as usize;
            let to = edge.to_symbol as usize;
            if from < self.outgoing_edges.len() {
                self.outgoing_edges[from].push(edge.to_symbol);
            }
            if to < self.incoming_edges.len() {
                self.incoming_edges[to].push(edge.from_symbol);
            }
        }

        // Rebuild refs_to_symbol
        for reference in &self.references {
            let sym_id = reference.symbol_id as usize;
            if sym_id < self.refs_to_symbol.len() {
                self.refs_to_symbol[sym_id].push(reference.token_id);
            }
        }
    }

    // -------------------------------------------------------------------------
    // Query Methods
    // -------------------------------------------------------------------------

    /// Find symbols by exact name
    pub fn symbols_by_name(&self, name: &str) -> Option<&SmallVec<[u32; 4]>> {
        self.symbol_by_name.get(&CompactString::new(name))
    }

    /// Find symbols matching a name pattern (substring match)
    pub fn symbols_matching(&self, pattern: &str) -> Vec<u32> {
        let pattern_lower = pattern.to_lowercase();
        self.symbol_by_name
            .iter()
            .filter(|(name, _)| name.to_lowercase().contains(&pattern_lower))
            .flat_map(|(_, ids)| ids.iter().copied())
            .collect()
    }

    /// Find tokens by exact name
    pub fn tokens_by_name(&self, name: &str) -> Option<&Vec<u32>> {
        self.token_by_name.get(&CompactString::new(name))
    }

    /// Find tokens matching a name pattern (substring match)
    pub fn tokens_matching(&self, pattern: &str) -> Vec<u32> {
        let pattern_lower = pattern.to_lowercase();
        self.token_by_name
            .iter()
            .filter(|(name, _)| name.to_lowercase().contains(&pattern_lower))
            .flat_map(|(_, ids)| ids.iter().copied())
            .collect()
    }

    /// Get a symbol by ID
    #[inline]
    pub fn symbol(&self, id: u32) -> Option<&Symbol> {
        self.symbols.get(id as usize)
    }

    /// Get a token by ID
    #[inline]
    pub fn token(&self, id: u32) -> Option<&Token> {
        self.tokens.get(id as usize)
    }

    /// Get a scope by ID
    #[inline]
    pub fn scope(&self, id: u32) -> Option<&Scope> {
        self.scopes.get(id as usize)
    }

    /// Get the name of a symbol
    pub fn symbol_name(&self, symbol: &Symbol) -> Option<&str> {
        self.strings.get(symbol.name_offset)
    }

    /// Get the name of a token
    pub fn token_name(&self, token: &Token) -> Option<&str> {
        self.strings.get(token.name_offset)
    }

    /// Get the file path for a file_id
    pub fn file_path(&self, file_id: u16) -> Option<&PathBuf> {
        self.files.get(file_id as usize)
    }

    /// Get symbols that call a given symbol (incoming edges)
    pub fn callers(&self, symbol_id: u32) -> &[u32] {
        self.incoming_edges
            .get(symbol_id as usize)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get symbols that a given symbol calls (outgoing edges)
    pub fn callees(&self, symbol_id: u32) -> &[u32] {
        self.outgoing_edges
            .get(symbol_id as usize)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get all references to a symbol
    pub fn references_to(&self, symbol_id: u32) -> impl Iterator<Item = &Reference> + '_ {
        self.refs_to_symbol
            .get(symbol_id as usize)
            .into_iter()
            .flat_map(move |token_ids| {
                let sid = symbol_id;
                token_ids.iter().filter_map(move |&tid| {
                    self.references
                        .iter()
                        .find(|r| r.token_id == tid && r.symbol_id == sid)
                })
            })
    }

    /// Get references of a specific kind to a symbol
    pub fn references_of_kind(
        &self,
        symbol_id: u32,
        kind: RefKind,
    ) -> impl Iterator<Item = &Reference> {
        self.references_to(symbol_id)
            .filter(move |r| r.ref_kind() == kind)
    }

    /// Get call references to a symbol
    pub fn call_references(&self, symbol_id: u32) -> impl Iterator<Item = &Reference> {
        self.references_of_kind(symbol_id, RefKind::Call)
    }

    /// Get all entry point symbols
    pub fn entry_point_symbols(&self) -> impl Iterator<Item = &Symbol> {
        self.entry_points.iter().filter_map(|&id| self.symbol(id))
    }

    /// Find symbols in a specific file
    pub fn symbols_in_file(&self, file_id: u16) -> impl Iterator<Item = &Symbol> {
        self.symbols.iter().filter(move |s| s.file_id == file_id)
    }

    /// Find tokens in a specific file
    pub fn tokens_in_file(&self, file_id: u16) -> impl Iterator<Item = &Token> {
        self.tokens.iter().filter(move |t| t.file_id == file_id)
    }

    // -------------------------------------------------------------------------
    // Statistics
    // -------------------------------------------------------------------------

    /// Get index statistics
    pub fn stats(&self) -> IndexStats {
        IndexStats {
            symbols: self.symbols.len(),
            tokens: self.tokens.len(),
            references: self.references.len(),
            scopes: self.scopes.len(),
            edges: self.edges.len(),
            files: self.files.len(),
            entry_points: self.entry_points.len(),
            string_bytes: self.strings.byte_size(),
            unique_names: self.strings.len(),
        }
    }

    // -------------------------------------------------------------------------
    // Incremental Update Methods
    // -------------------------------------------------------------------------

    /// Find file_id for a given path, if it exists in the index
    pub fn file_id_for_path(&self, path: &std::path::Path) -> Option<u16> {
        self.files.iter().position(|p| p == path).map(|i| i as u16)
    }

    /// Remove all data associated with a specific file.
    ///
    /// This removes symbols, tokens, references, scopes, and edges for the file,
    /// and cleans up the lookup structures accordingly.
    ///
    /// Returns the number of symbols removed.
    pub fn remove_file_data(&mut self, file_id: u16) -> usize {
        // Collect symbol IDs to remove
        let symbols_to_remove: Vec<u32> = self
            .symbols
            .iter()
            .filter(|s| s.file_id == file_id)
            .map(|s| s.id)
            .collect();

        if symbols_to_remove.is_empty() {
            return 0;
        }

        let removed_count = symbols_to_remove.len();

        // Create a set for fast lookup
        let symbol_set: std::collections::HashSet<u32> =
            symbols_to_remove.iter().copied().collect();

        // Remove symbols from symbol_by_name lookup
        for symbol_id in &symbols_to_remove {
            if let Some(symbol) = self.symbols.get(*symbol_id as usize) {
                if let Some(name) = self.strings.get(symbol.name_offset) {
                    let compact_name = CompactString::new(name);
                    if let Some(ids) = self.symbol_by_name.get_mut(&compact_name) {
                        ids.retain(|id| *id != *symbol_id);
                        if ids.is_empty() {
                            self.symbol_by_name.remove(&compact_name);
                        }
                    }
                }
            }
        }

        // Remove from entry_points
        self.entry_points.retain(|&id| !symbol_set.contains(&id));

        // Clear adjacency lists for removed symbols and remove edges pointing to/from them
        for &symbol_id in &symbols_to_remove {
            let idx = symbol_id as usize;
            if idx < self.incoming_edges.len() {
                self.incoming_edges[idx].clear();
            }
            if idx < self.outgoing_edges.len() {
                self.outgoing_edges[idx].clear();
            }
            if idx < self.refs_to_symbol.len() {
                self.refs_to_symbol[idx].clear();
            }
        }

        // Remove edges involving these symbols
        self.edges
            .retain(|e| !symbol_set.contains(&e.from_symbol) && !symbol_set.contains(&e.to_symbol));

        // Rebuild adjacency lists from remaining edges (simpler than surgical removal)
        for adj in &mut self.incoming_edges {
            adj.retain(|id| !symbol_set.contains(id));
        }
        for adj in &mut self.outgoing_edges {
            adj.retain(|id| !symbol_set.contains(id));
        }

        // Collect token IDs to remove
        let tokens_to_remove: Vec<u32> = self
            .tokens
            .iter()
            .filter(|t| t.file_id == file_id)
            .map(|t| t.id)
            .collect();

        let token_set: std::collections::HashSet<u32> = tokens_to_remove.iter().copied().collect();

        // Remove tokens from token_by_name lookup
        for token_id in &tokens_to_remove {
            if let Some(token) = self.tokens.get(*token_id as usize) {
                if let Some(name) = self.strings.get(token.name_offset) {
                    let compact_name = CompactString::new(name);
                    if let Some(ids) = self.token_by_name.get_mut(&compact_name) {
                        ids.retain(|&id| id != *token_id);
                        if ids.is_empty() {
                            self.token_by_name.remove(&compact_name);
                        }
                    }
                }
            }
        }

        // Remove references involving removed tokens or symbols
        self.references
            .retain(|r| !token_set.contains(&r.token_id) && !symbol_set.contains(&r.symbol_id));

        // Update refs_to_symbol to remove references to removed tokens
        for refs in &mut self.refs_to_symbol {
            refs.retain(|&tid| !token_set.contains(&tid));
        }

        // Remove scopes for this file
        // Note: We keep scope slots to preserve IDs, just mark them as invalid
        for scope in &mut self.scopes {
            if scope.file_id == file_id {
                // Reset to an empty/invalid scope
                scope.kind = super::types::ScopeKind::Unknown as u8;
                scope.start_line = 0;
                scope.end_line = 0;
            }
        }

        // Note: We don't remove the file from self.files to preserve file_id mappings
        // The file slot remains but with potentially no associated data

        removed_count
    }

    /// Get the next available symbol ID (for incremental additions)
    pub fn next_symbol_id(&self) -> u32 {
        self.symbols.len() as u32
    }

    /// Get the next available token ID (for incremental additions)
    pub fn next_token_id(&self) -> u32 {
        self.tokens.len() as u32
    }

    /// Get the next available scope ID (for incremental additions)
    pub fn next_scope_id(&self) -> u32 {
        self.scopes.len() as u32
    }

    /// Check if a file path is already indexed
    pub fn has_file(&self, path: &std::path::Path) -> bool {
        self.file_id_for_path(path).is_some()
    }
}

impl Default for SemanticIndex {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// INDEX STATISTICS
// =============================================================================

/// Statistics about the semantic index
#[derive(Debug, Clone, Copy)]
pub struct IndexStats {
    pub symbols: usize,
    pub tokens: usize,
    pub references: usize,
    pub scopes: usize,
    pub edges: usize,
    pub files: usize,
    pub entry_points: usize,
    pub string_bytes: usize,
    pub unique_names: usize,
}

impl std::fmt::Display for IndexStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Semantic Index Statistics:")?;
        writeln!(f, "  Symbols:      {:>8}", self.symbols)?;
        writeln!(f, "  Tokens:       {:>8}", self.tokens)?;
        writeln!(f, "  References:   {:>8}", self.references)?;
        writeln!(f, "  Scopes:       {:>8}", self.scopes)?;
        writeln!(f, "  Edges:        {:>8}", self.edges)?;
        writeln!(f, "  Files:        {:>8}", self.files)?;
        writeln!(f, "  Entry Points: {:>8}", self.entry_points)?;
        writeln!(f, "  String Bytes: {:>8}", self.string_bytes)?;
        writeln!(f, "  Unique Names: {:>8}", self.unique_names)?;
        Ok(())
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::types::{SymbolFlags, TokenKind};

    #[test]
    fn test_string_table_interning() {
        let mut table = StringTable::new();

        let offset1 = table.intern("foo");
        let offset2 = table.intern("bar");
        let offset3 = table.intern("foo"); // Duplicate

        assert_eq!(offset1, offset3); // Same string = same offset
        assert_ne!(offset1, offset2); // Different strings = different offsets

        assert_eq!(table.get(offset1), Some("foo"));
        assert_eq!(table.get(offset2), Some("bar"));
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn test_semantic_index_basic() {
        let mut index = SemanticIndex::new();

        // Add a file
        let file_id = index.add_file(PathBuf::from("test.ts"));
        assert_eq!(file_id, 0);

        // Add a symbol
        let name_offset = index.strings.intern("myFunction");
        let symbol = Symbol::new(
            0,
            name_offset,
            file_id,
            SymbolKind::Function,
            SymbolFlags::IS_ENTRY_POINT | SymbolFlags::IS_EXPORTED,
            10,
            20,
        );
        index.add_symbol(symbol, "myFunction");

        // Verify lookups
        assert_eq!(index.symbols.len(), 1);
        assert!(index.symbols_by_name("myFunction").is_some());
        assert_eq!(index.entry_points.len(), 1);
    }

    #[test]
    fn test_call_graph() {
        let mut index = SemanticIndex::new();
        let file_id = index.add_file(PathBuf::from("test.ts"));

        // Add two symbols
        let name1 = index.strings.intern("caller");
        let name2 = index.strings.intern("callee");

        let sym1 = Symbol::new(
            0,
            name1,
            file_id,
            SymbolKind::Function,
            SymbolFlags::empty(),
            1,
            10,
        );
        let sym2 = Symbol::new(
            1,
            name2,
            file_id,
            SymbolKind::Function,
            SymbolFlags::empty(),
            15,
            25,
        );

        index.add_symbol(sym1, "caller");
        index.add_symbol(sym2, "callee");

        // Add edge: caller -> callee
        index.add_edge(Edge::new(0, 1, 5));

        // Verify graph
        assert_eq!(index.callers(1), &[0]);
        assert_eq!(index.callees(0), &[1]);
    }

    #[test]
    fn test_references() {
        let mut index = SemanticIndex::new();
        let file_id = index.add_file(PathBuf::from("test.ts"));

        // Add a symbol
        let name = index.strings.intern("myVar");
        let symbol = Symbol::new(
            0,
            name,
            file_id,
            SymbolKind::Variable,
            SymbolFlags::empty(),
            1,
            1,
        );
        index.add_symbol(symbol, "myVar");

        // Add a token and reference
        let token = Token::new(0, name, file_id, 5, 10, TokenKind::Identifier, 0);
        index.add_token(token, "myVar");
        index.add_reference(Reference::new(0, 0, RefKind::Read));

        // Verify
        let refs: Vec<_> = index.references_to(0).collect();
        assert_eq!(refs.len(), 1);
        assert!(refs[0].is_read());
    }

    #[test]
    fn test_file_id_for_path() {
        let mut index = SemanticIndex::new();

        let file1_id = index.add_file(PathBuf::from("src/main.ts"));
        let file2_id = index.add_file(PathBuf::from("src/utils.ts"));

        assert_eq!(
            index.file_id_for_path(std::path::Path::new("src/main.ts")),
            Some(file1_id)
        );
        assert_eq!(
            index.file_id_for_path(std::path::Path::new("src/utils.ts")),
            Some(file2_id)
        );
        assert_eq!(
            index.file_id_for_path(std::path::Path::new("src/other.ts")),
            None
        );
    }

    #[test]
    fn test_remove_file_data() {
        let mut index = SemanticIndex::new();

        // Add two files with symbols
        let file1_id = index.add_file(PathBuf::from("file1.ts"));
        let file2_id = index.add_file(PathBuf::from("file2.ts"));

        let name1 = index.strings.intern("func1");
        let name2 = index.strings.intern("func2");

        let sym1 = Symbol::new(
            0,
            name1,
            file1_id,
            SymbolKind::Function,
            SymbolFlags::IS_ENTRY_POINT,
            1,
            10,
        );
        let sym2 = Symbol::new(
            1,
            name2,
            file2_id,
            SymbolKind::Function,
            SymbolFlags::empty(),
            1,
            10,
        );

        index.add_symbol(sym1, "func1");
        index.add_symbol(sym2, "func2");

        // Add an edge between them
        index.add_edge(Edge::new(0, 1, 5));

        // Verify initial state
        assert!(index.symbols_by_name("func1").is_some());
        assert!(index.symbols_by_name("func2").is_some());
        assert_eq!(index.entry_points.len(), 1);
        assert_eq!(index.edges.len(), 1);

        // Remove file1's data
        let removed = index.remove_file_data(file1_id);
        assert_eq!(removed, 1, "Should remove 1 symbol");

        // func1 should be gone from lookup
        assert!(
            index
                .symbols_by_name("func1")
                .map(|s| s.is_empty())
                .unwrap_or(true),
            "func1 should be removed from lookup"
        );

        // func2 should still be there
        assert!(
            index
                .symbols_by_name("func2")
                .map(|s| !s.is_empty())
                .unwrap_or(false),
            "func2 should still be in lookup"
        );

        // Entry points should be empty (func1 was the only entry point)
        assert!(
            index.entry_points.is_empty(),
            "Entry points should be empty"
        );

        // Edges involving func1 should be removed
        assert!(index.edges.is_empty(), "Edges should be removed");
    }

    #[test]
    fn test_next_id_methods() {
        let mut index = SemanticIndex::new();
        let file_id = index.add_file(PathBuf::from("test.ts"));

        // Initially all next IDs should be 0
        assert_eq!(index.next_symbol_id(), 0);
        assert_eq!(index.next_token_id(), 0);
        assert_eq!(index.next_scope_id(), 0);

        // Add a symbol
        let name = index.strings.intern("test");
        let symbol = Symbol::new(
            0,
            name,
            file_id,
            SymbolKind::Function,
            SymbolFlags::empty(),
            1,
            10,
        );
        index.add_symbol(symbol, "test");

        // Next symbol ID should be 1
        assert_eq!(index.next_symbol_id(), 1);

        // Add a token
        let token = Token::new(0, name, file_id, 1, 0, TokenKind::Identifier, 0);
        index.add_token(token, "test");

        // Next token ID should be 1
        assert_eq!(index.next_token_id(), 1);
    }

    #[test]
    fn test_has_file() {
        let mut index = SemanticIndex::new();

        assert!(!index.has_file(std::path::Path::new("test.ts")));

        index.add_file(PathBuf::from("test.ts"));

        assert!(index.has_file(std::path::Path::new("test.ts")));
        assert!(!index.has_file(std::path::Path::new("other.ts")));
    }
}
