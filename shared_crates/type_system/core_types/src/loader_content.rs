use serde::{Deserialize, Serialize};

/// Raw file contents as seen by the loader client.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct FileData {
    pub filename: String,
    pub raw_contents: String,
}

/// A full loader invocation: the generated JSON import files to load.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct ContentSet {
    pub files_to_load: Vec<FileData>,
}
