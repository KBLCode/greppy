//! Semantic Index Builder
//!
//! Builds a SemanticIndex from extracted file data.
//! This module bridges the extraction layer with the index layer.
//!
//! @module trace/builder

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use tracing::{debug, info};

use super::extract::{extract_file, ExtractedCall, ExtractedData, ExtractedSymbol};
use super::index::SemanticIndex;
use super::storage::{save_index, trace_index_path};
use super::types::{
    Edge, RefKind, Reference, Scope, ScopeKind, Symbol, SymbolFlags, SymbolKind, Token, TokenKind,
};
use crate::core::error::Result;

// =============================================================================
// BUILDER
// =============================================================================

/// Builder for constructing a SemanticIndex from source files
pub struct SemanticIndexBuilder {
    /// The index being built
    index: SemanticIndex,
    /// Map from (file_id, symbol_name) to symbol_id for call resolution
    symbol_lookup: HashMap<String, Vec<u32>>,
    /// Next symbol ID
    next_symbol_id: u32,
    /// Next token ID
    next_token_id: u32,
    /// Next scope ID
    next_scope_id: u32,
    /// Project root for relative paths
    project_root: PathBuf,
}

impl SemanticIndexBuilder {
    /// Create a new builder
    pub fn new(project_root: impl AsRef<Path>) -> Self {
        Self {
            index: SemanticIndex::new(),
            symbol_lookup: HashMap::new(),
            next_symbol_id: 0,
            next_token_id: 0,
            next_scope_id: 0,
            project_root: project_root.as_ref().to_path_buf(),
        }
    }

    /// Create with estimated capacity
    pub fn with_capacity(project_root: impl AsRef<Path>, estimated_files: usize) -> Self {
        // Estimate ~50 symbols, ~200 tokens, ~100 refs per file
        let symbols = estimated_files * 50;
        let tokens = estimated_files * 200;
        let refs = estimated_files * 100;
        let scopes = estimated_files * 20;
        let edges = estimated_files * 30;

        Self {
            index: SemanticIndex::with_capacity(
                symbols,
                tokens,
                refs,
                scopes,
                edges,
                estimated_files,
            ),
            symbol_lookup: HashMap::with_capacity(symbols),
            next_symbol_id: 0,
            next_token_id: 0,
            next_scope_id: 0,
            project_root: project_root.as_ref().to_path_buf(),
        }
    }

    /// Add a file to the index
    pub fn add_file(&mut self, path: &Path, content: &str) {
        // Get relative path for storage
        let rel_path = path
            .strip_prefix(&self.project_root)
            .unwrap_or(path)
            .to_path_buf();

        // Add file to index
        let file_id = self.index.add_file(rel_path.clone());

        // Extract data from file
        let data = extract_file(path, content, None);

        if data.is_empty() {
            return;
        }

        debug!(
            file = %rel_path.display(),
            symbols = data.symbols.len(),
            calls = data.calls.len(),
            tokens = data.tokens.len(),
            method = data.extraction_method.as_str(),
            "Extracted"
        );

        // Add symbols with file path context for entry point detection
        for sym in &data.symbols {
            self.add_symbol_with_path(file_id, sym, Some(&rel_path));
        }

        // Add tokens
        for tok in &data.tokens {
            self.add_token(file_id, tok);
        }

        // Add scopes
        for (idx, scope) in data.scopes.iter().enumerate() {
            self.add_scope(file_id, scope, idx);
        }

        // Store calls for later edge resolution
        // We need to resolve calls after all symbols are added
        for call in &data.calls {
            self.add_call_token(file_id, call);
        }

        // Add construction references
        for ref_item in &data.references {
            self.add_construction_reference(file_id, ref_item);
        }
    }

    /// Add a construction reference to the index
    fn add_construction_reference(
        &mut self,
        file_id: u16,
        extracted: &super::extract::ExtractedRef,
    ) {
        // Only process construction references
        if extracted.kind != super::extract::RefKind::Construction {
            return;
        }

        let id = self.next_token_id;
        self.next_token_id += 1;

        let name_offset = self.index.strings.intern(&extracted.name);

        // Create a token for the construction site
        let token = Token::new(
            id,
            name_offset,
            file_id,
            extracted.line,
            extracted.column,
            TokenKind::Type,
            0,
        );

        self.index.add_token(token, &extracted.name);

        // Try to find the symbol being constructed and add a reference
        if let Some(target_ids) = self.symbol_lookup.get(&extracted.name) {
            for &target_id in target_ids {
                self.index
                    .add_reference(Reference::new(id, target_id, RefKind::Construction));
            }
        }
    }

