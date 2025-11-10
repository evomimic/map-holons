use clap::Parser;
use std::path::PathBuf;

use json_schema_validation::json_schema_validator::validate_json_against_schema;

/// Simple JSON-Schema validator.
///
/// ```bash
/// jsv --schema schema.json --file data.json
/// ```
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    /// Path to the JSON-Schema file
    #[arg(short, long)]
    schema: PathBuf,

    /// Path to the JSON instance file
    #[arg(short, long, value_name = "JSON")]
    file: PathBuf,
}

fn main() {
    let args = Cli::parse();

    match validate_json_against_schema(&args.schema, &args.file) {
        Ok(()) => {
            println!("✅  Validation succeeded.");
            std::process::exit(0);
        }
        Err(err) => {
            eprintln!("❌  Validation failed:\n{err}");
            std::process::exit(1);
        }
    }
}
