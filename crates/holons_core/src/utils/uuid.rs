use fastrand::Rng;
use hdi::prelude::warn;
use shared_types_holon::TemporaryId;
use std::iter::repeat_with;

pub fn generate_temporary_id() -> TemporaryId {
    let mut rng = Rng::new();
    let random_bytes: Vec<u128> = repeat_with(|| rng.u128(..)).take(1_000_000).collect();

    warn!("RANDOM u128 :: {:?}", random_bytes);
    TemporaryId(random_bytes)
}

// use hdk::prelude::*;

// use crate::HolonError;

// pub fn generate_temporary_id() -> Result<TemporaryId, HolonError> {
//     let uuid_string = generate_uuid_string()?;

//     Ok(TemporaryId(uuid_string))
// }

// /// Generates a pseudo-UUID using fastrand.
// fn generate_uuid_string() -> Result<String, HolonError> {
//     // // Use sys_time() as a seed (with some entropy for uniqueness)
//     // let timestamp = sys_time().map_err(|e| HolonError::from(e))?.0; // Get timestamp as i64
//     // let seed = timestamp as u64;

//     let random_seed = fastrand::u64(..);
//     let mut rng = Rng::with_seed(random_seed);

//     // Create a 128-bit value and format it as a UUID-like string
//     Ok(format!(
//         "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
//         rng.u32(..),
//         rng.u16(..),
//         rng.u16(..) & 0x0fff | 0x4000, // Version 4 UUID style
//         rng.u16(..) & 0x3fff | 0x8000, // Variant bits
//         rng.u64(..) >> 16
//     ))
// }