    /// Add a symbol with file path context for entry point detection
    fn add_symbol_with_path(
        &mut self,
        file_id: u16,
        extracted: &ExtractedSymbol,
        file_path: Option<&Path>,
    ) {
        let id = self.next_symbol_id;
        self.next_symbol_id += 1;

        // Intern the name
        let name_offset = self.index.strings.intern(&extracted.name);

        // Convert kind
        let kind = match extracted.kind {
            super::extract::SymbolKind::Function => SymbolKind::Function,
            super::extract::SymbolKind::Method => SymbolKind::Method,
            super::extract::SymbolKind::Class => SymbolKind::Class,
            super::extract::SymbolKind::Struct => SymbolKind::Struct,
            super::extract::SymbolKind::Enum => SymbolKind::Enum,
            super::extract::SymbolKind::Interface => SymbolKind::Interface,
            super::extract::SymbolKind::TypeAlias => SymbolKind::TypeAlias,
            super::extract::SymbolKind::Constant => SymbolKind::Constant,
            super::extract::SymbolKind::Variable => SymbolKind::Variable,
            super::extract::SymbolKind::Module => SymbolKind::Module,
            // Trait maps to Interface (closest semantic match)
            super::extract::SymbolKind::Trait => SymbolKind::Interface,
            // Impl blocks are not tracked as standalone symbols
            super::extract::SymbolKind::Impl => SymbolKind::Unknown,
        };

        // Build flags
        let mut flags = SymbolFlags::empty();
        if extracted.is_exported {
            flags |= SymbolFlags::IS_EXPORTED;
        }
        if extracted.is_async {
            flags |= SymbolFlags::IS_ASYNC;
        }

        // Detect entry points using multiple heuristics
        let is_entry_point =
            self.detect_entry_point(&extracted.name, kind, extracted.is_exported, file_path);
        if is_entry_point {
            flags |= SymbolFlags::IS_ENTRY_POINT;
        }

        let symbol = Symbol::new(
            id,
            name_offset,
            file_id,
            kind,
            flags,
            extracted.start_line,
            extracted.end_line,
        );

        self.index.add_symbol(symbol, &extracted.name);

        // Add to lookup for call resolution
        self.symbol_lookup
            .entry(extracted.name.clone())
            .or_default()
            .push(id);
    }

    /// Detect if a symbol is an entry point based on various heuristics
    fn detect_entry_point(
        &self,
        name: &str,
        kind: SymbolKind,
        is_exported: bool,
        file_path: Option<&Path>,
    ) -> bool {
        // Only functions and methods can be entry points
        if !matches!(kind, SymbolKind::Function | SymbolKind::Method) {
            return false;
        }

        // main() is always an entry point
        if name == "main" {
            return true;
        }

        // Exported functions/methods are entry points
        if is_exported {
            return true;
        }

        // Check file path patterns for entry points
        if let Some(path) = file_path {
            let path_str = path.to_string_lossy().to_lowercase();
            let file_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();

            // Rust: public items in lib.rs are entry points
            if file_name == "lib.rs" {
                return true;
            }

            // Test files: test functions are entry points
            if path_str.contains("test")
                || path_str.contains("spec")
                || file_name.starts_with("test_")
                || file_name.ends_with("_test.rs")
            {
                return true;
            }

            // Benchmark files
            if path_str.contains("bench") {
                return true;
            }

            // TypeScript/JavaScript: index files and handlers
            if file_name == "index.ts"
                || file_name == "index.js"
                || file_name == "index.tsx"
                || file_name == "index.jsx"
            {
                return true;
            }

            // Check for common handler patterns
            if matches!(
                name,
                "handler" | "default" | "GET" | "POST" | "PUT" | "DELETE" | "PATCH"
            ) {
                return true;
            }
        }

        // Python: test_ prefixed functions are entry points
        if name.starts_with("test_") {
            return true;
        }

        false
    }

