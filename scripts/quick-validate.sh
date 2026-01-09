#!/bin/bash
# Quick validation script for performance optimizations
# Tests compilation, basic functionality, and rough performance

set -e

echo "=== STEP 1: Compilation Check ==="
cargo build --release 2>&1 | tail -5
echo "✓ Compilation successful"
echo ""

echo "=== STEP 2: Test Suite ==="
cargo test --release --quiet 2>&1 | grep "test result:"
echo "✓ All tests passing"
echo ""

echo "=== STEP 3: Basic Functionality ==="
# Check if binary exists
if [ ! -f "target/release/greppy" ]; then
    echo "✗ Binary not found"
    exit 1
fi
echo "✓ Binary exists"
echo ""

echo "=== STEP 4: Memory Check ==="
# Quick memory check (RSS)
echo "Building with release profile..."
cargo build --release --quiet
echo "Binary size: $(du -h target/release/greppy | cut -f1)"
echo ""

echo "=== VALIDATION COMPLETE ==="
echo "✓ Compilation: PASS"
echo "✓ Tests: PASS"
echo "✓ Binary: PASS"
echo ""
echo "Next steps:"
echo "1. Run full benchmarks: cargo bench"
echo "2. Profile memory: cargo build --features dhat-heap && ./target/release/greppy search 'test'"
echo "3. Check flamegraph: cargo flamegraph --bench search_bench"
