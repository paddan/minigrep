use crate::Args;
use aho_corasick::AhoCorasick;
use memchr::memmem;
use memmap2::{Advice, MmapOptions};
use rayon::prelude::*;
use regex::bytes::RegexBuilder;
use std::error::Error;
use std::fs::File;
use std::io::{self, BufWriter, Write};

/// Minimum chunk size — ensures enough work per thread.
const MIN_CHUNK_SIZE: usize = 64 * 1024;

/// Maximum chunk size — fits in L2 on modern cores and amortizes per-chunk
/// fixed costs (SIMD scan setup, allocation) over more matches.
const MAX_CHUNK_SIZE: usize = 512 * 1024;

/// Target chunks per rayon worker. Roughly N gives rayon some headroom to
/// steal short-running chunks from busy threads without blowing up the
/// per-chunk overhead.
const CHUNKS_PER_THREAD: usize = 3;

fn optimal_chunk_size(file_size: usize) -> usize {
    let num_threads = rayon::current_num_threads().max(1);
    let target_chunks = num_threads * CHUNKS_PER_THREAD;
    (file_size / target_chunks).clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE)
}

/// Mmap the input file and dispatch to the matcher selected by `args` (regex,
/// case-insensitive Aho-Corasick, or memmem literal). Matches are written to
/// stdout in input order via a single buffered writer. Symlinks are rejected
/// up front. Returns an error if the file cannot be opened or the regex fails
/// to compile.
pub fn grep(args: Args) -> Result<(), Box<dyn Error>> {
    if std::fs::symlink_metadata(&args.file_path)?
        .file_type()
        .is_symlink()
    {
        return Err("Symlinks not allowed".into());
    }
    let file = File::open(&args.file_path)?;
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    let _ = mmap.advise(Advice::Sequential);
    let content: &[u8] = &mmap;

    let outputs = if args.use_regex {
        // multi_line makes ^/$ match at \n boundaries within the chunk, which
        // is the standard grep semantic and lets us run find_iter over the
        // whole chunk instead of testing each line separately.
        let regex = RegexBuilder::new(&args.query)
            .case_insensitive(args.ignore_case)
            .multi_line(true)
            .size_limit(10 * (1 << 20))
            .dfa_size_limit(10 * (1 << 20))
            .build()?;
        process_chunks(content, |chunk| {
            collect_lines(chunk, regex.find_iter(chunk).map(|m| m.start()))
        })
    } else if args.ignore_case {
        let ac = AhoCorasick::builder()
            .ascii_case_insensitive(true)
            .build([&args.query])?;
        process_chunks(content, |chunk| {
            collect_lines(chunk, ac.find_iter(chunk).map(|m| m.start()))
        })
    } else {
        let finder = memmem::Finder::new(args.query.as_bytes());
        process_chunks(content, |chunk| {
            collect_lines(chunk, finder.find_iter(chunk))
        })
    };

    write_outputs(outputs);
    Ok(())
}

/// Walk newline-aligned chunks in parallel; each chunk produces its own
/// pre-formatted output buffer that we stream to stdout in input order.
fn process_chunks<F>(contents: &[u8], process_chunk: F) -> Vec<Vec<u8>>
where
    F: Fn(&[u8]) -> Vec<u8> + Sync,
{
    if contents.is_empty() {
        return Vec::new();
    }
    let ranges = build_chunk_ranges(contents);
    ranges
        .par_iter()
        .map(|&(start, end)| process_chunk(&contents[start..end]))
        .collect()
}

/// For each match position, expand to the full line and emit it. The
/// `next_line_start` cursor ensures a line containing multiple matches is
/// emitted only once and lets us skip ahead via SIMD between matches instead
/// of scanning every line.
fn collect_lines<I: Iterator<Item = usize>>(chunk: &[u8], positions: I) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut next_line_start = 0usize;
    for pos in positions {
        if pos < next_line_start {
            // Match is on a line we've already emitted (multiple hits per line).
            continue;
        }
        let line_start = memchr::memrchr(b'\n', &chunk[..pos]).map_or(0, |p| p + 1);
        let line_end = memchr::memchr(b'\n', &chunk[pos..]).map_or(chunk.len(), |p| pos + p);
        out.extend_from_slice(&chunk[line_start..line_end]);
        out.push(b'\n');
        next_line_start = line_end + 1;
    }
    out
}

fn build_chunk_ranges(contents: &[u8]) -> Vec<(usize, usize)> {
    let file_size = contents.len();
    if file_size < MIN_CHUNK_SIZE * 2 {
        return vec![(0, file_size)];
    }
    let chunk_size = optimal_chunk_size(file_size);
    let mut ranges = Vec::new();
    let mut start = 0usize;
    let mut pos = chunk_size;
    while pos < file_size {
        match memchr::memchr(b'\n', &contents[pos..]) {
            Some(nl_offset) => {
                let boundary = pos + nl_offset + 1;
                ranges.push((start, boundary));
                start = boundary;
                pos = boundary + chunk_size;
            }
            None => break,
        }
    }
    ranges.push((start, file_size));
    ranges
}