    /// Add a token to the index
    fn add_token(&mut self, file_id: u16, extracted: &super::extract::ExtractedToken) {
        let id = self.next_token_id;
        self.next_token_id += 1;

        let name_offset = self.index.strings.intern(&extracted.name);

        let kind = match extracted.kind {
            super::extract::TokenKind::Identifier => TokenKind::Identifier,
            // Keywords, Operators, Literals, Comments map to Unknown (no semantic index equivalent)
            super::extract::TokenKind::Keyword => TokenKind::Unknown,
            super::extract::TokenKind::Operator => TokenKind::Unknown,
            super::extract::TokenKind::Literal => TokenKind::Unknown,
            super::extract::TokenKind::Comment => TokenKind::Unknown,
            super::extract::TokenKind::Unknown => TokenKind::Unknown,
        };

        let token = Token::new(
            id,
            name_offset,
            file_id,
            extracted.line,
            extracted.column,
            kind,
            0, // scope_id - would need scope resolution
        );

        self.index.add_token(token, &extracted.name);
    }

    /// Add a call as a token (for later edge resolution)
    fn add_call_token(&mut self, file_id: u16, call: &ExtractedCall) {
        let id = self.next_token_id;
        self.next_token_id += 1;

        let name_offset = self.index.strings.intern(&call.callee_name);

        let token = Token::new(
            id,
            name_offset,
            file_id,
            call.line,
            call.column,
            TokenKind::Call,
            0,
        );

        self.index.add_token(token, &call.callee_name);
    }

    /// Add a scope to the index
    fn add_scope(&mut self, file_id: u16, extracted: &super::extract::ExtractedScope, _idx: usize) {
        let id = self.next_scope_id;
        self.next_scope_id += 1;

        let kind = match extracted.kind {
            super::extract::ScopeKind::File => ScopeKind::File,
            super::extract::ScopeKind::Module => ScopeKind::Module,
            super::extract::ScopeKind::Class => ScopeKind::Class,
            super::extract::ScopeKind::Function => ScopeKind::Function,
            super::extract::ScopeKind::Block => ScopeKind::Block,
            // Loop and Conditional map to Block (generic block scope)
            super::extract::ScopeKind::Loop => ScopeKind::Block,
            super::extract::ScopeKind::Conditional => ScopeKind::Block,
        };

        let parent_id = extracted.parent_index.map(|i| i as u32).unwrap_or(u32::MAX);
        let name_offset = extracted
            .name
            .as_ref()
            .map(|n| self.index.strings.intern(n))
            .unwrap_or(0);

        let scope = Scope::new(
            id,
            kind,
            file_id,
            parent_id,
            extracted.start_line,
            extracted.end_line,
            name_offset,
        );

        self.index.add_scope(scope);
    }

    /// Resolve call edges after all files are processed
    pub fn resolve_edges(&mut self) {
        // For each call token, try to find the target symbol
        let call_tokens: Vec<_> = self
            .index
            .tokens
            .iter()
            .filter(|t| t.token_kind() == TokenKind::Call)
            .cloned()
            .collect();

        for token in call_tokens {
            let callee_name = match self.index.strings.get(token.name_offset) {
                Some(name) => name.to_string(),
                None => continue,
            };

            // Find symbols with this name
            if let Some(target_ids) = self.symbol_lookup.get(&callee_name) {
                // Find the containing symbol for this call
                let caller_id = self.find_containing_symbol(token.file_id, token.line);

                if let Some(caller_id) = caller_id {
                    // Add edges to all matching symbols (could be overloaded)
                    for &target_id in target_ids {
                        // Don't add self-edges
                        if caller_id != target_id {
                            self.index
                                .add_edge(Edge::new(caller_id, target_id, token.line));
                        }
                    }

                    // Add reference
                    for &target_id in target_ids {
                        self.index.add_reference(Reference::new(
                            token.id,
                            target_id,
                            RefKind::Call,
                        ));
                    }
                }
            }
        }

        info!(
            edges = self.index.edges.len(),
            references = self.index.references.len(),
            "Resolved call edges"
        );
    }

