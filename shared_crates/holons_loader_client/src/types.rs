/// Raw file contents as seen by the loader client.
/// All filesystem access happens in the receptor layer.
#[derive(Debug, Clone)]
pub struct FileData {
    pub filename: String, // original path or logical name, used for provenance / errors
    pub raw_contents: String,
}

/// A full loader invocation: schema + the files to load.
/* 
#[derive(Debug, Clone)]
pub struct ContentSet {
    pub schema: FileData,
    pub files_to_load: Vec<FileData>,
}*/

/// Canonical on-disk path to the Holon Loader JSON Schema.
///
/// This path is intended for caller (receptor/test) use when constructing a ContentSet.
/// The loader client itself only works with in-memory file contents.
pub const BOOTSTRAP_IMPORT_SCHEMA_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../import_files/map-schema/bootstrap-import.schema.json"
);
