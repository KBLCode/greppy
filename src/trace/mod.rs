//! Greppy Trace - Complete Codebase Invocation Mapping
//!
//! This module provides semantic code indexing and traversal capabilities:
//! - Symbol definitions (functions, classes, methods)
//! - Call graph edges
//! - Reference tracking (reads, writes, type annotations)
//! - Scope tree for context
//! - Token index for every identifier
//!
//! @module trace

pub mod builder;
pub mod context;
pub mod extract;
pub mod index;
pub mod output;
pub mod snapshots;
pub mod storage;
pub mod traverse;
pub mod types;

// =============================================================================
// RE-EXPORTS: Core Data Structures (types.rs)
// =============================================================================

pub use types::{
    Edge, RefKind, Reference, Scope, ScopeKind, Symbol, SymbolFlags, SymbolKind, Token, TokenKind,
    NO_PARENT_SCOPE,
};

// =============================================================================
// RE-EXPORTS: Index (index.rs)
// =============================================================================

pub use index::{IndexStats, SemanticIndex, StringTable};

// =============================================================================
// RE-EXPORTS: Storage (storage.rs)
// =============================================================================

pub use storage::{
    load_index, load_index_streaming, save_index, trace_index_exists, trace_index_path,
};

// =============================================================================
// RE-EXPORTS: Traversal (traverse.rs)
// =============================================================================

pub use traverse::{
    find_call_refs, find_dead_symbols, find_read_refs, find_refs, find_refs_of_kind,
    find_write_refs, format_call_chain, format_invocation_path, trace_symbol, trace_symbol_by_name,
    InvocationPath as TraverseInvocationPath, ReferenceContext, TraceResult as TraverseTraceResult,
};

// =============================================================================
// RE-EXPORTS: Extraction (extract/)
// =============================================================================

pub use extract::{
    detect_language, extract_file, is_treesitter_supported, ExtractedCall, ExtractedData,
    ExtractedRef, ExtractedScope, ExtractedSymbol, ExtractedToken, ExtractionMethod,
};

// =============================================================================
// RE-EXPORTS: Output (output/)
// =============================================================================

pub use output::{
    create_formatter, AsciiFormatter, ChainStep, DeadCodeResult, DeadSymbol, FlowAction,
    FlowResult, FlowStep, ImpactResult, InvocationPath, JsonFormatter, OutputFormat,
    PlainFormatter, ReferenceInfo, ReferenceKind, RefsResult, RiskLevel, TraceFormatter,
    TraceResult,
};

// =============================================================================
// RE-EXPORTS: Builder (builder.rs)
// =============================================================================

pub use builder::{build_and_save_index, build_index_parallel, BuildStats, SemanticIndexBuilder};

// =============================================================================
// RE-EXPORTS: Context (context.rs)
// =============================================================================

pub use context::{CacheStats, CodeContext, ContextBuilder, FileCache};

// =============================================================================
// RE-EXPORTS: Snapshots (snapshots.rs)
// =============================================================================

pub use snapshots::{
    cleanup_snapshots, compare_snapshots, create_snapshot, delete_snapshot, latest_snapshot,
    list_snapshots, load_snapshot, snapshots_dir, FileMetrics, Snapshot, SnapshotComparison,
    SnapshotDiff, SnapshotList, SnapshotMetrics, SnapshotSummary,
};
