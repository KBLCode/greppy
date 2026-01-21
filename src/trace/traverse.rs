//! Graph Traversal Algorithms
//!
//! Provides algorithms for navigating the semantic index:
//! - BFS backward traversal from target to entry points
//! - Reference finding
//! - Path reconstruction
//!
//! @module trace/traverse

use std::collections::{HashSet, VecDeque};

use super::index::SemanticIndex;
use super::types::{RefKind, Reference, Symbol};

// =============================================================================
// INVOCATION PATH
// =============================================================================

/// A single invocation path from an entry point to a target symbol
#[derive(Debug, Clone)]
pub struct InvocationPath {
    /// The entry point symbol (starting point of the call chain)
    pub entry_point: u32,
    /// Chain of symbol IDs from entry point to target (inclusive)
    /// First element is entry_point, last is target
    pub chain: Vec<u32>,
    /// Line numbers where each call occurs (len = chain.len() - 1)
    pub call_lines: Vec<u32>,
}

impl InvocationPath {
    /// Create a new invocation path
    pub fn new(entry_point: u32, chain: Vec<u32>, call_lines: Vec<u32>) -> Self {
        Self {
            entry_point,
            chain,
            call_lines,
        }
    }

    /// Get the target symbol (last in chain)
    pub fn target(&self) -> u32 {
        *self.chain.last().unwrap_or(&self.entry_point)
    }

    /// Get the depth of the call chain (0 = direct call from entry point)
    pub fn depth(&self) -> usize {
        self.chain.len().saturating_sub(1)
    }

    /// Check if this is a direct call (entry point directly calls target)
    pub fn is_direct(&self) -> bool {
        self.chain.len() <= 2
    }
}

// =============================================================================
// TRACE RESULT
// =============================================================================

/// Result of a trace operation
#[derive(Debug, Clone)]
pub struct TraceResult {
    /// Target symbol ID that was traced
    pub target: u32,
    /// All invocation paths to the target
    pub paths: Vec<InvocationPath>,
    /// Symbols that were visited during traversal
    pub visited_count: usize,
}

impl TraceResult {
    /// Check if any paths were found
    pub fn has_paths(&self) -> bool {
        !self.paths.is_empty()
    }

    /// Get the number of unique entry points
    pub fn entry_point_count(&self) -> usize {
        let entries: HashSet<_> = self.paths.iter().map(|p| p.entry_point).collect();
        entries.len()
    }

    /// Get the shortest path depth
    pub fn min_depth(&self) -> Option<usize> {
        self.paths.iter().map(|p| p.depth()).min()
    }

    /// Get the longest path depth
    pub fn max_depth(&self) -> Option<usize> {
        self.paths.iter().map(|p| p.depth()).max()
    }
}

// =============================================================================
// TRACE SYMBOL
// =============================================================================

/// Trace all invocation paths from entry points to a target symbol
///
/// Uses BFS backward traversal from the target to find all entry points
/// that can reach it, then reconstructs the paths.
///
/// # Arguments
/// * `index` - The semantic index to search
/// * `target_id` - The symbol ID to trace to
/// * `max_depth` - Maximum call chain depth (default: 50)
///
/// # Returns
/// A `TraceResult` containing all invocation paths
pub fn trace_symbol(
    index: &SemanticIndex,
    target_id: u32,
    max_depth: Option<usize>,
) -> TraceResult {
    let max_depth = max_depth.unwrap_or(50);

    // Validate target exists
    if index.symbol(target_id).is_none() {
        return TraceResult {
            target: target_id,
            paths: Vec::new(),
            visited_count: 0,
        };
    }

    // BFS backward traversal
    let mut visited: HashSet<u32> = HashSet::new();
    let mut queue: VecDeque<(u32, Vec<u32>, Vec<u32>)> = VecDeque::new();
    let mut paths: Vec<InvocationPath> = Vec::new();

    // Start from target
    queue.push_back((target_id, vec![target_id], Vec::new()));
    visited.insert(target_id);

    while let Some((current, chain, call_lines)) = queue.pop_front() {
        // Check depth limit
        if chain.len() > max_depth {
            continue;
        }

        // Get symbol info
        let symbol = match index.symbol(current) {
            Some(s) => s,
            None => continue,
        };

        // Check if this is an entry point
        if symbol.is_entry_point() {
            // Found a complete path - reverse it so entry point is first
            let mut path_chain = chain.clone();
            let mut path_lines = call_lines.clone();
            path_chain.reverse();
            path_lines.reverse();

            paths.push(InvocationPath::new(current, path_chain, path_lines));
        }

        // Find all callers (symbols that call this one)
        for &caller_id in index.callers(current) {
            if visited.contains(&caller_id) {
                // Skip already visited to avoid cycles
                // But we might want to still record the path if it reaches an entry point
                continue;
            }

            // Find the edge to get the call line
            let call_line = index
                .edges
                .iter()
                .find(|e| e.from_symbol == caller_id && e.to_symbol == current)
                .map(|e| e.line)
                .unwrap_or(0);

            // Extend the chain
            let mut new_chain = chain.clone();
            new_chain.push(caller_id);

            let mut new_lines = call_lines.clone();
            new_lines.push(call_line);

            visited.insert(caller_id);
            queue.push_back((caller_id, new_chain, new_lines));
        }
    }

    TraceResult {
        target: target_id,
        paths,
        visited_count: visited.len(),
    }
}

