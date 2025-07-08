use serde::{Deserialize, Serialize};
use std::{fmt, string::FromUtf8Error};

/// A Holochain-agnostic identifier that wraps the raw 39-byte representation
/// of a Holochain `ActionHash`.
///
/// This type intentionally avoids a direct dependency on Holochain by
/// representing the hash as a raw `Vec<u8>`. Consumers of this type must
/// assume and ensure that the data follows the binary layout expected by
/// `ActionHash::from_raw_39(...)`.
///
/// # Important
/// - This type does **not** include the `hash_type` metadata from `HoloHash<T>`.
/// - It is assumed that all `LocalId` values are ActionHashes. If you need to
///   support other Holochain hash types (e.g. `EntryHash`, `DnaHash`), you must
///   extend this type or encode the type information explicitly.
/// - Use conversion helpers (see below) in a Holochain-aware crate to safely
///   convert between `LocalId` and `ActionHash`.
///
/// # Invariants
/// - Must always contain exactly 39 bytes (Holochain’s canonical hash length)
///   if you intend to convert back into `ActionHash`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocalId(pub Vec<u8>);

impl LocalId {
    /// Creates a `LocalId` from raw bytes. Callers must ensure the byte
    /// format is valid for a Holochain ActionHash (39 bytes).
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Returns the raw bytes of the ID.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for LocalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match short_hash(self, 6) {
            Ok(s) => write!(f, "{}", s),
            Err(_) => write!(f, "<invalid utf-8>"),
        }
    }
}

/// Helper for truncating a LocalId for display.
pub fn short_hash(hash: &LocalId, length: usize) -> Result<String, FromUtf8Error> {
    let string = String::from_utf8(hash.0.clone())?; // try from inner bytes
    let start = string.len().saturating_sub(length);
    Ok(format!("…{}", &string[start..]))
}
