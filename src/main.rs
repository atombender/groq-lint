mod reporter;

use clap::Parser;
use groq_lint::lint;
use std::fs;
use std::io::{self, Read};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// GROQ query or file path
    input: Option<String>,

    /// Number of context lines around findings (0 = show all)
    #[arg(short = 'C', long, default_value = "3")]
    context: usize,
}

fn main() {
    let args = Args::parse();

    let query = match args.input {
        Some(path_or_query) => {
            if let Ok(content) = fs::read_to_string(&path_or_query) {
                content
            } else {
                path_or_query
            }
        }
        None => {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer).unwrap();
            buffer
        }
    };

    if query.trim().is_empty() {
        eprintln!("No query provided.");
        std::process::exit(1);
    }

    match lint(&query) {
        Ok(findings) => {
            reporter::print_report(&query, &findings, args.context);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
