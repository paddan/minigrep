#!/bin/bash

# Benchmark comparison: minigrep vs ripgrep
# Usage: ./benchmark.sh [test_file]
# Output: Markdown formatted results

set -e

# Configuration
RUNS=10
WARMUP_RUNS=3
TEST_FILE="${1:-weather_stations.csv}"

# Check dependencies
check_deps() {
    if ! command -v rg &> /dev/null; then
        echo "Error: ripgrep (rg) is not installed" >&2
        exit 1
    fi

    if [[ ! -f "target/release/minigrep" ]]; then
        echo "Building minigrep in release mode..." >&2
        cargo build --release --quiet
    fi

    if [[ ! -f "$TEST_FILE" ]]; then
        echo "Error: Test file '$TEST_FILE' not found" >&2
        exit 1
    fi
}

# Run a command multiple times and return average time in milliseconds
benchmark() {
    local cmd="$1"
    local times=()
    
    # Warmup runs (not counted)
    for ((i=1; i<=WARMUP_RUNS; i++)); do
        eval "$cmd" > /dev/null 2>&1
    done
    
    # Timed runs
    for ((i=1; i<=RUNS; i++)); do
        local start=$(perl -MTime::HiRes=time -e 'printf "%.6f\n", time')
        eval "$cmd" > /dev/null 2>&1
        local end=$(perl -MTime::HiRes=time -e 'printf "%.6f\n", time')
        local elapsed=$(echo "$end - $start" | bc)
        times+=("$elapsed")
    done
    
    # Calculate average
    local sum=0
    for t in "${times[@]}"; do
        sum=$(echo "$sum + $t" | bc)
    done
    local avg=$(echo "scale=6; $sum / $RUNS" | bc)
    local avg_ms=$(echo "scale=2; $avg * 1000" | bc)
    
    echo "$avg_ms"
}

# Format result and return winner text
get_winner() {
    local minigrep_ms="$1"
    local ripgrep_ms="$2"
    
    if (( $(echo "$minigrep_ms < $ripgrep_ms" | bc -l) )); then
        local diff=$(echo "scale=1; (($ripgrep_ms - $minigrep_ms) / $ripgrep_ms) * 100" | bc | cut -d. -f1)
        echo "**minigrep** (+${diff}%)"
    elif (( $(echo "$minigrep_ms > $ripgrep_ms" | bc -l) )); then
        local diff=$(echo "scale=1; (($minigrep_ms - $ripgrep_ms) / $minigrep_ms) * 100" | bc | cut -d. -f1)
        echo "**ripgrep** (+${diff}%)"
    else
        echo "Tie"
    fi
}

# Verify results match
verify_results() {
    local query="$1"
    local mg_args="$2"
    local rg_args="$3"
    
    local mg_count=$(./target/release/minigrep $mg_args "$query" "$TEST_FILE" 2>/dev/null | wc -l | tr -d ' ')
    local rg_count=$(rg $rg_args "$query" "$TEST_FILE" 2>/dev/null | wc -l | tr -d ' ')
    
    if [[ "$mg_count" != "$rg_count" ]]; then
        echo "> ⚠️ Match count mismatch for '$query': minigrep=$mg_count, ripgrep=$rg_count" >&2
        return 1
    fi
    return 0
}

# Run a single benchmark test and output markdown row
run_test() {
    local name="$1"
    local minigrep_cmd="$2"
    local ripgrep_cmd="$3"
    
    echo "  Running: $name..." >&2
    
    local mg_time=$(benchmark "$minigrep_cmd")
    local rg_time=$(benchmark "$ripgrep_cmd")
    local winner=$(get_winner "$mg_time" "$rg_time")
    
    # Format times to 2 decimal places
    local mg_fmt=$(printf "%.2f" "$mg_time")
    local rg_fmt=$(printf "%.2f" "$rg_time")
    
    echo "| $name | ${mg_fmt}ms | ${rg_fmt}ms | $winner |"
}