    /// Find the symbol containing a given line in a file
    fn find_containing_symbol(&self, file_id: u16, line: u32) -> Option<u32> {
        // Find the smallest symbol that contains this line
        let mut best: Option<(u32, u32)> = None; // (symbol_id, size)

        for symbol in &self.index.symbols {
            if symbol.file_id == file_id && symbol.start_line <= line && symbol.end_line >= line {
                let size = symbol.end_line - symbol.start_line;
                match best {
                    None => best = Some((symbol.id, size)),
                    Some((_, best_size)) if size < best_size => best = Some((symbol.id, size)),
                    _ => {}
                }
            }
        }

        best.map(|(id, _)| id)
    }

    /// Build the final index
    pub fn build(mut self) -> SemanticIndex {
        self.resolve_edges();
        self.index
    }

    /// Get statistics about the build
    pub fn stats(&self) -> BuildStats {
        BuildStats {
            files: self.index.files.len(),
            symbols: self.index.symbols.len(),
            tokens: self.index.tokens.len(),
            scopes: self.index.scopes.len(),
            edges: self.index.edges.len(),
            references: self.index.references.len(),
        }
    }
}

/// Statistics about the build process
#[derive(Debug, Clone, Copy)]
pub struct BuildStats {
    pub files: usize,
    pub symbols: usize,
    pub tokens: usize,
    pub scopes: usize,
    pub edges: usize,
    pub references: usize,
}

impl std::fmt::Display for BuildStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} files, {} symbols, {} tokens, {} edges",
            self.files, self.symbols, self.tokens, self.edges
        )
    }
}

// =============================================================================
// PARALLEL BUILDER
// =============================================================================

/// Build a SemanticIndex from a list of files in parallel
pub fn build_index_parallel(project_root: &Path, files: &[(PathBuf, String)]) -> SemanticIndex {
    info!(files = files.len(), "Building semantic index");

    // Extract all files in parallel
    let extractions: Vec<(PathBuf, ExtractedData)> = files
        .par_iter()
        .map(|(path, content)| {
            let data = extract_file(path, content, None);
            (path.clone(), data)
        })
        .collect();

    // Build index sequentially (index is not thread-safe)
    let mut builder = SemanticIndexBuilder::with_capacity(project_root, files.len());

    for (path, data) in &extractions {
        if !data.is_empty() {
            // Re-add using the builder's method which handles all the details
            let rel_path = path
                .strip_prefix(project_root)
                .unwrap_or(path)
                .to_path_buf();
            let file_id = builder.index.add_file(rel_path.clone());

            // Add symbols with file path for entry point detection
            for sym in &data.symbols {
                builder.add_symbol_with_path(file_id, sym, Some(&rel_path));
            }

            // Add tokens
            for tok in &data.tokens {
                builder.add_token(file_id, tok);
            }

            // Add scopes
            for (idx, scope) in data.scopes.iter().enumerate() {
                builder.add_scope(file_id, scope, idx);
            }

            // Add calls
            for call in &data.calls {
                builder.add_call_token(file_id, call);
            }

            // Add construction references
            for ref_item in &data.references {
                builder.add_construction_reference(file_id, ref_item);
            }
        }
    }

    let stats = builder.stats();
    info!(%stats, "Extraction complete, resolving edges");

    builder.build()
}

/// Build and save a semantic index for a project
pub fn build_and_save_index(
    project_root: &Path,
    files: &[(PathBuf, String)],
) -> Result<BuildStats> {
    let index = build_index_parallel(project_root, files);
    let stats = index.stats();

    let path = trace_index_path(project_root);

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    save_index(&index, &path)?;

    info!(
        path = %path.display(),
        symbols = stats.symbols,
        tokens = stats.tokens,
        edges = stats.edges,
        "Saved semantic index"
    );

    Ok(BuildStats {
        files: stats.files,
        symbols: stats.symbols,
        tokens: stats.tokens,
        scopes: stats.scopes,
        edges: stats.edges,
        references: stats.references,
    })
}

// =============================================================================
// INCREMENTAL UPDATE
// =============================================================================

/// Result of an incremental file update
#[derive(Debug, Clone, Copy)]
pub struct IncrementalUpdateResult {
    /// Number of symbols removed (from old version)
    pub symbols_removed: usize,
    /// Number of symbols added (from new version)
    pub symbols_added: usize,
    /// Number of edges after update
    pub edges_count: usize,
    /// Time taken in milliseconds
    pub elapsed_ms: f64,
}

