use std::path::PathBuf;
use json_schema_validation::json_schema_validator::validate_json_against_schema;

// Basic tests on generic JSON Schema validation
#[test]
fn basic_valid_json_passes_validation() {
    let schema = PathBuf::from("tests/fixtures/basic_schema.json");
    let data = PathBuf::from("tests/fixtures/basic_valid_example.json");

    assert!(validate_json_against_schema(&schema, &data).is_ok());
}

#[test]
fn basic_invalid_json_fails_validation() {
    let schema = PathBuf::from("tests/fixtures/basic_schema.json");
    let data = PathBuf::from("tests/fixtures/basic_invalid_example.json");

    let result = validate_json_against_schema(&schema, &data);
    assert!(result.is_err());

    if let Err(e) = result {
        let message = format!("{e}");

        println!("Validation error:\n{message}");
    }
}

#[test]
fn complex_object_valid_json_passes_validation() {
    let schema = PathBuf::from("tests/fixtures/complex_object_schema.json");
    let data = PathBuf::from("tests/fixtures/complex_object_valid_example.json");

    assert!(validate_json_against_schema(&schema, &data).is_ok());
}

#[test]
fn complex_object_invalid_json_fails_validation() {
    let schema = PathBuf::from("tests/fixtures/complex_object_schema.json");
    let data = PathBuf::from("tests/fixtures/complex_object_invalid_example.json");

    let result = validate_json_against_schema(&schema, &data);
    assert!(result.is_err());

    if let Err(e) = result {
        let message = format!("{e}");

        println!("Validation error:\n{message}");
    }
}

// Tests for MAP core schema validation
#[test]
fn validate_core_and_meta_files() {
    let schema = PathBuf::from("tests/fixtures/map_core_schema/bootstrap-import.schema.json");
    let mut data = PathBuf::from("tests/fixtures/map_core_schema/map-meta-schema.json");

    let mut result = validate_json_against_schema(&schema, &data);
    assert!(result.is_ok(), "Validation failed for map-meta-schema.json: {:?}", result);

    data = PathBuf::from("tests/fixtures/map_core_schema/map-meta-value-types.json");
    result = validate_json_against_schema(&schema, &data);
    assert!(result.is_ok(), "Validation failed for map-meta-value-types.json: {:?}", result);

    data = PathBuf::from("tests/fixtures/map_core_schema/map-keyrule-types.json");
    result = validate_json_against_schema(&schema, &data);
    assert!(result.is_ok(), "Validation failed for map-keyrule-types.json: {:?}", result);

    data = PathBuf::from("tests/fixtures/map_core_schema/map-base-schema.json");
    result = validate_json_against_schema(&schema, &data);
    assert!(result.is_ok(), "Validation failed for map-base-schema.json: {:?}", result);
}