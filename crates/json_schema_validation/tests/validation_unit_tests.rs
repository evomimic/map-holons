use std::path::PathBuf;
use json_schema_validation::json_schema_validator::validate_json_against_schema;

#[test]
fn valid_json_passes_validation() {
    let schema = PathBuf::from("tests/fixtures/basic_schema.json");
    let data = PathBuf::from("tests/fixtures/basic_valid_example.json");

    assert!(validate_json_against_schema(&schema, &data).is_ok());
}

#[test]
fn invalid_json_fails_validation() {
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
fn valid_json_passes_validation_2() {
    let schema = PathBuf::from("tests/fixtures/complex_object_schema.json");
    let data = PathBuf::from("tests/fixtures/complex_object_valid_example.json");

    assert!(validate_json_against_schema(&schema, &data).is_ok());
}

#[test]
fn invalid_json_fails_validation_2() {
    let schema = PathBuf::from("tests/fixtures/complex_object_schema.json");
    let data = PathBuf::from("tests/fixtures/complex_object_invalid_example.json");

    let result = validate_json_against_schema(&schema, &data);
    assert!(result.is_err());

    if let Err(e) = result {
        let message = format!("{e}");

        println!("Validation error:\n{message}");
    }
}