impl std::fmt::Display for IncrementalUpdateResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "removed {} symbols, added {} symbols, {} edges ({:.1}ms)",
            self.symbols_removed, self.symbols_added, self.edges_count, self.elapsed_ms
        )
    }
}

/// Update a single file in an existing SemanticIndex.
///
/// This performs an incremental update:
/// 1. Remove all data for the old version of the file (if it exists)
/// 2. Re-extract the file
/// 3. Add the new symbols, tokens, references, scopes
/// 4. Re-resolve edges for the new symbols
///
/// This is much faster than rebuilding the entire index (~20-50ms per file).
///
/// # Arguments
/// * `index` - The existing SemanticIndex to update
/// * `project_root` - Project root path for computing relative paths
/// * `path` - Path to the file to update
/// * `content` - New content of the file
///
/// # Returns
/// Statistics about the update operation
pub fn update_file_incremental(
    index: &mut SemanticIndex,
    project_root: &Path,
    path: &Path,
    content: &str,
) -> IncrementalUpdateResult {
    let start = std::time::Instant::now();

    // Get relative path
    let rel_path = path
        .strip_prefix(project_root)
        .unwrap_or(path)
        .to_path_buf();

    // Check if file already exists in index
    let file_id = if let Some(existing_id) = index.file_id_for_path(&rel_path) {
        // Remove old data
        let removed = index.remove_file_data(existing_id);
        debug!(
            file = %rel_path.display(),
            removed_symbols = removed,
            "Removed old file data"
        );
        existing_id
    } else {
        // New file - add it
        index.add_file(rel_path.clone())
    };

    // Extract data from file
    let data = extract_file(path, content, None);

    if data.is_empty() {
        return IncrementalUpdateResult {
            symbols_removed: 0,
            symbols_added: 0,
            edges_count: index.edges.len(),
            elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
        };
    }

    debug!(
        file = %rel_path.display(),
        symbols = data.symbols.len(),
        calls = data.calls.len(),
        tokens = data.tokens.len(),
        method = data.extraction_method.as_str(),
        "Extracted for incremental update"
    );

    // Track IDs for the new symbols
    let mut new_symbol_ids: Vec<u32> = Vec::with_capacity(data.symbols.len());
    let mut symbol_lookup: HashMap<String, Vec<u32>> = HashMap::new();

    // Add symbols with entry point detection
    for sym in &data.symbols {
        let id = index.next_symbol_id();
        let name_offset = index.strings.intern(&sym.name);

        let kind = match sym.kind {
            super::extract::SymbolKind::Function => SymbolKind::Function,
            super::extract::SymbolKind::Method => SymbolKind::Method,
            super::extract::SymbolKind::Class => SymbolKind::Class,
            super::extract::SymbolKind::Struct => SymbolKind::Struct,
            super::extract::SymbolKind::Enum => SymbolKind::Enum,
            super::extract::SymbolKind::Interface => SymbolKind::Interface,
            super::extract::SymbolKind::TypeAlias => SymbolKind::TypeAlias,
            super::extract::SymbolKind::Constant => SymbolKind::Constant,
            super::extract::SymbolKind::Variable => SymbolKind::Variable,
            super::extract::SymbolKind::Module => SymbolKind::Module,
            // Trait maps to Interface (closest semantic match)
            super::extract::SymbolKind::Trait => SymbolKind::Interface,
            // Impl blocks are not tracked as standalone symbols
            super::extract::SymbolKind::Impl => SymbolKind::Unknown,
        };

        let mut flags = SymbolFlags::empty();
        if sym.is_exported {
            flags |= SymbolFlags::IS_EXPORTED;
        }
        if sym.is_async {
            flags |= SymbolFlags::IS_ASYNC;
        }

        // Detect entry points using the same logic as the builder
        let is_entry_point =
            detect_entry_point_standalone(&sym.name, kind, sym.is_exported, Some(&rel_path));
        if is_entry_point {
            flags |= SymbolFlags::IS_ENTRY_POINT;
        }

        let symbol = Symbol::new(
            id,
            name_offset,
            file_id,
            kind,
            flags,
            sym.start_line,
            sym.end_line,
        );

        index.add_symbol(symbol, &sym.name);
        new_symbol_ids.push(id);
        symbol_lookup.entry(sym.name.clone()).or_default().push(id);
    }

    // Add tokens
    for tok in &data.tokens {
        let id = index.next_token_id();
        let name_offset = index.strings.intern(&tok.name);

        let kind = match tok.kind {
            super::extract::TokenKind::Identifier => TokenKind::Identifier,
            // Keywords, Operators, Literals, Comments map to Unknown (no semantic index equivalent)
            super::extract::TokenKind::Keyword => TokenKind::Unknown,
            super::extract::TokenKind::Operator => TokenKind::Unknown,
            super::extract::TokenKind::Literal => TokenKind::Unknown,
            super::extract::TokenKind::Comment => TokenKind::Unknown,
            super::extract::TokenKind::Unknown => TokenKind::Unknown,
        };

        let token = Token::new(id, name_offset, file_id, tok.line, tok.column, kind, 0);
        index.add_token(token, &tok.name);
    }

    // Add scopes
    for scope in data.scopes.iter() {
        let id = index.next_scope_id();
        let kind = match scope.kind {
            super::extract::ScopeKind::File => ScopeKind::File,
            super::extract::ScopeKind::Module => ScopeKind::Module,
            super::extract::ScopeKind::Class => ScopeKind::Class,
            super::extract::ScopeKind::Function => ScopeKind::Function,
            super::extract::ScopeKind::Block => ScopeKind::Block,
            // Loop and Conditional map to Block (generic block scope)
            super::extract::ScopeKind::Loop => ScopeKind::Block,
            super::extract::ScopeKind::Conditional => ScopeKind::Block,
        };

        let parent_id = scope.parent_index.map(|i| i as u32).unwrap_or(u32::MAX);
        let name_offset = scope
            .name
            .as_ref()
            .map(|n| index.strings.intern(n))
            .unwrap_or(0);

        let scope_obj = Scope::new(
            id,
            kind,
            file_id,
            parent_id,
            scope.start_line,
            scope.end_line,
            name_offset,
        );
        index.add_scope(scope_obj);
    }

    // Add call tokens and resolve edges
    for call in &data.calls {
        let token_id = index.next_token_id();
        let name_offset = index.strings.intern(&call.callee_name);

        let token = Token::new(
            token_id,
            name_offset,
            file_id,
            call.line,
            call.column,
            TokenKind::Call,
            0,
        );
        index.add_token(token, &call.callee_name);

        // Find the caller (containing symbol in this file)
        let caller_id = find_containing_symbol_in_file(index, file_id, call.line);

        // Find target symbols (could be in this file or other files)
        let target_ids: Vec<u32> = if let Some(ids) = symbol_lookup.get(&call.callee_name) {
            // Target is in the same file we're updating
            ids.clone()
        } else if let Some(ids) = index.symbols_by_name(&call.callee_name) {
            // Target is in another file
            ids.to_vec()
        } else {
            Vec::new()
        };

        if let Some(caller_id) = caller_id {
            for &target_id in &target_ids {
                if caller_id != target_id {
                    index.add_edge(Edge::new(caller_id, target_id, call.line));
                }
            }

            for &target_id in &target_ids {
                index.add_reference(Reference::new(token_id, target_id, RefKind::Call));
            }
        }
    }

    // Add construction references
    for ref_item in &data.references {
        if ref_item.kind != super::extract::RefKind::Construction {
            continue;
        }

        let token_id = index.next_token_id();
        let name_offset = index.strings.intern(&ref_item.name);

        let token = Token::new(
            token_id,
            name_offset,
            file_id,
            ref_item.line,
            ref_item.column,
            TokenKind::Type,
            0,
        );
        index.add_token(token, &ref_item.name);

        // Find target symbols
        let target_ids: Vec<u32> = if let Some(ids) = symbol_lookup.get(&ref_item.name) {
            ids.clone()
        } else if let Some(ids) = index.symbols_by_name(&ref_item.name) {
            ids.to_vec()
        } else {
            Vec::new()
        };

        for &target_id in &target_ids {
            index.add_reference(Reference::new(token_id, target_id, RefKind::Construction));
        }
    }

    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;

    debug!(
        file = %rel_path.display(),
        symbols_added = new_symbol_ids.len(),
        elapsed_ms = elapsed_ms,
        "Incremental update complete"
    );

    IncrementalUpdateResult {
        symbols_removed: 0, // TODO: track this properly
        symbols_added: new_symbol_ids.len(),
        edges_count: index.edges.len(),
        elapsed_ms,
    }
}

