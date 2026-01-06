use base_types::MapString;
use core_types::TemporaryId;
use sha2::{Digest, Sha256};
use uuid::Builder;

pub fn create_temporary_id_from_key(key: &MapString) -> TemporaryId {
    let mut hasher = Sha256::new();
    hasher.update(key.0.clone());
    let hash = hasher.finalize();

    // Take the first 16 bytes for UUID
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&hash[..16]);

    // Set UUID variant RFC4122 version Custom
    let uuid = Builder::from_custom_bytes(bytes.clone()).into_uuid();

    TemporaryId(uuid)
}
