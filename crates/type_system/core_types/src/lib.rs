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
pub mod maps;
// pub mod core_names;
pub mod type_kinds;
pub mod property_name;


pub use ids::*;
pub use maps::*;
// pub use core_names::*;
pub use type_kinds::*;
pub use property_name::*;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