/// Trace a symbol by name
///
/// Finds all symbols matching the name and traces each one.
pub fn trace_symbol_by_name(
    index: &SemanticIndex,
    name: &str,
    max_depth: Option<usize>,
) -> Vec<TraceResult> {
    let symbol_ids = match index.symbols_by_name(name) {
        Some(ids) => ids.clone(),
        None => return Vec::new(),
    };

    symbol_ids
        .iter()
        .map(|&id| trace_symbol(index, id, max_depth))
        .collect()
}

// =============================================================================
// FIND REFERENCES
// =============================================================================

/// Reference with full context
#[derive(Debug, Clone)]
pub struct ReferenceContext {
    /// The reference
    pub reference: Reference,
    /// Token ID
    pub token_id: u32,
    /// File ID where the reference occurs
    pub file_id: u16,
    /// Line number
    pub line: u32,
    /// Column number
    pub column: u16,
    /// Scope ID
    pub scope_id: u32,
    /// Symbol name (if available)
    pub symbol_name: Option<String>,
}

/// Find all references to a symbol
pub fn find_refs(index: &SemanticIndex, symbol_id: u32) -> Vec<ReferenceContext> {
    let mut results = Vec::new();

    for reference in index.references_to(symbol_id) {
        if let Some(token) = index.token(reference.token_id) {
            let symbol_name = index
                .symbol(symbol_id)
                .and_then(|s| index.symbol_name(s))
                .map(|s| s.to_string());

            results.push(ReferenceContext {
                reference: *reference,
                token_id: reference.token_id,
                file_id: token.file_id,
                line: token.line,
                column: token.column,
                scope_id: token.scope_id,
                symbol_name,
            });
        }
    }

    results
}

/// Find references of a specific kind
pub fn find_refs_of_kind(
    index: &SemanticIndex,
    symbol_id: u32,
    kind: RefKind,
) -> Vec<ReferenceContext> {
    find_refs(index, symbol_id)
        .into_iter()
        .filter(|r| r.reference.ref_kind() == kind)
        .collect()
}

/// Find call references to a symbol
pub fn find_call_refs(index: &SemanticIndex, symbol_id: u32) -> Vec<ReferenceContext> {
    find_refs_of_kind(index, symbol_id, RefKind::Call)
}

/// Find read references to a symbol
pub fn find_read_refs(index: &SemanticIndex, symbol_id: u32) -> Vec<ReferenceContext> {
    find_refs_of_kind(index, symbol_id, RefKind::Read)
}

/// Find write references to a symbol
pub fn find_write_refs(index: &SemanticIndex, symbol_id: u32) -> Vec<ReferenceContext> {
    find_refs_of_kind(index, symbol_id, RefKind::Write)
}

// =============================================================================
// DEAD CODE DETECTION
// =============================================================================

/// Find potentially dead symbols (no incoming references or calls)
///
/// Returns symbols that:
/// - Are not entry points
/// - Have no incoming edges (no one calls them)
/// - Have no references
pub fn find_dead_symbols(index: &SemanticIndex) -> Vec<&Symbol> {
    index
        .symbols
        .iter()
        .filter(|s| {
            // Skip entry points - they're supposed to be "uncalled"
            if s.is_entry_point() {
                return false;
            }

            // Check for incoming edges
            let has_callers = !index.callers(s.id).is_empty();
            if has_callers {
                return false;
            }

            // Check for references
            let has_refs = index.references_to(s.id).next().is_some();
            !has_refs
        })
        .collect()
}

// =============================================================================
// CALL CHAIN HELPERS
// =============================================================================

/// Format a call chain as a string
pub fn format_call_chain(index: &SemanticIndex, chain: &[u32]) -> String {
    chain
        .iter()
        .filter_map(|&id| index.symbol(id).and_then(|s| index.symbol_name(s)))
        .collect::<Vec<_>>()
        .join(" -> ")
}

