use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use map_schema_tool::{
    bootstrap_bundle::generate_core_schema_bootstrap_bundle,
    decompile_input_string, decompile_inputs, roundtrip_json_inputs,
    tdl_compiler::{
        check_input_string, check_inputs, compile_input_string, compile_inputs,
        render_check_output, type_definition_counts,
    },
};
use std::{
    io::{self, IsTerminal, Read},
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

    /// Compile TDL source files into JSON import files.
    Compile {
        /// Input TDL files or directories containing TDL files.
        inputs: Vec<PathBuf>,

        /// Output directory for generated JSON import files.
        #[arg(short = 'o', long = "out-dir", visible_alias = "out")]
        out_dir: Option<PathBuf>,
    },

    /// Build the manifest-selected Core Schema bootstrap distribution bundle.
    BootstrapBundle {
        /// Root directory containing generated canonical schema JSON imports.
        import_root: PathBuf,

        /// Source manifest selecting the operational schema packages to bootstrap.
        #[arg(long)]
        manifest: PathBuf,

        /// Output directory for the bootstrap bundle.
        #[arg(short = 'o', long = "out-dir", visible_alias = "out")]
        out_dir: PathBuf,
    },

    /// Validate TDL syntax and lowering diagnostics.
    Check {
        /// Input TDL files or directories containing TDL files.
        inputs: Vec<PathBuf>,
    },

    /// Decompile JSON to scratch TDL, recompile it, and compare loader-fact signatures.
    RoundtripJson {
        /// Input JSON files or directories containing JSON files.
        inputs: Vec<PathBuf>,

        /// Output directory for scratch decompiled TDL files.
        #[arg(long = "tdl-out")]
        tdl_out: PathBuf,

        /// Output directory for scratch recompiled JSON files.
        #[arg(long = "json-out")]
        json_out: PathBuf,
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
        Commands::Compile { inputs, out_dir } => {
            if inputs.is_empty() {
                let stdin = read_stdin()?;
                print!("{}", compile_input_string(&stdin, "stdin.tdl")?);
            } else if let Some(out_dir) = out_dir {
                let written = compile_inputs(&inputs, &out_dir)?;
                println!("wrote {} JSON files to {}", written.len(), out_dir.display());
                println!("defined type descriptors by schema:");
                for (schema, counts) in type_definition_counts(&inputs)? {
                    let total = counts.values().sum::<usize>();
                    println!("  {schema}: {total}");
                    for (type_kind, count) in counts {
                        println!("    {type_kind}: {count}");
                    }
                }
            } else {
                print!("{}", compile_input_string(&read_single_input(&inputs)?, &inputs[0])?);
            }
        }
        Commands::BootstrapBundle { import_root, manifest, out_dir } => {
            generate_core_schema_bootstrap_bundle(&import_root, &manifest, &out_dir)?;
            println!("wrote Core Schema bootstrap bundle to {}", out_dir.display());
        }
        Commands::Check { inputs } => {
            let diagnostics = if inputs.is_empty() {
                if io::stdin().is_terminal() {
                    return Err(missing_check_input_error());
                }
                let stdin = read_stdin()?;
                if stdin.trim().is_empty() {
                    return Err(missing_check_input_error());
                }
                check_input_string(&stdin, "stdin.tdl")?
            } else {
                check_inputs(&inputs)?
            };
            print!("{}", render_check_output(&diagnostics));
        }
        Commands::RoundtripJson { inputs, tdl_out, json_out } => {
            if inputs.is_empty() {
                return Err(anyhow!("roundtrip-json requires at least one JSON input"));
            }
            let report = roundtrip_json_inputs(&inputs, &tdl_out, &json_out)?;
            println!(
                "roundtrip ok: wrote {} TDL files to {} and {} JSON files to {}",
                report.decompiled_files.len(),
                tdl_out.display(),
                report.compiled_files.len(),
                json_out.display()
            );
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
      Convert generated loader JSON into TDL files. Directory inputs preserve
      relative paths and write one .tdl file per .json file.

  compile [TDL_FILE_OR_DIR ...] --out-dir <DIR>
      Convert TDL files into generated loader JSON. Compile works over a corpus.

  bootstrap-bundle GENERATED_IMPORT_ROOT --manifest <FILE> --out-dir <DIR>
      Create the manifest-selected operational CoreSchemaSpace bootstrap bundle.

  check [TDL_FILE_OR_DIR ...]
      Validate TDL syntax and lowering constraints without writing JSON.

  roundtrip-json [JSON_FILE_OR_DIR ...] --tdl-out <DIR> --json-out <DIR>
      Decompile JSON to scratch TDL, recompile that TDL to canonical JSON,
      and compare deterministic loader-fact signatures.

Common workflows:
  npm run map-schema:check:coreschema
  npm run map-schema:compile:coreschema
  npm run map-schema:roundtrip:coreschema

Direct examples:
  cargo run --manifest-path tools/map-schema/Cargo.toml -- check schema-src
  cargo run --manifest-path tools/map-schema/Cargo.toml -- compile schema-src --out-dir generated/json-imports
  cargo run --manifest-path tools/map-schema/Cargo.toml -- roundtrip-json generated/json-imports --tdl-out generated/tdl-decompiled --json-out generated/json-roundtrip

Single-file stdin/stdout mode:
  map-schema decompile < input.json > output.tdl
  map-schema compile < input.tdl > output.json

Notes:
  Decompile can inspect one generated JSON file, but corpus context is best
  supplied when preserving cross-file source relationships.

  Compile validates TDL syntax and lowering constraints across the files passed
  in the same invocation. Descriptor semantics remain outside source tooling.
"#
}

fn read_stdin() -> Result<String> {
    let mut raw = String::new();
    io::stdin().read_to_string(&mut raw)?;
    Ok(raw)
}

fn missing_check_input_error() -> anyhow::Error {
    anyhow!(
        "map-schema check needs TDL input; use `npm run map-schema:check:coreschema`, `npm run map-schema:check -- schema-src`, or pipe one TDL document on stdin"
    )
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