/// Remove a file from the index (e.g., when file is deleted)
///
/// # Arguments
/// * `index` - The SemanticIndex to update
/// * `project_root` - Project root path
/// * `path` - Path to the deleted file
///
/// # Returns
/// Number of symbols removed, or 0 if file wasn't in index
pub fn remove_file_from_index(
    index: &mut SemanticIndex,
    project_root: &Path,
    path: &Path,
) -> usize {
    let rel_path = path
        .strip_prefix(project_root)
        .unwrap_or(path)
        .to_path_buf();

    if let Some(file_id) = index.file_id_for_path(&rel_path) {
        let removed = index.remove_file_data(file_id);
        debug!(
            file = %rel_path.display(),
            removed_symbols = removed,
            "Removed file from index"
        );
        removed
    } else {
        debug!(file = %rel_path.display(), "File not in index, nothing to remove");
        0
    }
}

/// Standalone entry point detection for incremental updates
fn detect_entry_point_standalone(
    name: &str,
    kind: SymbolKind,
    is_exported: bool,
    file_path: Option<&Path>,
) -> bool {
    // Only functions and methods can be entry points
    if !matches!(kind, SymbolKind::Function | SymbolKind::Method) {
        return false;
    }

    // main() is always an entry point
    if name == "main" {
        return true;
    }

    // Exported functions/methods are entry points
    if is_exported {
        return true;
    }

    // Check file path patterns for entry points
    if let Some(path) = file_path {
        let path_str = path.to_string_lossy().to_lowercase();
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        // Rust: public items in lib.rs are entry points
        if file_name == "lib.rs" {
            return true;
        }

        // Test files: test functions are entry points
        if path_str.contains("test")
            || path_str.contains("spec")
            || file_name.starts_with("test_")
            || file_name.ends_with("_test.rs")
        {
            return true;
        }

        // Benchmark files
        if path_str.contains("bench") {
            return true;
        }

        // TypeScript/JavaScript: index files and handlers
        if file_name == "index.ts"
            || file_name == "index.js"
            || file_name == "index.tsx"
            || file_name == "index.jsx"
        {
            return true;
        }

        // Check for common handler patterns
        if matches!(
            name,
            "handler" | "default" | "GET" | "POST" | "PUT" | "DELETE" | "PATCH"
        ) {
            return true;
        }
    }

    // Python: test_ prefixed functions are entry points
    if name.starts_with("test_") {
        return true;
    }

    false
}

