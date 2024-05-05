use crate::Args;
use memmap2::MmapOptions;
use rayon::prelude::*;
use regex::Regex;
use std::error::Error;
use std::fs::File;

pub fn grep(args: Args) -> Result<(), Box<dyn Error>> {
    let file = File::open(args.file_path)?;
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    let content = unsafe { std::str::from_utf8_unchecked(&mmap) };

    let results = if args.use_regex {
        if args.ignore_case {
            search_regex_case_insensitive_parallel(&args.query, content)
        } else {
            search_regex_parallel(&args.query, content)
        }
    } else {
        if args.ignore_case {
            search_case_insensitive_parallel(&args.query, content)
        } else {
            search_parallel(&args.query, content)
        }
    };

    for line in results {
        println!("{line}");
    }

    Ok(())
}

// Simple search functions
fn search_parallel(query: &str, contents: &str) -> Vec<String> {
    contents
        .par_lines()
        .filter(|line| line.contains(query))
        .map(String::from)
        .collect()
}

fn search_case_insensitive_parallel(query: &str, contents: &str) -> Vec<String> {
    let query = query.to_lowercase();
    contents
        .par_lines()
        .filter(|line| line.to_lowercase().contains(&query))
        .map(String::from)
        .collect()
}

// Regex search functions
fn search_regex_parallel(query: &str, contents: &str) -> Vec<String> {
    let regex = Regex::new(query).unwrap();
    contents
        .par_lines()
        .filter(|line| regex.is_match(line))
        .map(String::from)
        .collect()
}

fn search_regex_case_insensitive_parallel(query: &str, contents: &str) -> Vec<String> {
    let query = format!("(?i){}", query);
    let regex = Regex::new(&query).unwrap();
    contents
        .par_lines()
        .filter(|line| regex.is_match(line))
        .map(String::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    static CONTENTS: &str = r#"
Rust:
safe, fast, productive.
Pick three
Duct tape
Trust me
"#;

    #[test]
    fn case_sensitive() {
        let query = "Duct";
        assert_eq!(vec!["Duct tape"], search_parallel(query, CONTENTS));
    }

    #[test]
    fn case_insensitive() {
        let query = "rUsT";
        assert_eq!(
            vec!["Rust:", "Trust me"],
            search_case_insensitive_parallel(query, CONTENTS)
        );
    }

    // Test for basic regex matching
    #[test]
    fn regex_basic_search() {
        let query = r"safe, fast, \w+."; // Matches "safe, fast, productive."
        assert_eq!(
            vec!["safe, fast, productive."],
            search_regex_parallel(query, CONTENTS)
        );
    }

    // Test for regex that includes a group and special characters
    #[test]
    fn regex_special_characters_search() {
        let query = r"\b\w+\b\."; // Match words followed by a period, ensuring word boundaries
        let expected = vec!["safe, fast, productive."];
        let results = search_regex_parallel(query, CONTENTS);
        assert_eq!(expected, results);
    }

    // Test for case insensitive regex search
    #[test]
    fn regex_case_insensitive_search() {
        let query = r"(?i)rust"; // Matches "Trust" and Rust
        assert_eq!(
            vec!["Rust:", "Trust me"],
            search_regex_case_insensitive_parallel(query, CONTENTS)
        );
    }

    // Test regex with no matches
    #[test]
    fn regex_no_match_search() {
        let query = r"nonexistent"; // Should match nothing
        let results = search_regex_parallel(query, CONTENTS);
        assert!(results.is_empty());
    }

    // Test for matching at the start and end of the text
    #[test]
    fn regex_edge_cases_search() {
        let query = r"^Rust:|me$"; // Matches "Rust:" at the start and "secure." at the end
        let expected = vec!["Rust:", "Trust me"]; // Ensure this is what you expect based on regex.
        let results = search_regex_parallel(query, CONTENTS);
        assert_eq!(expected, results);
    }
}
