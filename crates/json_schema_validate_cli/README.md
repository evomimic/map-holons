# json_schema_validate_cli

A simple command-line interface for validating JSON files against a JSON Schema, using the `json_schema_validation` utility.

## Features

- Validates any JSON file against any JSON Schema (Draft 7+).
- Human-friendly output for both success and error cases.
- Exits with code 0 (success) or 1 (failure) for use in CI/CD or scripts.

## Usage

First, build the CLI:
```
cargo build -p json_schema_validate_cli --release
```
This creates the executable at `./target/release/jsv`.

## Basic validation
```
./target/release/jsv --schema schema.json --file data.json
```

### Options

```
>jsv --help

Simple JSON-Schema validator.

Usage: jsv --schema <SCHEMA> --file <JSON>

Options:
  -s, --schema <SCHEMA>   Path to the JSON-Schema file
  -f, --file <JSON>       Path to the JSON instance file
  -h, --help              Print help
  -V, --version           Print version
```
- Returns ✅ Validation succeeded. on success.
- Returns ❌ Validation failed: and lists errors on failure.

## Example Command

`./target/release/jsv --schema bootstrap-import.schema.json --file metaschema-root.json`
