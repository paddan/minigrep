# mg

A performance-focused grep clone in Rust, benchmarked against ripgrep.

Build the release binary with `cargo build --release`. Run it as `./target/release/mg <query> <file> [-i] [-r]`. Re-run the benchmarks with `./benchmark.sh` (requires `rg` on PATH).

## Benchmark hardware

| | |
|---|---|
| Machine | MacBook Pro (16", 2021) — `MacBookPro18,4` |
| CPU | Apple M1 Max (10 cores) |
| Memory | 32 GB |
| OS | macOS 26.5 (build 25F71), Darwin 25.5.0 arm64 |
| ripgrep | 15.1.0 |
| Compiler | release profile (`cargo build --release`) |

Each query is run 10 times with 3 warmup runs; the table shows the mean wall time. Differences below ±2% are reported as `≈ tie` (within measurement noise). The standard file is the [1BRC](https://github.com/gunnarmorling/1brc) weather-stations CSV; the large file is that same file concatenated 1000× via `mktemp`. ripgrep is run with `-j <core count>` for fairness, though it makes no measurable difference — rg only parallelizes across files in a directory walk, not within a single file.

## Results: Standard File (805K)

| Test                                    | mg         | ripgrep    | Winner                 |
|------------------------------------------|------------|------------|------------------------|
| Exact: `San`                             |    26.56ms |    27.03ms | ≈ tie                  |
| Exact: `Tokyo`                           |    25.92ms |    27.04ms | **mg** 1.04x           |
| Exact (rare): `Reykjavik`                |    25.39ms |    29.16ms | **mg** 1.14x           |
| Exact (no match): `ZZZZZ`                |    25.30ms |    28.27ms | **mg** 1.11x           |
| Case-insensitive: `san`                  |    26.10ms |    27.25ms | **mg** 1.04x           |
| Case-insensitive: `tokyo`                |    27.52ms |    28.40ms | **mg** 1.03x           |
| Regex: `^[A-Z][a-z]+;`                   |    28.60ms |    31.73ms | **mg** 1.10x           |
| Regex: `;-?[0-9]+\.`                     |    28.04ms |    31.12ms | **mg** 1.10x           |
| Regex (alternation): `^(San\|New\|Los) ` |    27.47ms |    29.66ms | **mg** 1.07x           |
| Regex (hyphenated names)                 |    29.19ms |    30.08ms | **mg** 1.03x           |
| Regex (bounded quantifier + anchor)      |    28.41ms |    30.89ms | **mg** 1.08x           |
| Regex (case-insensitive): `^[a-z]`       |    27.35ms |    32.75ms | **mg** 1.19x           |

On the standard file most of the ~26 ms baseline is process startup (open, mmap, rayon thread-pool init), so the spread is small. The large-file results below isolate actual search throughput.

## Results: Large File (786M, 44,693,000 lines)

| Test                                    | mg         | ripgrep    | Winner                 |
|------------------------------------------|------------|------------|------------------------|
| Exact: `San`                             |    61.23ms |   204.86ms | **mg** 3.34x           |
| Exact: `Tokyo`                           |    53.86ms |   124.04ms | **mg** 2.30x           |
| Exact (rare): `Reykjavik`                |    53.03ms |   122.75ms | **mg** 2.31x           |
| Exact (no match): `ZZZZZ`                |    55.84ms |   130.37ms | **mg** 2.33x           |
| Case-insensitive: `san`                  |   111.93ms |   323.54ms | **mg** 2.89x           |
| Case-insensitive: `tokyo`                |    78.02ms |   192.00ms | **mg** 2.46x           |
| Regex: `^[A-Z][a-z]+;`                   |   501.72ms |  3261.34ms | **mg** 6.50x           |
| Regex: `;-?[0-9]+\.`                     |   429.00ms |  3207.90ms | **mg** 7.47x           |
| Regex (alternation): `^(San\|New\|Los) ` |    71.01ms |   272.54ms | **mg** 3.83x           |
| Regex (hyphenated names)                 |   292.97ms |  2266.93ms | **mg** 7.73x           |
| Regex (bounded quantifier + anchor)      |   493.07ms |  3481.61ms | **mg** 7.06x           |
| Regex (case-insensitive): `^[a-z]`       |    29.69ms |  2494.76ms | **mg** 84.02x          |
