#!/usr/bin/env bash
set -euo pipefail

# Greppy Performance Profiling Script
# Usage: ./scripts/profile.sh [flamegraph|dhat|hyperfine|all]

PROFILE_TYPE="${1:-all}"
PROJECT_DIR="${2:-.}"

echo "🔍 Greppy Performance Profiling"
echo "================================"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check dependencies
check_deps() {
    local missing=()
    
    if ! command -v cargo &> /dev/null; then
        missing+=("cargo")
    fi
    
    if [[ "$PROFILE_TYPE" == "flamegraph" || "$PROFILE_TYPE" == "all" ]]; then
        if ! command -v cargo-flamegraph &> /dev/null; then
            echo -e "${YELLOW}Installing flamegraph...${NC}"
            cargo install flamegraph
        fi
    fi
    
    if [[ "$PROFILE_TYPE" == "hyperfine" || "$PROFILE_TYPE" == "all" ]]; then
        if ! command -v hyperfine &> /dev/null; then
            echo -e "${YELLOW}Installing hyperfine...${NC}"
            if [[ "$OSTYPE" == "darwin"* ]]; then
                brew install hyperfine
            else
                cargo install hyperfine
            fi
        fi
    fi
    
    if [ ${#missing[@]} -ne 0 ]; then
        echo -e "${RED}Missing dependencies: ${missing[*]}${NC}"
        exit 1
    fi
}

# Run flamegraph profiling
run_flamegraph() {
    echo -e "\n${GREEN}📊 Running Flamegraph CPU Profiling${NC}"
    echo "-----------------------------------"
    
    # Build with debug symbols
    cargo build --release
    
    # Profile search operations
    echo "Profiling search operations..."
    cargo flamegraph --bin greppy -- search "authenticate" --project "$PROJECT_DIR"
    
    if [ -f flamegraph.svg ]; then
        echo -e "${GREEN}✓ Flamegraph generated: flamegraph.svg${NC}"
        
        # Open in browser (macOS)
        if [[ "$OSTYPE" == "darwin"* ]]; then
            open flamegraph.svg
        fi
    fi
}

# Run dhat memory profiling
run_dhat() {
    echo -e "\n${GREEN}💾 Running DHAT Memory Profiling${NC}"
    echo "--------------------------------"
    
    # Add dhat feature flag
    echo "Building with dhat..."
    cargo build --release --features dhat-heap
    
    echo "Running memory profile..."
    cargo run --release --features dhat-heap --bin greppy -- search "test" --project "$PROJECT_DIR"
    
    if [ -f dhat-heap.json ]; then
        echo -e "${GREEN}✓ Memory profile generated: dhat-heap.json${NC}"
        echo "View with: https://nnethercote.github.io/dh_view/dh_view.html"
    fi
}

# Run hyperfine benchmarks
run_hyperfine() {
    echo -e "\n${GREEN}⚡ Running Hyperfine Benchmarks${NC}"
    echo "------------------------------"
    
    # Build release binary
    cargo build --release
    
    # Ensure daemon is running
    ./target/release/greppy start || true
    sleep 2
    
    # Index test project
    echo "Indexing project..."
    ./target/release/greppy index --project "$PROJECT_DIR" || true
    
    # Benchmark simple search
    echo -e "\n${YELLOW}Simple search (1 term):${NC}"
    hyperfine --warmup 5 --runs 100 \
        './target/release/greppy search "auth" --project '"$PROJECT_DIR"
    
    # Benchmark complex search
    echo -e "\n${YELLOW}Complex search (3 terms):${NC}"
    hyperfine --warmup 5 --runs 100 \
        './target/release/greppy search "user database authenticate" --project '"$PROJECT_DIR"
    
    # Benchmark cached vs uncached
    echo -e "\n${YELLOW}Cache performance:${NC}"
    hyperfine --warmup 3 \
        --prepare './target/release/greppy stop && ./target/release/greppy start && sleep 1' \
        './target/release/greppy search "test" --project '"$PROJECT_DIR"
    
    # Export results
    echo -e "\n${YELLOW}Exporting detailed results...${NC}"
    hyperfine --warmup 5 --runs 100 \
        --export-json hyperfine-results.json \
        --export-markdown hyperfine-results.md \
        './target/release/greppy search "authenticate" --project '"$PROJECT_DIR"
    
    echo -e "${GREEN}✓ Results exported to hyperfine-results.{json,md}${NC}"
}

# Run criterion benchmarks
run_criterion() {
    echo -e "\n${GREEN}📈 Running Criterion Benchmarks${NC}"
    echo "-------------------------------"
    
    # Run all benchmarks
    cargo bench
    
    echo -e "${GREEN}✓ Benchmark results in target/criterion/${NC}"
    echo "View HTML reports: open target/criterion/report/index.html"
}

# Main execution
main() {
    check_deps
    
    case "$PROFILE_TYPE" in
        flamegraph)
            run_flamegraph
            ;;
        dhat)
            run_dhat
            ;;
        hyperfine)
            run_hyperfine
            ;;
        criterion)
            run_criterion
            ;;
        all)
            run_criterion
            run_hyperfine
            run_flamegraph
            echo -e "\n${GREEN}✅ All profiling complete!${NC}"
            echo "Results:"
            echo "  - Criterion: target/criterion/report/index.html"
            echo "  - Hyperfine: hyperfine-results.{json,md}"
            echo "  - Flamegraph: flamegraph.svg"
            ;;
        *)
            echo -e "${RED}Unknown profile type: $PROFILE_TYPE${NC}"
            echo "Usage: $0 [flamegraph|dhat|hyperfine|criterion|all]"
            exit 1
            ;;
    esac
}

main
