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

Each query is run 10 times with 3 warmup runs; the table shows the mean wall time. Differences below ±2% are reported as `≈ tie` (within measurement noise). The standard file is the [1BRC](https://github.com/gunnarmorling/1brc) weather-stations CSV; the large file is that same file concatenated 1000× via `mktemp`.

## Results: Standard File (805K)

| Test                                   | mg         | ripgrep    | Winner                 |
|----------------------------------------|------------|------------|------------------------|
| Exact: `San`                           |    10.35ms |    12.23ms | **mg** +15.3%          |
| Exact: `Tokyo`                         |    11.71ms |    12.30ms | **mg** +4.8%           |
| Exact (rare): `Reykjavik`              |    11.00ms |    11.18ms | ≈ tie                  |
| Exact (no match): `ZZZZZ`              |    10.74ms |    11.09ms | **mg** +3.2%           |
| Case-insensitive: `san`                |    10.88ms |    11.60ms | **mg** +6.2%           |
| Case-insensitive: `tokyo`              |    10.61ms |    12.01ms | **mg** +11.6%          |
| Regex: `^[A-Z][a-z]+;`                 |    11.77ms |    14.88ms | **mg** +20.9%          |
| Regex: `;-?[0-9]+\.`                   |    11.85ms |    14.64ms | **mg** +19.0%          |
| Regex (case-insensitive): `^[a-z]`     |     9.93ms |    14.02ms | **mg** +29.1%          |

On the standard file most of the ~11 ms baseline is process startup (open, mmap, rayon thread-pool init), so the spread is small. The large-file results below isolate actual search throughput.

## Results: Large File (786M, 44,693,000 lines)

| Test                                   | mg         | ripgrep    | Winner                 |
|----------------------------------------|------------|------------|------------------------|
| Exact: `San`                           |    42.79ms |   182.39ms | **mg** +76.5%          |
| Case-insensitive: `san`                |    95.09ms |   300.61ms | **mg** +68.3%          |
| Regex: `^[A-Z]`                        |   362.12ms |  2427.30ms | **mg** +85.0%          |
