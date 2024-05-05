use crate::Args;
use memmap2::MmapOptions;
use rayon::prelude::*;
use regex::Regex;
use std::error::Error;
use std::fs::File;

pub fn grep(args: Args) -> Result<Vec<String>, Box<dyn Error>> {
    let file = File::open(args.file_path)?;
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    let content = unsafe { std::str::from_utf8_unchecked(&mmap) };

    return Ok(if args.use_regex {
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
    });
}

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

    #[test]
    fn case_sensitive() {
        let args = Args {
            file_path: String::from("poem.txt"),
            query: String::from("How"),
            ignore_case: false,
            use_regex: false,
        };

        assert_eq!(
            vec!["How dreary to be somebody!", "How public, like a frog"],
            grep(args).unwrap()
        );
    }

    #[test]
    fn case_insensitive() {
        let args = Args {
            file_path: String::from("poem.txt"),
            query: String::from("how"),
            ignore_case: true,
            use_regex: false,
        };

        assert_eq!(
            vec!["How dreary to be somebody!", "How public, like a frog"],
            grep(args).unwrap()
        );
    }

    // Test for basic regex matching
    #[test]
    fn regex_basic_search() {
        let args = Args {
            file_path: String::from("poem.txt"),
            query: String::from(r" to\b"),
            ignore_case: false,
            use_regex: true,
        };

        assert_eq!(vec!["How dreary to be somebody!"], grep(args).unwrap());
    }

    // Test for regex that includes a group and special characters
    #[test]
    fn regex_special_characters_search() {
        let args = Args {
            file_path: String::from("poem.txt"),
            query: String::from(r" \b\w+\b\!"),
            ignore_case: false,
            use_regex: true,
        };

        assert_eq!(
            vec![
                "I'm nobody! Who are you?",
                "Then there's a pair of us - don't tell!",
                "How dreary to be somebody!",
                "To an admiring bog!"
            ],
            grep(args).unwrap()
        );
    }

    // Test for case insensitive regex search
    #[test]
    fn regex_case_insensitive_search() {
        let args = Args {
            file_path: String::from("poem.txt"),
            query: String::from(r"^to"),
            ignore_case: true,
            use_regex: true,
        };

        assert_eq!(
            vec!["To tell your name the livelong day", "To an admiring bog!"],
            grep(args).unwrap()
        );
    }

    // Test regex with no matches
    #[test]
    fn regex_no_match_search() {
        let args = Args {
            file_path: String::from("poem.txt"),
            query: String::from(r"Does not exist"),
            ignore_case: true,
            use_regex: true,
        };
        assert!(grep(args).unwrap().is_empty());
    }

    // Test for matching at the start and end of the text
    #[test]
    fn regex_edge_cases_search() {
        let args = Args {
            file_path: String::from("poem.txt"),
            query: String::from(r"^Are.*too\?$"),
            ignore_case: true,
            use_regex: true,
        };

        assert_eq!(vec!["Are you nobody, too?"], grep(args).unwrap());
    }
}
