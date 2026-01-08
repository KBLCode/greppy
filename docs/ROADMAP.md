# Greppy Roadmap

## Current Version: v0.1.0 (Released)

### Features Shipped
- ✅ Sub-millisecond BM25 search via Tantivy
- ✅ Background daemon architecture with Unix socket IPC
- ✅ Tree-sitter AST parsing (Rust, TypeScript, JavaScript, Python, Go, Java, C/C++)
- ✅ Semantic metadata extraction (symbols, signatures, doc comments)
- ✅ Multi-factor scoring (symbol boost, export bonus, test penalty)
- ✅ File watching with debounced re-indexing
- ✅ JSON output for AI tool integration
- ✅ Project auto-detection (git, package.json, Cargo.toml, etc.)

---

## Version 0.2.0: LLM-Powered Semantic Search (In Development)

### Goal
Enable truly semantic code search by using Claude Haiku to understand query intent and rewrite queries for better BM25 matching. This allows AI coding tools to find relevant code without stuffing context.

### Phase 2.1: OAuth Infrastructure

**New Commands:**
```bash
greppy auth login     # Initiate OAuth flow with Anthropic
greppy auth logout    # Clear stored credentials
greppy auth status    # Show authentication status
```

**Implementation:**
- [ ] OAuth 2.0 PKCE flow for Anthropic Claude
- [ ] Token storage in ~/.config/greppy/auth.json
- [ ] Automatic token refresh
- [ ] Secure credential handling (no tokens in logs/errors)

**Files to Create:**
```
src/auth/
├── mod.rs           # Module exports
├── oauth.rs         # OAuth PKCE flow implementation
├── storage.rs       # Secure token storage
└── client.rs        # Authenticated HTTP client
```

**Dependencies:**
```toml
oauth2 = "4.4"
reqwest = { version = "0.11", features = ["json", "rustls-tls"] }
keyring = "2.0"      # Optional: OS keychain integration
```

### Phase 2.2: LLM Query Processor

**How It Works:**
1. User searches: `greppy search --smart "how does authentication work"`
2. Greppy sends query to Claude Haiku with system prompt
3. Haiku returns structured response:
   ```json
   {
     "intent": "find_implementation",
     "entity_type": "function",
     "expanded_query": "authenticate login token session OAuth verify credentials",
     "filters": {
       "symbol_types": ["function", "method"],
       "exclude_tests": true
     }
   }
   ```
4. Greppy uses expanded query for BM25 search
5. Results filtered/boosted based on Haiku's analysis

**Files to Create:**
```
src/llm/
├── mod.rs           # Module exports
├── client.rs        # Claude API client (uses auth tokens)
├── query.rs         # Query enhancement logic
└── prompts.rs       # System prompts for query understanding
```

**System Prompt (Draft):**
```
You are a code search query optimizer. Given a natural language query about code, 
extract the intent and expand it into search terms that will match code effectively.

Respond with JSON only:
{
  "intent": "find_definition|find_usage|find_implementation|understand_flow|find_error_handling",
  "entity_type": "function|class|method|variable|type|module|null",
  "expanded_query": "space-separated search terms including synonyms",
  "filters": {
    "symbol_types": ["function", "class", ...] or null,
    "exclude_tests": boolean,
    "file_patterns": ["*.rs", "*.ts"] or null
  }
}
```

### Phase 2.3: Smart Search Mode

**CLI Changes:**
```bash
# Standard search (no LLM)
greppy search "authenticate"

# Smart search (uses Haiku if authenticated)
greppy search --smart "how does the login flow work"
greppy search -s "error handling in API routes"

# Force smart search (fail if not authenticated)
greppy search --smart --require-auth "find rate limiting"
```

**Fallback Behavior:**
- If --smart but not authenticated → use regular search, warn user
- If --smart --require-auth but not authenticated → error with auth instructions
- If Haiku API fails → fallback to regular search, log warning

### Phase 2.4: Configuration

**Config File:** ~/.config/greppy/config.toml
```toml
[llm]
enabled = true
model = "claude-3-haiku-20240307"  # Cheapest/fastest
timeout_ms = 2000                   # Max wait for LLM response
fallback_on_error = true            # Use regular search if LLM fails

[search]
default_smart = false               # Don't use --smart by default
max_results = 50
include_tests = false

[daemon]
auto_start = true
watch_enabled = true
debounce_ms = 500
```

---

## Version 0.3.0: Performance & Intelligence (Planned)

### Incremental Indexing
- [ ] Track file hashes to detect changes
- [ ] Only re-index modified files
- [ ] Background incremental updates

### Token-Aware Output
- [ ] --max-tokens flag for context-limited output
- [ ] Smart truncation that preserves semantic boundaries
- [ ] Token counting for popular tokenizers (cl100k, etc.)

### Reference Tracking
- [ ] Find all usages of a symbol
- [ ] Cross-file reference graph
- [ ] "Go to definition" support

### Streaming Results
- [ ] Stream results as they are found
- [ ] Progressive rendering for large result sets

---

## Version 0.4.0: Advanced Features (Future)

### Multi-Project Support
- [ ] Index multiple projects simultaneously
- [ ] Cross-project search
- [ ] Workspace configuration

### Embeddings (Optional)
- [ ] Local embedding model support (ONNX)
- [ ] Hybrid BM25 + vector search
- [ ] Semantic similarity ranking

### IDE Integration
- [ ] VS Code extension
- [ ] Neovim plugin
- [ ] JetBrains plugin

---

## Non-Goals (Out of Scope)

- **Cloud-hosted index**: Greppy is local-first, always
- **Replacing LSP**: We complement, not replace language servers
- **Full AST analysis**: We extract metadata, not build compilers
- **Paid features**: Core functionality remains free and open source

---

## Contributing

Priority areas for contribution:
1. Additional language parsers (Ruby, PHP, Swift, Kotlin)
2. Performance optimizations
3. Documentation and examples
4. Testing on different platforms
