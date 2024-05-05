pub mod grep;

use argh::FromArgs;
use grep::grep;
use std::process;

#[derive(FromArgs)]
#[argh(description = "Fast grep")]
pub struct Args {
    #[argh(positional)]
    pub query: String,

    #[argh(positional)]
    pub file_path: String,

    #[argh(switch, short = 'i', long = "ignore-case", description = "ignore case")]
    pub ignore_case: bool,

    #[argh(switch, short = 'r', long = "use-regex", description = "use regex")]
    pub use_regex: bool,
}

fn main() {
    let args: Args = argh::from_env();

    match grep(args) {
        Ok(lines) => {
            for line in lines {
                println!("{line}")
            }
        }
        Err(e) => {
            eprintln!("Application error: {e}");
            process::exit(1);
        }
    }
}