/// Find the symbol containing a given line in a specific file
fn find_containing_symbol_in_file(index: &SemanticIndex, file_id: u16, line: u32) -> Option<u32> {
    let mut best: Option<(u32, u32)> = None;

    for symbol in &index.symbols {
        if symbol.file_id == file_id && symbol.start_line <= line && symbol.end_line >= line {
            let size = symbol.end_line - symbol.start_line;
            match best {
                None => best = Some((symbol.id, size)),
                Some((_, best_size)) if size < best_size => best = Some((symbol.id, size)),
                _ => {}
            }
        }
    }

    best.map(|(id, _)| id)
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_builder_basic() {
        let dir = tempdir().unwrap();
        let mut builder = SemanticIndexBuilder::new(dir.path());

        let code = r#"
function greet(name: string): string {
    return `Hello, ${name}!`;
}

function main() {
    greet("World");
}
"#;

        let path = dir.path().join("test.ts");
        builder.add_file(&path, code);

        let index = builder.build();

        assert!(!index.symbols.is_empty(), "Should have symbols");
        assert!(
            index.symbols_by_name("greet").is_some(),
            "Should find greet"
        );
        assert!(index.symbols_by_name("main").is_some(), "Should find main");
    }

    #[test]
    fn test_builder_call_resolution() {
        let dir = tempdir().unwrap();
        let mut builder = SemanticIndexBuilder::new(dir.path());

        let code = r#"
function helper() {
    return 42;
}

function main() {
    const x = helper();
    return x;
}
"#;

        let path = dir.path().join("test.ts");
        builder.add_file(&path, code);

        let index = builder.build();

        // Check that we have edges
        let main_ids = index.symbols_by_name("main").unwrap();
        let helper_ids = index.symbols_by_name("helper").unwrap();

        assert!(!main_ids.is_empty());
        assert!(!helper_ids.is_empty());

        // main should call helper
        let callees = index.callees(main_ids[0]);
        assert!(callees.contains(&helper_ids[0]), "main should call helper");
    }

    #[test]
    fn test_incremental_update_add_file() {
        let dir = tempdir().unwrap();
        let project_root = dir.path();

        // Build initial index with one file
        let mut builder = SemanticIndexBuilder::new(project_root);
        let file1 = project_root.join("file1.ts");
        let code1 = "function foo() { return 1; }";
        builder.add_file(&file1, code1);
        let mut index = builder.build();

        let initial_symbols = index.symbols.len();
        assert!(index.symbols_by_name("foo").is_some());

        // Incrementally add a second file
        let file2 = project_root.join("file2.ts");
        let code2 = "function bar() { return 2; }";
        let result = update_file_incremental(&mut index, project_root, &file2, code2);

        assert!(result.symbols_added >= 1, "Should add at least one symbol");
        assert!(index.symbols_by_name("bar").is_some(), "Should find bar");
        assert!(
            index.symbols.len() > initial_symbols,
            "Should have more symbols"
        );
    }

    #[test]
    fn test_incremental_update_modify_file() {
        let dir = tempdir().unwrap();
        let project_root = dir.path();

        // Build initial index
        let mut builder = SemanticIndexBuilder::new(project_root);
        let file1 = project_root.join("file1.ts");
        let code1 = "function foo() { return 1; }";
        builder.add_file(&file1, code1);
        let mut index = builder.build();

        assert!(index.symbols_by_name("foo").is_some());
        assert!(index.symbols_by_name("baz").is_none());

        // Update the same file with different content
        let code2 = "function baz() { return 2; }";
        let result = update_file_incremental(&mut index, project_root, &file1, code2);

        assert!(result.symbols_added >= 1);
        assert!(index.symbols_by_name("baz").is_some(), "Should find baz");
        // Note: foo may still be in index since we don't remove stale symbols from Vec
        // but the lookup should only find baz for new queries
    }

    #[test]
    fn test_remove_file_from_index() {
        let dir = tempdir().unwrap();
        let project_root = dir.path();

        // Build initial index with two files
        let mut builder = SemanticIndexBuilder::new(project_root);
        let file1 = project_root.join("file1.ts");
        let file2 = project_root.join("file2.ts");
        builder.add_file(&file1, "function foo() {}");
        builder.add_file(&file2, "function bar() {}");
        let mut index = builder.build();

        assert!(index.symbols_by_name("foo").is_some());
        assert!(index.symbols_by_name("bar").is_some());

        // Remove file1
        let removed = remove_file_from_index(&mut index, project_root, &file1);
        assert!(removed >= 1, "Should remove at least one symbol");

        // foo should be gone from lookup
        assert!(
            index
                .symbols_by_name("foo")
                .map(|s| s.is_empty())
                .unwrap_or(true),
            "foo should be removed from lookup"
        );
        // bar should still be there
        assert!(
            index
                .symbols_by_name("bar")
                .map(|s| !s.is_empty())
                .unwrap_or(false),
            "bar should still be in index"
        );
    }

    #[test]
    fn test_incremental_update_performance() {
        let dir = tempdir().unwrap();
        let project_root = dir.path();

        // Build a small index
        let mut builder = SemanticIndexBuilder::with_capacity(project_root, 10);
        for i in 0..10 {
            let path = project_root.join(format!("file{}.ts", i));
            let code = format!("function func{}() {{ return {}; }}", i, i);
            builder.add_file(&path, &code);
        }
        let mut index = builder.build();

        // Update a single file and check it's fast
        let update_path = project_root.join("file0.ts");
        let new_code = "function updatedFunc() { return 42; }";
        let result = update_file_incremental(&mut index, project_root, &update_path, new_code);

        // Should complete in under 100ms (target is 20-50ms)
        assert!(
            result.elapsed_ms < 100.0,
            "Incremental update should be fast (<100ms), was {}ms",
            result.elapsed_ms
        );
    }
}
