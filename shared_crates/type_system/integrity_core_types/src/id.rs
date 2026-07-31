use serde::{Deserialize, Serialize};
use std::fmt;

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
/// - It is assumed that all `LocalId` values are of type ActionHash (aka HoloHash<Action>).
///   If you need to support other Holochain hash types (e.g. `EntryHash`, `DnaHash`), you must
///   extend this type or encode the type information explicitly.
/// - Use conversion helpers (see below) in a Holochain-aware crate to safely
///   convert between `LocalId` and `ActionHash`.
///
/// # Invariants
/// - Must always contain exactly 39 bytes (Holochain’s canonical hash length)
///   if you intend to convert back into `ActionHash`.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
        // Display is for human-readable diagnostics only. Hex rather than UTF-8: a LocalId holds
        // raw hash bytes, which are never valid UTF-8. Truncated, so never an identity — do not
        // use for lookup keys or hashing. Use `Debug` when the full value is wanted.
        write!(f, "{}", short_hex(self, 8))
    }
}

impl fmt::Debug for LocalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalId")
            .field("bytes", &hex::encode(&self.0)) // or base64, or custom
            .finish()
    }
}

/// Returns the trailing `length` hex characters of a `LocalId`, for log diagnostics.
///
/// Hex rather than UTF-8: a `LocalId` wraps raw binary hash bytes, which are not valid UTF-8,
/// so any UTF-8 rendering fails for every genuine ActionHash. Cannot fail.
///
/// Truncated and hex-encoded, so it is for human reading only — never for identity, lookup
/// keys, or hashing.
pub fn short_hex(hash: &LocalId, length: usize) -> String {
    let encoded = hex::encode(&hash.0);
    let start = encoded.len().saturating_sub(length);
    format!("…{}", &encoded[start..])
}

/// A Holochain-agnostic identifier that wraps the raw 39-byte representation
/// of a Holochain `AgentPubKey`.
///
/// This type intentionally avoids a direct dependency on Holochain by
/// representing the hash as a raw `Vec<u8>`. Consumers of this type must
/// assume and ensure that the data follows the binary layout expected by
/// `AgentPubKey::from_raw_39(...)`.
///
/// # Important
/// - This type does **not** include the `hash_type` metadata from `HoloHash<T>`.
/// - It is assumed that all `PersistenceAgentId` values are of type AgentPubkey (aka HoloHash<Agent>).
/// - Use conversion helpers (see below) in a Holochain-aware crate to safely
///   convert between `LocalId` and `AgentPubKey`.
///
/// # Invariants
/// - Must always contain exactly 39 bytes (Holochain’s canonical hash length)
///   if you intend to convert back into `AgentPubKey`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PersistenceAgentId(pub Vec<u8>);

impl PersistenceAgentId {
    /// Creates a `PersistenceAgentId` from raw bytes. Callers must ensure the byte
    /// format is valid for a Holochain AgentPubKey (39 bytes).
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Returns the raw bytes of the ID.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A LocalId shaped like a real ActionHash: a 3-byte multihash prefix followed by 36 bytes of
    /// binary payload. Deliberately not valid UTF-8 — that is the normal case for a real hash.
    fn action_hash_shaped_local_id() -> LocalId {
        let mut bytes = vec![0x84, 0x29, 0x24];
        bytes.extend((0u8..36).map(|i| 0x80u8.wrapping_add(i)));
        assert_eq!(bytes.len(), 39, "fixture must be a canonical 39-byte hash");
        assert!(String::from_utf8(bytes.clone()).is_err(), "fixture must not be valid UTF-8");
        LocalId(bytes)
    }

    #[test]
    fn display_renders_hex_not_a_utf8_placeholder() {
        let rendered = action_hash_shaped_local_id().to_string();

        assert!(!rendered.contains("invalid utf-8"), "Display collapsed to a placeholder");
        let digits: String = rendered.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        assert_eq!(digits.len(), 8, "expected 8 hex chars, got {rendered:?}");
    }

    #[test]
    fn display_matches_trailing_hex_of_the_bytes() {
        let id = action_hash_shaped_local_id();
        let full_hex = hex::encode(id.as_bytes());

        assert_eq!(id.to_string(), format!("…{}", &full_hex[full_hex.len() - 8..]));
    }

    #[test]
    fn short_hex_tolerates_ids_shorter_than_the_requested_width() {
        // saturating_sub keeps this from panicking on a stub id used in tests.
        assert_eq!(short_hex(&LocalId(vec![0xab]), 8), "…ab");
    }
}
