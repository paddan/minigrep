#!/bin/bash

# Benchmark comparison: mg vs ripgrep
# Usage: ./benchmark.sh [test_file]
# Output: Markdown formatted results

set -e

# Configuration
RUNS=10
WARMUP_RUNS=3
TEST_FILE="${1:-weather_stations.csv}"

# ripgrep defaults to -j0 (heuristic thread count), which for a single-file
# search stays at 1 thread regardless of core count — rg only parallelizes
# across files in a directory walk, not within one file. We pass -j explicitly
# so the comparison isn't accidentally handicapping rg on a technicality, even
# though it measurably makes no difference for single-file input.
THREADS=$(sysctl -n hw.ncpu 2>/dev/null || nproc 2>/dev/null || echo 4)

# Check dependencies
check_deps() {
    if ! command -v rg &> /dev/null; then
        echo "Error: ripgrep (rg) is not installed" >&2
        exit 1
    fi

    if [[ ! -f "target/release/mg" ]]; then
        echo "Building mg in release mode..." >&2
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
        local start=$(python3 -c 'import time; print(f"{time.time():.6f}")')
        eval "$cmd" > /dev/null 2>&1
        local end=$(python3 -c 'import time; print(f"{time.time():.6f}")')
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

# Below this percentage we treat the result as within measurement noise.
TIE_THRESHOLD_PCT=2.0

# Holds the path to the buffered markdown report. Script-level so the EXIT
# trap (registered in main) can still see it after main has returned.
REPORT_FILE=""

# Format result and return winner text.
# Args: label1 time1 label2 time2 (exactly two engines).
# Picks the fastest engine and reports its margin over the other; if that
# margin is within the tie band it's reported as a tie.
get_winner() {
    local label1="$1" time1="$2" label2="$3" time2="$4"

    local first second winner_label
    if (( $(echo "$time1 <= $time2" | bc -l) )); then
        first="$time1"; second="$time2"; winner_label="$label1"
    else
        first="$time2"; second="$time1"; winner_label="$label2"
    fi

    # Margin of the winner over the other, as a percentage of the other —
    # used only to decide the tie band, since percentage compresses toward
    # 100% and hides the true gap on large speedups.
    local pct=$(echo "scale=3; (($second - $first) / $second) * 100" | bc)

    if (( $(echo "$pct < $TIE_THRESHOLD_PCT" | bc -l) )); then
        echo "≈ tie"
    else
        # Reported as a multiplier (times faster), which scales sensibly
        # whether the gap is 1.15x or 200x.
        local ratio=$(echo "scale=2; $second / $first" | bc)
        echo "**${winner_label}** ${ratio}x"
    fi
}

# Verify results match
verify_results() {
    local query="$1"
    local mg_args="$2"
    local rg_args="$3"

    local mg_count=$(./target/release/mg $mg_args "$query" "$TEST_FILE" 2>/dev/null | wc -l | tr -d ' ')
    local rg_count=$(rg -j "$THREADS" $rg_args "$query" "$TEST_FILE" 2>/dev/null | wc -l | tr -d ' ')

    if [[ "$mg_count" != "$rg_count" ]]; then
        echo "> ⚠️ Match count mismatch for '$query': mg=$mg_count, ripgrep=$rg_count" >&2
        return 1
    fi
    return 0
}

# Column widths used to pad the markdown table so it reads cleanly as raw text.
COL_TEST=38
COL_TIME=10
COL_WINNER=22

# Print the table header for a results section.
print_header() {
    printf "| %-*s | %-*s | %-*s | %-*s |\n" \
        "$COL_TEST" "Test" \
        "$COL_TIME" "mg" \
        "$COL_TIME" "ripgrep" \
        "$COL_WINNER" "Winner"
    printf "|%s|%s|%s|%s|\n" \
        "$(printf -- '-%.0s' $(seq 1 $((COL_TEST+2))))" \
        "$(printf -- '-%.0s' $(seq 1 $((COL_TIME+2))))" \
        "$(printf -- '-%.0s' $(seq 1 $((COL_TIME+2))))" \
        "$(printf -- '-%.0s' $(seq 1 $((COL_WINNER+2))))"
}

# Run a single benchmark test and output markdown row.
# Args: name minigrep_cmd ripgrep_cmd
run_test() {
    local name="$1"
    local minigrep_cmd="$2"
    local ripgrep_cmd="$3"

    echo "  Running: $name..." >&2

    local mg_time=$(benchmark "$minigrep_cmd")
    local rg_time=$(benchmark "$ripgrep_cmd")
    local mg_fmt="$(printf "%.2f" "$mg_time")ms"
    local rg_fmt="$(printf "%.2f" "$rg_time")ms"

    local winner=$(get_winner mg "$mg_time" ripgrep "$rg_time")

    printf "| %-*s | %*s | %*s | %-*s |\n" \
        "$COL_TEST" "$name" \
        "$COL_TIME" "$mg_fmt" \
        "$COL_TIME" "$rg_fmt" \
        "$COL_WINNER" "$winner"
}

# Run the full test suite (exact, case-insensitive, regex) against a given file.
# Same tests are used for both the standard and large files so results are comparable.
run_test_suite() {
    local file="$1"

    # Exact match tests
    run_test "Exact: \`San\`" \
        "./target/release/mg 'San' '$file'" \
        "rg -N -j "$THREADS" --color never 'San' '$file'"

    run_test "Exact: \`Tokyo\`" \
        "./target/release/mg 'Tokyo' '$file'" \
        "rg -N -j "$THREADS" --color never 'Tokyo' '$file'"

    run_test "Exact (rare): \`Reykjavik\`" \
        "./target/release/mg 'Reykjavik' '$file'" \
        "rg -N -j "$THREADS" --color never 'Reykjavik' '$file'"

    run_test "Exact (no match): \`ZZZZZ\`" \
        "./target/release/mg 'ZZZZZ' '$file'" \
        "rg -N -j "$THREADS" --color never 'ZZZZZ' '$file'"

    run_test "Case-insensitive: \`san\`" \
        "./target/release/mg -i 'san' '$file'" \
        "rg -Ni -j "$THREADS" --color never 'san' '$file'"

    run_test "Case-insensitive: \`tokyo\`" \
        "./target/release/mg -i 'tokyo' '$file'" \
        "rg -Ni -j "$THREADS" --color never 'tokyo' '$file'"

    run_test "Regex: \`^[A-Z][a-z]+;\`" \
        "./target/release/mg -r '^[A-Z][a-z]+;' '$file'" \
        "rg -N -j "$THREADS" --color never '^[A-Z][a-z]+;' '$file'"

    run_test "Regex: \`;-?[0-9]+\\.\`" \
        "./target/release/mg -r ';-?[0-9]+\\.' '$file'" \
        "rg -N -j "$THREADS" --color never ';-?[0-9]+\\.' '$file'"

    run_test "Regex (alternation): \`^(San\\|New\\|Los) \`" \
        "./target/release/mg -r '^(San|New|Los) ' '$file'" \
        "rg -N -j "$THREADS" --color never '^(San|New|Los) ' '$file'"

    run_test "Regex (hyphenated names)" \
        "./target/release/mg -r '[A-Z][a-z]+-[A-Z][a-z]+;' '$file'" \
        "rg -N -j "$THREADS" --color never '[A-Z][a-z]+-[A-Z][a-z]+;' '$file'"

    run_test "Regex (bounded quantifier + anchor)" \
        "./target/release/mg -r ';-?[0-9]{1,2}\\.[0-9]{4}\$' '$file'" \
        "rg -N -j "$THREADS" --color never ';-?[0-9]{1,2}\\.[0-9]{4}\$' '$file'"

    run_test "Regex (case-insensitive): \`^[a-z]\`" \
        "./target/release/mg -ri '^[a-z]' '$file'" \
        "rg -Ni -j "$THREADS" --color never '^[a-z]' '$file'"
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

    # Pretty-print line counts with thousands separators (LC_ALL=en_US for comma).
    local lines_fmt
    lines_fmt=$(LC_ALL=en_US.UTF-8 printf "%'d" "$line_count" 2>/dev/null || echo "$line_count")

    # Buffer all markdown output to a temp file so live "Running:" progress on
    # stderr doesn't interleave with the table on stdout. The EXIT trap dumps
    # whatever was written and cleans up — that way a partial report is still
    # printed if `set -e` aborts mid-benchmark (e.g. an rg invocation fails).
    REPORT_FILE=$(mktemp)
    trap 'if [[ -s "$REPORT_FILE" ]]; then cat "$REPORT_FILE"; fi; rm -f "$REPORT_FILE"' EXIT

    {
    cat << EOF
# mg vs ripgrep Benchmark

> Generated: $date

## Configuration

| Parameter | Value |
|-----------|-------|
| Test file | \`$TEST_FILE\` |
| File size | $file_size |
| Lines     | $lines_fmt |
| Runs      | $RUNS (+ $WARMUP_RUNS warmup) |
| ripgrep   | $rg_version |
| Threads   | $THREADS (rg -j; single-file search stays 1-threaded regardless) |
| Tie band  | ±${TIE_THRESHOLD_PCT}% (treated as noise) |

## Results: Standard File ($file_size)

EOF
    print_header
    run_test_suite "$TEST_FILE"

    # Large file test
    echo "" >&2
    echo "Creating large test file..." >&2
    local large_file=$(mktemp)
    for i in $(seq 1 1000); do cat "$TEST_FILE"; done > "$large_file"
    local large_size=$(ls -lh "$large_file" | awk '{print $5}')
    local large_lines=$(wc -l < "$large_file" | tr -d ' ')
    local large_lines_fmt
    large_lines_fmt=$(LC_ALL=en_US.UTF-8 printf "%'d" "$large_lines" 2>/dev/null || echo "$large_lines")

    cat << EOF

## Results: Large File ($large_size, $large_lines_fmt lines)

EOF
    print_header
    run_test_suite "$large_file"

    # Cleanup
    rm -f "$large_file"
    } > "$REPORT_FILE"

    echo "" >&2
    echo "Benchmark complete!" >&2
    echo "" >&2
    # Report is dumped by the EXIT trap above (works on success, set -e, SIGINT).
}

main "$@"