/// Format an invocation path with file locations
pub fn format_invocation_path(index: &SemanticIndex, path: &InvocationPath) -> String {
    let mut result = String::new();

    for (i, &symbol_id) in path.chain.iter().enumerate() {
        if let Some(symbol) = index.symbol(symbol_id) {
            let name = index.symbol_name(symbol).unwrap_or("<unknown>");
            let file = index
                .file_path(symbol.file_id)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "<unknown>".into());
            let start_line = symbol.start_line;

            if i == 0 {
                result.push_str(&format!("{} ({}:{})", name, file, start_line));
            } else {
                let call_line = path.call_lines.get(i - 1).copied().unwrap_or(0);
                result.push_str(&format!(
                    "\n  -> {} ({}:{}) [called at line {}]",
                    name, file, start_line, call_line
                ));
            }
        }
    }

    result
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::types::{Edge, SymbolFlags, SymbolKind};

    fn create_test_index() -> SemanticIndex {
        let mut index = SemanticIndex::new();

        // Add a file
        let file_id = index.add_file("test.rs".into());

        // Create symbols: main -> a -> b -> target
        let names: Vec<_> = ["main", "a", "b", "target"]
            .iter()
            .map(|n| index.strings.intern(n))
            .collect();

        // main is entry point
        index.add_symbol(
            Symbol::new(
                0,
                names[0],
                file_id,
                SymbolKind::Function,
                SymbolFlags::IS_ENTRY_POINT,
                1,
                10,
            ),
            "main",
        );

        // Other functions
        for (i, name) in ["a", "b", "target"].iter().enumerate() {
            index.add_symbol(
                Symbol::new(
                    (i + 1) as u32,
                    names[i + 1],
                    file_id,
                    SymbolKind::Function,
                    SymbolFlags::empty(),
                    ((i + 1) * 10 + 1) as u32,
                    ((i + 2) * 10) as u32,
                ),
                name,
            );
        }

        // Add call edges: main -> a -> b -> target
        index.add_edge(Edge::new(0, 1, 5)); // main calls a
        index.add_edge(Edge::new(1, 2, 15)); // a calls b
        index.add_edge(Edge::new(2, 3, 25)); // b calls target

        index
    }

    #[test]
    fn test_trace_symbol() {
        let index = create_test_index();

        // Trace to "target" (id=3)
        let result = trace_symbol(&index, 3, None);

        assert!(result.has_paths());
        assert_eq!(result.paths.len(), 1);

        let path = &result.paths[0];
        assert_eq!(path.entry_point, 0); // main
        assert_eq!(path.chain, vec![0, 1, 2, 3]); // main -> a -> b -> target
        assert_eq!(path.depth(), 3);
    }

    #[test]
    fn test_trace_direct_call() {
        let index = create_test_index();

        // Trace to "a" (id=1) - directly called by main
        let result = trace_symbol(&index, 1, None);

        assert!(result.has_paths());
        assert_eq!(result.paths.len(), 1);

        let path = &result.paths[0];
        assert_eq!(path.chain, vec![0, 1]); // main -> a
        assert!(path.is_direct());
    }

    #[test]
    fn test_trace_nonexistent() {
        let index = create_test_index();

        // Trace to nonexistent symbol
        let result = trace_symbol(&index, 999, None);

        assert!(!result.has_paths());
        assert_eq!(result.visited_count, 0);
    }

    #[test]
    fn test_format_call_chain() {
        let index = create_test_index();
        let chain = vec![0, 1, 2, 3];

        let formatted = format_call_chain(&index, &chain);
        assert_eq!(formatted, "main -> a -> b -> target");
    }

    #[test]
    fn test_find_dead_symbols() {
        let mut index = SemanticIndex::new();
        let file_id = index.add_file("test.rs".into());

        // Add an entry point
        let name1 = index.strings.intern("main");
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

        // Add a called function
        let name2 = index.strings.intern("used");
        index.add_symbol(
            Symbol::new(
                1,
                name2,
                file_id,
                SymbolKind::Function,
                SymbolFlags::empty(),
                15,
                25,
            ),
            "used",
        );

        // Add a dead function (never called)
        let name3 = index.strings.intern("dead");
        index.add_symbol(
            Symbol::new(
                2,
                name3,
                file_id,
                SymbolKind::Function,
                SymbolFlags::empty(),
                30,
                40,
            ),
            "dead",
        );

        // main calls used
        index.add_edge(Edge::new(0, 1, 5));

        let dead = find_dead_symbols(&index);
        assert_eq!(dead.len(), 1);
        assert_eq!(dead[0].id, 2);
    }
}
