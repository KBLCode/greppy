#!/usr/bin/env bash
set -euo pipefail

# Quick performance validation script
# Usage: ./scripts/quick-bench.sh

echo "🚀 Greppy Quick Performance Check"
echo "=================================="

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# Check if greppy is built
if [ ! -f "target/release/greppy" ]; then
    echo -e "${YELLOW}Building release binary...${NC}"
    cargo build --release
fi

# Start daemon
echo -e "\n${GREEN}Starting daemon...${NC}"
./target/release/greppy start || true
sleep 2

# Index current project
echo -e "\n${GREEN}Indexing project...${NC}"
./target/release/greppy index --force

# Quick benchmarks
echo -e "\n${GREEN}Running quick benchmarks...${NC}"

echo -e "\n${YELLOW}1. Simple search (1 term):${NC}"
hyperfine --warmup 3 --runs 50 \
    './target/release/greppy search "search"'

echo -e "\n${YELLOW}2. Complex search (3 terms):${NC}"
hyperfine --warmup 3 --runs 50 \
    './target/release/greppy search "index search query"'

echo -e "\n${YELLOW}3. Cache performance:${NC}"
hyperfine --warmup 3 --runs 50 \
    './target/release/greppy search "cache"'

# Memory check
echo -e "\n${YELLOW}4. Memory usage:${NC}"
DAEMON_PID=$(pgrep -f "greppy.*daemon" || echo "")
if [ -n "$DAEMON_PID" ]; then
    RSS=$(ps -o rss= -p "$DAEMON_PID" | awk '{print $1/1024}')
    echo "Daemon RSS: ${RSS} MB"
    
    if (( $(echo "$RSS > 150" | bc -l) )); then
        echo -e "${RED}⚠️  Memory usage high (>${RSS}MB)${NC}"
    else
        echo -e "${GREEN}✓ Memory usage OK${NC}"
    fi
else
    echo -e "${RED}Daemon not running${NC}"
fi

# Throughput test
echo -e "\n${YELLOW}5. Throughput test (100 searches):${NC}"
START=$(date +%s.%N)
for i in {1..100}; do
    ./target/release/greppy search "test" > /dev/null 2>&1
done
END=$(date +%s.%N)
DURATION=$(echo "$END - $START" | bc)
THROUGHPUT=$(echo "100 / $DURATION" | bc -l)
printf "Throughput: %.0f searches/sec\n" "$THROUGHPUT"

if (( $(echo "$THROUGHPUT < 500" | bc -l) )); then
    echo -e "${RED}⚠️  Throughput low (<500/sec)${NC}"
else
    echo -e "${GREEN}✓ Throughput OK${NC}"
fi

echo -e "\n${GREEN}✅ Quick benchmark complete!${NC}"
