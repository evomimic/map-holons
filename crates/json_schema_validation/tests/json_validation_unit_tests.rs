use std::path::PathBuf;
use json_schema_validation::json_schema_validator::validate_json_against_schema;

// Basic tests on generic JSON Schema validation
#[test]
fn basic_valid_json_passes_validation() {
    let schema = PathBuf::from("tests/fixtures/basic-schema.json");
    let data = PathBuf::from("tests/fixtures/basic-valid-example.json");

    assert!(validate_json_against_schema(&schema, &data).is_ok());
}

#[test]
fn basic_invalid_json_fails_validation() {
    let schema = PathBuf::from("tests/fixtures/basic-schema.json");
    let data = PathBuf::from("tests/fixtures/basic-invalid-example.json");

    let result = validate_json_against_schema(&schema, &data);
    assert!(result.is_err());

    if let Err(e) = result {
        let message = format!("{e}");

        println!("Validation error:\n{message}");
    }
}

#[test]
fn complex_object_valid_json_passes_validation() {
    let schema = PathBuf::from("tests/fixtures/complex-object-schema.json");
    let data = PathBuf::from("tests/fixtures/complex-object-valid-example.json");

    assert!(validate_json_against_schema(&schema, &data).is_ok());
}

#[test]
fn complex_object_invalid_json_fails_validation() {
    let schema = PathBuf::from("tests/fixtures/complex-object-schema.json");
    let data = PathBuf::from("tests/fixtures/complex-object-invalid-example.json");

    let result = validate_json_against_schema(&schema, &data);
    assert!(result.is_err());

    if let Err(e) = result {
        let message = format!("{e}");

        println!("Validation error:\n{message}");
    }
}

/// Test MAP metaschema for JSON schema validation
#[test]
fn validate_metaschema_files() {
    let schema = PathBuf::from("tests/fixtures/map_core_schema/bootstrap-import.schema.json");
    assert_all_pass(&schema, "tests/fixtures/map-metaschema-json");
}

/// Assert that *every* `.json` file in `dir` passes `schema` validation.
/// If one fails, the panic message shows which file and *why*.
fn assert_all_pass(schema: &PathBuf, dir: &str) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            match validate_json_against_schema(schema, &path) {
                Ok(()) => { /* good */ }
                Err(e) => panic!(
                    "❌  {} failed validation with error(s):\n{}\n",
                    path.display(),
                    e
                ),
            }
        }
    }
}

/// Test that invalid MAP files fail JSON schema validation
#[test]
fn edge_cases_all_fail() {
    let schema = PathBuf::from("tests/fixtures/map_core_schema/bootstrap-import.schema.json");
    assert_all_fail(&schema, "tests/fixtures/invalid-map-json");
}

/// Assert that *every* `.json` file in `dir` **fails** `schema` validation.
/// If one *passes*, we panic and say so.
fn assert_all_fail(schema: &PathBuf, dir: &str) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            match validate_json_against_schema(schema, &path) {
                Err(_) => { /* expected */ }
                Ok(()) => panic!(
                    "❌  {} unexpectedly passed validation (was supposed to fail)",
                    path.display()
                ),
            }
        }
    }
}