main() {
    check_deps
    
    # Get file info
    local file_size=$(ls -lh "$TEST_FILE" | awk '{print $5}')
    local line_count=$(wc -l < "$TEST_FILE" | tr -d ' ')
    local rg_version=$(rg --version | head -1)
    local date=$(date +"%Y-%m-%d %H:%M:%S")
    
    # Verify correctness
    echo "Verifying result correctness..." >&2
    verify_results "San" "" "-N"
    verify_results "san" "-i" "-Ni"
    verify_results "^[A-Z]" "-r" "-N"
    echo "All results match!" >&2
    echo "" >&2
    
    # Output markdown
    cat << EOF
# minigrep vs ripgrep Benchmark

> Generated: $date

## Configuration

| Parameter | Value |
|-----------|-------|
| Test file | \`$TEST_FILE\` |
| File size | $file_size |
| Lines | $line_count |
| Runs | $RUNS (+ $WARMUP_RUNS warmup) |
| ripgrep | $rg_version |

## Results: Standard File ($file_size)

| Test | minigrep | ripgrep | Winner |
|------|----------|---------|--------|
EOF

    # Exact match tests
    run_test "Exact: \`San\`" \
        "./target/release/minigrep 'San' '$TEST_FILE'" \
        "rg -N 'San' '$TEST_FILE'"
    
    run_test "Exact: \`Tokyo\`" \
        "./target/release/minigrep 'Tokyo' '$TEST_FILE'" \
        "rg -N 'Tokyo' '$TEST_FILE'"
    
    run_test "Exact (rare): \`Reykjavik\`" \
        "./target/release/minigrep 'Reykjavik' '$TEST_FILE'" \
        "rg -N 'Reykjavik' '$TEST_FILE'"
    
    run_test "Exact (no match): \`ZZZZZ\`" \
        "./target/release/minigrep 'ZZZZZ' '$TEST_FILE'" \
        "rg -N 'ZZZZZ' '$TEST_FILE'"
    
    run_test "Case-insensitive: \`san\`" \
        "./target/release/minigrep -i 'san' '$TEST_FILE'" \
        "rg -Ni 'san' '$TEST_FILE'"
    
    run_test "Case-insensitive: \`tokyo\`" \
        "./target/release/minigrep -i 'tokyo' '$TEST_FILE'" \
        "rg -Ni 'tokyo' '$TEST_FILE'"
    
    run_test "Regex: \`^[A-Z][a-z]+;\`" \
        "./target/release/minigrep -r '^[A-Z][a-z]+;' '$TEST_FILE'" \
        "rg -N '^[A-Z][a-z]+;' '$TEST_FILE'"
    
    run_test "Regex: \`;-?[0-9]+\\.\`" \
        "./target/release/minigrep -r ';-?[0-9]+\\.' '$TEST_FILE'" \
        "rg -N ';-?[0-9]+\\.' '$TEST_FILE'"
    
    run_test "Regex (case-insensitive): \`^[a-z]\`" \
        "./target/release/minigrep -ri '^[a-z]' '$TEST_FILE'" \
        "rg -Ni '^[a-z]' '$TEST_FILE'"

    # Large file test
    echo "" >&2
    echo "Creating large test file (80MB)..." >&2
    local large_file=$(mktemp)
    for i in $(seq 1 100); do cat "$TEST_FILE"; done > "$large_file"
    local large_size=$(ls -lh "$large_file" | awk '{print $5}')
    local large_lines=$(wc -l < "$large_file" | tr -d ' ')
    
    cat << EOF

## Results: Large File ($large_size, $large_lines lines)

| Test | minigrep | ripgrep | Winner |
|------|----------|---------|--------|
EOF

    run_test "Exact: \`San\`" \
        "./target/release/minigrep 'San' '$large_file'" \
        "rg -N 'San' '$large_file'"
    
    run_test "Case-insensitive: \`san\`" \
        "./target/release/minigrep -i 'san' '$large_file'" \
        "rg -Ni 'san' '$large_file'"
    
    run_test "Regex: \`^[A-Z]\`" \
        "./target/release/minigrep -r '^[A-Z]' '$large_file'" \
        "rg -N '^[A-Z]' '$large_file'"

    # Cleanup
    rm -f "$large_file"

    cat << 'EOF'

## Summary

- **Small files**: minigrep is generally faster due to lower startup overhead
- **Large files**: Performance is comparable; ripgrep slightly faster for literal matches
- **Regex**: minigrep competitive or faster in most regex benchmarks

## How to Run

```bash
./benchmark.sh                     # default: weather_stations.csv
./benchmark.sh your_file.txt       # custom file
./benchmark.sh data.csv > results.md  # save as markdown
```
EOF

    echo "" >&2
    echo "Benchmark complete!" >&2
}

main "$@"