fn write_outputs(outputs: Vec<Vec<u8>>) {
    let stdout = io::stdout().lock();
    let mut writer = BufWriter::with_capacity(64 * 1024, stdout);
    for buf in outputs {
        if buf.is_empty() {
            continue;
        }
        if writer.write_all(&buf).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    fn run_grep(query: &str, file: &str, ignore_case: bool, use_regex: bool) -> Vec<String> {
        let mut cmd = Command::new("cargo");
        cmd.args(["run", "--quiet", "--"]);
        if ignore_case {
            cmd.arg("-i");
        }
        if use_regex {
            cmd.arg("-r");
        }
        cmd.arg(query).arg(file);

        let output = cmd.output().expect("Failed to execute command");
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout
            .lines()
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect()
    }

    #[test]
    fn case_sensitive() {
        let result = run_grep("How", "poem.txt", false, false);
        assert_eq!(
            vec!["How dreary to be somebody!", "How public, like a frog"],
            result
        );
    }

    #[test]
    fn case_insensitive() {
        let result = run_grep("how", "poem.txt", true, false);
        assert_eq!(
            vec!["How dreary to be somebody!", "How public, like a frog"],
            result
        );
    }

    #[test]
    fn regex_basic_search() {
        let result = run_grep(r" to\b", "poem.txt", false, true);
        assert_eq!(vec!["How dreary to be somebody!"], result);
    }

    #[test]
    fn regex_special_characters_search() {
        let result = run_grep(r" \b\w+\b\!", "poem.txt", false, true);
        assert_eq!(
            vec![
                "I'm nobody! Who are you?",
                "Then there's a pair of us - don't tell!",
                "How dreary to be somebody!",
                "To an admiring bog!"
            ],
            result
        );
    }

    #[test]
    fn regex_case_insensitive_search() {
        let result = run_grep(r"^to", "poem.txt", true, true);
        assert_eq!(
            vec!["To tell your name the livelong day", "To an admiring bog!"],
            result
        );
    }

    #[test]
    fn regex_no_match_search() {
        let result = run_grep(r"Does not exist", "poem.txt", true, true);
        assert!(result.is_empty());
    }

    #[test]
    fn regex_edge_cases_search() {
        let result = run_grep(r"^Are.*too\?$", "poem.txt", true, true);
        assert_eq!(vec!["Are you nobody, too?"], result);
    }

    #[test]
    fn aho_corasick_search() {
        let result = run_grep("nobody", "poem.txt", false, false);
        assert_eq!(
            vec!["I'm nobody! Who are you?", "Are you nobody, too?"],
            result
        );
    }

    // Locks down the next_line_start dedup in collect_lines: every line in
    // poem.txt contains at least one 'o' (most contain several), so a broken
    // dedup would emit duplicate lines instead of one per matching line. Also
    // exercises the trailing-line path since poem.txt has no final newline.
    #[test]
    fn dedup_multiple_matches_per_line_literal() {
        let result = run_grep("o", "poem.txt", false, false);
        assert_eq!(
            vec![
                "I'm nobody! Who are you?",
                "Are you nobody, too?",
                "Then there's a pair of us - don't tell!",
                "They'd banish us, you know.",
                "How dreary to be somebody!",
                "How public, like a frog",
                "To tell your name the livelong day",
                "To an admiring bog!",
            ],
            result
        );
    }

    // Same dedup contract via the regex path (multi_line + find_iter).
    #[test]
    fn dedup_multiple_matches_per_line_regex() {
        let result = run_grep("o", "poem.txt", false, true);
        assert_eq!(result.len(), 8);
    }

    // poem.txt has no trailing newline, so this also locks down the
    // trailing-data path in collect_lines (the line is on the file's last
    // byte range, not preceded by another \n).
    #[test]
    fn matches_last_line_without_trailing_newline() {
        let result = run_grep("bog", "poem.txt", false, false);
        assert_eq!(vec!["To an admiring bog!"], result);
    }

    // Zero-byte file should yield zero output regardless of query.
    #[test]
    fn empty_file_returns_no_matches() {
        let path = std::env::temp_dir().join("mg_test_empty_file.txt");
        std::fs::File::create(&path).expect("create temp file");
        let path_str = path.to_str().expect("temp path is utf-8");
        let result = run_grep("anything", path_str, false, false);
        let _ = std::fs::remove_file(&path);
        assert!(result.is_empty());
    }
}
