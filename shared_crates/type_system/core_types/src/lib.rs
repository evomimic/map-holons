//! Core Types for the Memetic Activation Platform (MAP)
//!
//! This crate defines core semantic and structural types that form the foundation
//! of the MAP Type System. These types are concerned with the **schema-level
//! representation** of MAP data — including names, identifiers, and structural
//! classifications.
//!
//! Key type categories include:
//! - **Semantic Names**: such as `PropertyName`, `RelationshipName`, and `SchemaName`
//! - **Identifiers**: such as `LocalId`, `ExternalId`, and `HolonId`
//! - **Structural Types**: like `PropertyMap`, `RelationshipMap`
//! - **Type Classifiers**: such as `ValueType` and `TypeKind` (formerly `TypeKind`)
//!
//! These types define the shape and meaning of data in MAP holons and descriptors,
//! and are shared across guest and client implementations.

pub mod ids;
pub mod type_kinds;

pub use ids::*;
pub use type_kinds::*;

//Re-export selected integrity_core_types at the root.
// Prefer explicit lists over globs to keep the API curated and stable.
pub use integrity_core_types::{
    HolonError, HolonNodeModel, LocalId, PersistenceAgentId, PersistenceTimestamp, PropertyMap,
    PropertyName, PropertyValue, RelationshipName, ValidationError,
};

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn it_works() {
//         let result = add(2, 2);
//         assert_eq!(result, 4);
//     }
// }
