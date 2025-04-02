use hdk::prelude::*;
use shared_types_holon::{MapString, TemporaryId};
// use uuid::Builder;

use crate::HolonError;

/// Generates a pseudo-UUID by calling random_bytes from HC
pub fn generate_temporary_id() -> Result<TemporaryId, HolonError> {
    let bytes: [u8; 16] = random_bytes(16)
        .map_err(|e| HolonError::from(e))?
        .into_vec()
        .try_into()
        .map_err(|_| HolonError::InvalidType("Expected 16 bytes".to_string()))?;

    // Create a 128-bit value and format it as a UUID-like string
    let uuid = format!(
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            bytes[0], bytes[1], bytes[2], bytes[3],
            bytes[4], bytes[5],
            bytes[6], bytes[7],
            bytes[8], bytes[9],
            bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
        );

    Ok(TemporaryId(MapString(uuid)))
}

// /// Generates a pseudo-UUID by calling random_bytes from HC
// pub fn generate_temporary_id() -> Result<TemporaryId, HolonError> {
//     let random_bytes: [u8; 16] = random_bytes(16)
//         .map_err(|e| HolonError::from(e))?
//         .into_vec()
//         .try_into()
//         .map_err(|_| HolonError::InvalidType("Expected 16 bytes".to_string()))?;

//     let uuid = Builder::from_random_bytes(random_bytes).into_uuid();

//     Ok(TemporaryId(uuid))
// }
