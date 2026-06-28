use anyhow::Result;
use clap::{Parser, Subcommand};
use map_schema_tool::decompile_inputs;
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Decompile { inputs, out } => {
            let written = decompile_inputs(&inputs, &out)?;
            println!("wrote {} TDL files to {}", written.len(), out.display());
        }
    }

    Ok(())
}
