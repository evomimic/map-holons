use anyhow::Result;
use clap::{Parser, Subcommand};
use map_schema_tool::{
    diagnostics::format_diagnostics,
    decompile_inputs, dump_symbols,
    tdl_compiler::{check_inputs, compile_inputs},
};
use std::path::PathBuf;

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
        #[arg(required = true)]
        inputs: Vec<PathBuf>,

        /// Output directory for generated .tdl files.
        #[arg(short, long)]
        out: PathBuf,
    },

    /// Print the derived semantic symbol table for JSON import files.
    Symbols {
        /// Input JSON files or directories containing JSON files.
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
    },

    /// Compile TDL source files into JSON import files.
    Compile {
        /// Input TDL files or directories containing TDL files.
        #[arg(required = true)]
        inputs: Vec<PathBuf>,

        /// Output directory for generated JSON import files.
        #[arg(short, long)]
        out: PathBuf,
    },

    /// Validate TDL source files and report semantic diagnostics.
    Check {
        /// Input TDL files or directories containing TDL files.
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Decompile { inputs, out } => {
            let written = decompile_inputs(&inputs, &out)?;
            println!("wrote {} TDL files to {}", written.len(), out.display());
        }
        Commands::Symbols { inputs } => {
            print!("{}", dump_symbols(&inputs)?);
        }
        Commands::Compile { inputs, out } => {
            let written = compile_inputs(&inputs, &out)?;
            println!("wrote {} JSON files to {}", written.len(), out.display());
        }
        Commands::Check { inputs } => {
            let diagnostics = check_inputs(&inputs)?;
            if diagnostics.is_empty() {
                println!("no diagnostics");
            } else {
                println!("{}", format_diagnostics(&diagnostics));
            }
        }
    }

    Ok(())
}
