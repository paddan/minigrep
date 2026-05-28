# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

A performance-focused grep clone in Rust, benchmarked against ripgrep. Supports literal search, ASCII case-insensitive search, and regex.

## Commands

- Build (release, required for benchmarks): `cargo build --release`
- Run: `cargo run -- <query> <file> [-i] [-r]`
- All tests: `cargo test`
- Single test: `cargo test --test <name>` or `cargo test <test_fn_name>` (e.g. `cargo test regex_basic_search`)
- Lint: `cargo clippy`
- Benchmark vs ripgrep: `./benchmark.sh [test_file]` (requires `rg` on PATH; defaults to `weather_stations.csv`)

Note: tests in `src/grep.rs` shell out to `cargo run`, so they implicitly compile the binary and are slower than typical unit tests.

## Architecture

Two files: `src/main.rs` (argh-based CLI) and `src/grep.rs` (all search logic).

The search pipeline in `grep.rs::grep`:
1. Reject symlinks, then mmap the input file (`memmap2`) and hint `Advice::Sequential` for OS prefetch. Content is processed as `&[u8]`, never decoded to UTF-8.
2. Dispatch to one of three matchers based on flags:
   - regex (`-r`): `regex::bytes::RegexBuilder` with `multi_line(true)` and 10MB size/dfa limits. Multi-line is what lets `^`/`$` match per-line *inside* a chunk, so `find_iter` over a whole chunk gives the same results as per-line `is_match`.
   - case-insensitive literal (`-i`): `aho_corasick::AhoCorasick` with `ascii_case_insensitive`.
   - default literal: `memchr::memmem::Finder` (SIMD).
3. All three feed positions into the same `collect_lines` via `process_chunks`.

`process_chunks` strategy:
- `build_chunk_ranges` walks the mmap once with `memchr::memchr` to produce `(start, end)` ranges aligned on newline boundaries. Target ~3 chunks per rayon thread, clamped to `[MIN_CHUNK_SIZE, MAX_CHUNK_SIZE] = [64KB, 512KB]`. Files smaller than `2 × MIN_CHUNK_SIZE` become a single range (no parallelism).
- Each chunk is processed in parallel via `rayon::par_iter().map(...)` and produces its own owned `Vec<u8>` of matched lines (line bytes + `\n`, copied from the mmap).
- The outer collect is `Vec<Vec<u8>>` — one entry per chunk, in input order. `write_outputs` then streams these to a 64KB `BufWriter` on locked stdout. Write errors abort (broken pipe expected when piping to `head` etc.).

`collect_lines` (the core inner loop):
- Takes a matcher's `find_iter` positions (`Iterator<Item = usize>`). For each match position, expands to the surrounding line via `memchr::memrchr`/`memchr::memchr` and emits the line.
- A `next_line_start` cursor dedups: multiple match positions on the same line emit the line only once, and the cursor lets us SIMD-skip past already-emitted lines instead of scanning every line.
- Locked down by `dedup_multiple_matches_per_line_{literal,regex}` tests.

Constraints worth knowing before changing things:
- Case-insensitive search is ASCII-only by design (Aho-Corasick `ascii_case_insensitive`). Don't "fix" this to Unicode without a discussion — it's a perf tradeoff.
- Chunks own their output bytes (`Vec<Vec<u8>>`), so the mmap doesn't strictly have to outlive `write_outputs`. But `collect_lines` borrows from the mmap during parallel processing, so the mmap must outlive `process_chunks`.
- Chunk boundaries must land on newlines, otherwise lines get split across chunks.
- A query containing a literal `\n` will match across lines (memmem on the whole chunk) but `collect_lines` only emits the line containing the match *start*. argh delivers the query from argv so this is hard to hit in practice; don't rely on the behavior either way.

## Benchmarking

`benchmark.sh` runs each query 10 times (+3 warmup) against both binaries, verifies match counts agree, and emits a markdown table. It also synthesizes a ~800MB large file by concatenating `weather_stations.csv` 1000× to `mktemp`. Both `bc` and `perl` (Time::HiRes) are required.
