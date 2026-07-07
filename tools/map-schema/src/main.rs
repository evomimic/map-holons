use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use map_schema_tool::{
    decompile_input_string, decompile_inputs, dump_symbols, dump_symbols_from_string,
    diff_inputs,
    tdl_compiler::{
        check_input_string, check_inputs, compile_input_string, compile_inputs, render_check_output,
    },
};
use std::{
    io::{self, Read},
    path::PathBuf,
};

#[derive(Debug, Parser)]
#[command(author, version, about = "MAP schema authoring tool", disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Print a workflow-oriented guide for map-schema commands.
    Help,

    /// Decompile JSON import files into TDL source files.
    Decompile {
        /// Input JSON files or directories containing JSON files.
        inputs: Vec<PathBuf>,

        /// Output directory for generated .tdl files.
        #[arg(short = 'o', long = "out-dir", visible_alias = "out")]
        out_dir: Option<PathBuf>,
    },

    /// Print the derived semantic symbol table for JSON import files.
    Symbols {
        /// Input JSON files or directories containing JSON files.
        inputs: Vec<PathBuf>,
    },

    /// Compile TDL source files into JSON import files.
    Compile {
        /// Input TDL files or directories containing TDL files.
        inputs: Vec<PathBuf>,

        /// Output directory for generated JSON import files.
        #[arg(short = 'o', long = "out-dir", visible_alias = "out")]
        out_dir: Option<PathBuf>,
    },

    /// Validate TDL source files and report semantic diagnostics.
    Check {
        /// Input TDL files or directories containing TDL files.
        inputs: Vec<PathBuf>,
    },

    /// Compare two schema corpora by Canonical Holon IR semantics.
    Diff {
        /// Left-hand input files or directories. One source format per side.
        #[arg(long = "left", required = true)]
        left: Vec<PathBuf>,

        /// Right-hand input files or directories. One source format per side.
        #[arg(long = "right", required = true)]
        right: Vec<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Help => {
            print!("{}", map_schema_help());
        }
        Commands::Decompile { inputs, out_dir } => {
            if inputs.is_empty() {
                let stdin = read_stdin()?;
                print!("{}", decompile_input_string(&stdin, "stdin.json")?);
            } else if let Some(out_dir) = out_dir {
                let written = decompile_inputs(&inputs, &out_dir)?;
                println!("wrote {} TDL files to {}", written.len(), out_dir.display());
            } else {
                print!("{}", decompile_input_string(&read_single_input(&inputs)?, &inputs[0])?);
            }
        }
        Commands::Symbols { inputs } => {
            if inputs.is_empty() {
                let stdin = read_stdin()?;
                print!("{}", dump_symbols_from_string(&stdin, "stdin.json")?);
            } else {
                print!("{}", dump_symbols(&inputs)?);
            }
        }
        Commands::Compile { inputs, out_dir } => {
            if inputs.is_empty() {
                let stdin = read_stdin()?;
                print!("{}", compile_input_string(&stdin, "stdin.tdl")?);
            } else if let Some(out_dir) = out_dir {
                let written = compile_inputs(&inputs, &out_dir)?;
                println!("wrote {} JSON files to {}", written.len(), out_dir.display());
            } else {
                print!("{}", compile_input_string(&read_single_input(&inputs)?, &inputs[0])?);
            }
        }
        Commands::Check { inputs } => {
            let diagnostics = if inputs.is_empty() {
                let stdin = read_stdin()?;
                check_input_string(&stdin, "stdin.tdl")?
            } else {
                check_inputs(&inputs)?
            };
            print!("{}", render_check_output(&diagnostics));
        }
        Commands::Diff { left, right } => {
            print!("{}", diff_inputs(&left, &right)?);
        }
    }

    Ok(())
}

fn map_schema_help() -> &'static str {
    r#"map-schema helps maintain MAP schema import JSON and TDL source files.

Commands:
  help
      Print this workflow-oriented guide.

  decompile [JSON_FILE_OR_DIR ...] --out-dir <DIR>
      Convert loader JSON imports into TDL files. Directory inputs preserve
      relative paths and write one .tdl file per .json file.

  compile [TDL_FILE_OR_DIR ...] --out-dir <DIR>
      Convert TDL files into loader JSON imports. Compile works over a corpus:
      pass all TDL files needed to resolve references such as HolonType,
      PropertyType, DeclaredRelationshipType, and MapStringValueType.

  check [TDL_FILE_OR_DIR ...]
      Validate TDL syntax and semantic references without writing JSON.

  diff --left <JSON_OR_TDL_FILE_OR_DIR ...> --right <JSON_OR_TDL_FILE_OR_DIR ...>
      Compare two schema corpora by Canonical Holon IR semantics. Each side must
      lower without blocking diagnostics before a diff is produced.

  symbols [JSON_FILE_OR_DIR ...]
      Print the semantic symbol table derived from JSON imports.

Common workflows:
  npm run map-schema:decompile:coreschema
  npm run map-schema:check:coreschema
  npm run map-schema:compile:coreschema

Direct examples:
  cargo run --manifest-path tools/map-schema/Cargo.toml -- decompile host/import_files/map-schema/core-schema --out-dir schema-src
  cargo run --manifest-path tools/map-schema/Cargo.toml -- check schema-src
  cargo run --manifest-path tools/map-schema/Cargo.toml -- compile schema-src --out-dir generated/json-imports
  cargo run --manifest-path tools/map-schema/Cargo.toml -- diff --left host/import_files/map-schema/core-schema --right generated/json-imports

Single-file stdin/stdout mode:
  map-schema decompile < input.json > output.tdl
  map-schema compile < input.tdl > output.json

Notes:
  Decompile can inspect one JSON file, but dependency names are best resolved
  when the full JSON corpus is present.

  Compile validates semantic references against the TDL files passed in the
  same invocation. A dependent standalone file may fail until its core schema
  dependencies are included in the input corpus.

  Diff accepts JSON on one side and TDL on the other, but a single side must
  not mix source formats.
"#
}

fn read_stdin() -> Result<String> {
    let mut raw = String::new();
    io::stdin().read_to_string(&mut raw)?;
    Ok(raw)
}

fn read_single_input(inputs: &[PathBuf]) -> Result<String> {
    if inputs.len() != 1 {
        return Err(anyhow!(
            "multiple inputs require --out-dir; use stdin/stdout for one document or pass an explicit output directory for corpus transforms"
        ));
    }
    let input = &inputs[0];
    if input.is_dir() {
        return Err(anyhow!(
            "directory inputs require --out-dir; use stdin/stdout only for a single document"
        ));
    }
    std::fs::read_to_string(input).map_err(Into::into)
}
