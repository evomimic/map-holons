//! Base Types for the Memetic Activation Platform (MAP)
//!
//! This crate defines the foundational scalar and compound value types used
//! across the MAP ecosystem. These types are the lowest-level building blocks
//! in the MAP Type System and are used to represent runtime property values
//! within holons and related data structures.
//!
//! Types in this crate include:
//! - Scalar wrappers like `MapString`, `MapBoolean`, and `MapInteger`
//! - Enumeration values via `MapEnumValue`
//! - Byte arrays via `MapBytes`
//! - The `BaseValue` enum for representing dynamically typed property values
//!
//! These types are portable between guest-side and client-side environments
//! and are designed for serialization, hashing, and consistent formatting.

pub mod base_types;

pub use base_types::*;
