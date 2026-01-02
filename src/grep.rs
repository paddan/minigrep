use crate::Args;
use aho_corasick::AhoCorasick;
use memchr::memmem;
use memmap2::MmapOptions;
use rayon::prelude::*;
use regex::bytes::RegexBuilder;
use std::error::Error;
use std::fs::File;
use std::io::{self, BufWriter, Write};

/// Minimum chunk size for parallel processing (64KB)
const MIN_CHUNK_SIZE: usize = 64 * 1024;

/// Performs a grep-like search on the contents of a file, using the specified search options.
///
/// This function takes in an `Args` struct that contains the search query, file path, and various
/// search options (e.g. case-insensitive, use regex). It then opens the file, maps it into memory,
/// and performs the search using the appropriate parallel search function based on the provided
/// options.
///
/// Results are streamed directly to stdout, avoiding allocation of a result vector.
pub fn grep(args: Args) -> Result<(), Box<dyn Error>> {
    let file = File::open(&args.file_path)?;
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    let content: &[u8] = &mmap;

    if args.use_regex {
        let regex = RegexBuilder::new(&args.query)
            .case_insensitive(args.ignore_case)
            .size_limit(10 * (1 << 20))  // 10MB compiled size limit
            .dfa_size_limit(10 * (1 << 20))
            .build()?;
        search_regex_parallel(&regex, content);
    } else if args.ignore_case {
        // Use Aho-Corasick for case-insensitive (handles ASCII case folding efficiently)
        let ac = AhoCorasick::builder()
            .ascii_case_insensitive(true)
            .build([&args.query])?;
        search_aho_corasick_parallel(&ac, content);
    } else {
        // Use memchr's memmem for case-sensitive literal search (SIMD accelerated)
        let finder = memmem::Finder::new(args.query.as_bytes());
        search_memmem_parallel(&finder, content);
    }

    Ok(())
}

/// Search using memchr's memmem (SIMD-accelerated literal search)
fn search_memmem_parallel(finder: &memmem::Finder, contents: &[u8]) {
    let matches = search_chunks_parallel(contents, |line| finder.find(line).is_some());
    write_matches(matches);
}

/// Search using Aho-Corasick automaton
fn search_aho_corasick_parallel(ac: &AhoCorasick, contents: &[u8]) {
    let matches = search_chunks_parallel(contents, |line| ac.find(line).is_some());
    write_matches(matches);
}

/// Search using regex
fn search_regex_parallel(regex: &regex::bytes::Regex, contents: &[u8]) {
    let matches = search_chunks_parallel(contents, |line| regex.is_match(line));
    write_matches(matches);
}

/// Parallel search with chunked processing for better cache locality
fn search_chunks_parallel<'a, F>(contents: &'a [u8], matcher: F) -> Vec<&'a [u8]>
where
    F: Fn(&[u8]) -> bool + Sync,
{
    // For small files, use simple parallel line iteration
    if contents.len() < MIN_CHUNK_SIZE * 2 {
        return contents
            .par_split(|&b| b == b'\n')
            .filter(|line| matcher(line))
            .collect();
    }

    // For larger files, split into chunks first for better cache locality
    let num_threads = rayon::current_num_threads();
    let chunk_size = (contents.len() / num_threads).max(MIN_CHUNK_SIZE);

    // Find chunk boundaries at newlines
    let mut chunk_starts = vec![0usize];
    let mut pos = chunk_size;
    while pos < contents.len() {
        // Find next newline after pos
        if let Some(nl_offset) = memchr::memchr(b'\n', &contents[pos..]) {
            pos += nl_offset + 1;
            if pos < contents.len() {
                chunk_starts.push(pos);
            }
        } else {
            break;
        }
        pos += chunk_size;
    }

    // Process chunks in parallel
    chunk_starts
        .par_iter()
        .enumerate()
        .flat_map(|(i, &start)| {
            let end = if i + 1 < chunk_starts.len() {
                chunk_starts[i + 1]
            } else {
                contents.len()
            };
            let chunk = &contents[start..end];
            chunk
                .split(|&b| b == b'\n')
                .filter(|line| matcher(line))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Write matched lines to stdout with buffering
fn write_matches(matches: Vec<&[u8]>) {
    let stdout = io::stdout().lock();
    let mut writer = BufWriter::with_capacity(64 * 1024, stdout);
    for line in matches {
        if writer.write_all(line).is_err() || writer.write_all(b"\n").is_err() {
            break; // Stop on broken pipe or other write errors
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
}
