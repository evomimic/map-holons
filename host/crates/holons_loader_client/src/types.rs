/// Canonical on-disk path to the Holon Loader JSON Import File Validation Schema.
///
/// This path is intended for caller (receptor/test) use when constructing a ContentSet.
/// The loader client itself only works with in-memory file contents.
pub const BOOTSTRAP_IMPORT_VALIDATION_SCHEMA_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../import_files/map-schema/bootstrap-import.schema.json"
);
