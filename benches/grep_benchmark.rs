use aho_corasick::AhoCorasick;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use memmap2::MmapOptions;
use rayon::prelude::*;
use regex::bytes::RegexBuilder;
use std::fs::File;

const TEST_FILE: &str = "weather_stations.csv";

fn load_file() -> memmap2::Mmap {
    let file = File::open(TEST_FILE).expect("Failed to open test file");
    unsafe { MmapOptions::new().map(&file).expect("Failed to mmap file") }
}

// Aho-Corasick exact match (case-sensitive)
fn bench_aho_corasick(contents: &[u8], query: &str) -> usize {
    let ac = AhoCorasick::new([query]).unwrap();
    contents
        .par_split(|&b| b == b'\n')
        .filter(|line| ac.find(line).is_some())
        .count()
}

// Aho-Corasick case-insensitive
fn bench_aho_corasick_case_insensitive(contents: &[u8], query: &str) -> usize {
    let ac = AhoCorasick::builder()
        .ascii_case_insensitive(true)
        .build([query])
        .unwrap();
    contents
        .par_split(|&b| b == b'\n')
        .filter(|line| ac.find(line).is_some())
        .count()
}

// Regex match (case-sensitive)
fn bench_regex(contents: &[u8], pattern: &str) -> usize {
    let regex = RegexBuilder::new(pattern).build().unwrap();
    contents
        .par_split(|&b| b == b'\n')
        .filter(|line| regex.is_match(line))
        .count()
}

// Regex case-insensitive
fn bench_regex_case_insensitive(contents: &[u8], pattern: &str) -> usize {
    let regex = RegexBuilder::new(pattern)
        .case_insensitive(true)
        .build()
        .unwrap();
    contents
        .par_split(|&b| b == b'\n')
        .filter(|line| regex.is_match(line))
        .count()
}

// Sequential version for comparison
fn bench_aho_corasick_sequential(contents: &[u8], query: &str) -> usize {
    let ac = AhoCorasick::new([query]).unwrap();
    contents
        .split(|&b| b == b'\n')
        .filter(|line| ac.find(line).is_some())
        .count()
}

fn bench_regex_sequential(contents: &[u8], pattern: &str) -> usize {
    let regex = RegexBuilder::new(pattern).build().unwrap();
    contents
        .split(|&b| b == b'\n')
        .filter(|line| regex.is_match(line))
        .count()
}

fn criterion_benchmark(c: &mut Criterion) {
    let mmap = load_file();
    let contents: &[u8] = &mmap;
    let file_size = contents.len() as u64;

    // Common city name (many matches)
    let common_query = "San";
    // Rare query (few matches)
    let rare_query = "Reykjavik";
    // Complex regex pattern
    let regex_pattern = r"^[A-Z][a-z]+;-?\d+\.\d+";

    let mut group = c.benchmark_group("exact_match");
    group.throughput(Throughput::Bytes(file_size));

    group.bench_with_input(
        BenchmarkId::new("aho_corasick_parallel", common_query),
        &common_query,
        |b, query| {
            b.iter(|| bench_aho_corasick(black_box(contents), black_box(query)));
        },
    );

    group.bench_with_input(
        BenchmarkId::new("aho_corasick_sequential", common_query),
        &common_query,
        |b, query| {
            b.iter(|| bench_aho_corasick_sequential(black_box(contents), black_box(query)));
        },
    );

    group.bench_with_input(
        BenchmarkId::new("aho_corasick_parallel", rare_query),
        &rare_query,
        |b, query| {
            b.iter(|| bench_aho_corasick(black_box(contents), black_box(query)));
        },
    );

    group.bench_with_input(
        BenchmarkId::new("aho_corasick_sequential", rare_query),
        &rare_query,
        |b, query| {
            b.iter(|| bench_aho_corasick_sequential(black_box(contents), black_box(query)));
        },
    );

    group.finish();

    let mut group = c.benchmark_group("case_insensitive");
    group.throughput(Throughput::Bytes(file_size));

    group.bench_with_input(
        BenchmarkId::new("aho_corasick_case_insensitive", common_query),
        &common_query,
        |b, query| {
            b.iter(|| bench_aho_corasick_case_insensitive(black_box(contents), black_box(query)));
        },
    );

    group.bench_with_input(
        BenchmarkId::new("aho_corasick_case_sensitive", common_query),
        &common_query,
        |b, query| {
            b.iter(|| bench_aho_corasick(black_box(contents), black_box(query)));
        },
    );

    group.finish();

    let mut group = c.benchmark_group("regex");
    group.throughput(Throughput::Bytes(file_size));

    group.bench_with_input(
        BenchmarkId::new("regex_parallel", regex_pattern),
        &regex_pattern,
        |b, pattern| {
            b.iter(|| bench_regex(black_box(contents), black_box(pattern)));
        },
    );

    group.bench_with_input(
        BenchmarkId::new("regex_sequential", regex_pattern),
        &regex_pattern,
        |b, pattern| {
            b.iter(|| bench_regex_sequential(black_box(contents), black_box(pattern)));
        },
    );

    group.bench_with_input(
        BenchmarkId::new("regex_case_insensitive", common_query),
        &common_query,
        |b, pattern| {
            b.iter(|| bench_regex_case_insensitive(black_box(contents), black_box(pattern)));
        },
    );

    group.finish();

    // Compare exact match vs regex for simple patterns
    let mut group = c.benchmark_group("exact_vs_regex");
    group.throughput(Throughput::Bytes(file_size));

    group.bench_with_input(BenchmarkId::new("aho_corasick", common_query), &common_query, |b, query| {
        b.iter(|| bench_aho_corasick(black_box(contents), black_box(query)));
    });

    group.bench_with_input(BenchmarkId::new("regex", common_query), &common_query, |b, query| {
        b.iter(|| bench_regex(black_box(contents), black_box(query)));
    });

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
