use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use map_schema_tool::{
    decompile_input_string, decompile_inputs, dump_symbols, dump_symbols_from_string,
    tdl_compiler::{
        check_input_string, check_inputs, compile_input_string, compile_inputs, render_check_output,
    },
};
use std::{
    io::{self, Read},
    path::PathBuf,
};

#[derive(Debug, Parser)]
#[command(author, version, about = "MAP schema authoring tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
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
    }

    Ok(())
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
