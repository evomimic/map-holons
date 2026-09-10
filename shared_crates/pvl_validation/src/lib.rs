//! Descriptor-independent Peer Validation Layer (PVL) rules.
//!
//! This crate validates native storage envelopes and lifecycle inputs without
//! resolving MAP descriptors. Keeping that boundary explicit makes the crate
//! safe for Holochain WASM integrity code and separates deterministic storage
//! checks from descriptor-aware schema conformance.

pub mod holon_node_envelope;
pub mod holon_node_lifecycle;
pub mod holon_node_properties;
pub mod identifier_validation;
pub mod link_lifecycle;
pub mod pvl_limits_v1;
pub mod smartlink_envelope;

// Present the validation operations and diagnostics as a single public façade.
pub use holon_node_envelope::*;
pub use holon_node_lifecycle::*;
pub use holon_node_properties::*;
pub use identifier_validation::*;
pub use link_lifecycle::*;
pub use smartlink_envelope::*;

pub use integrity_core_types::{PvlField, PvlMalformedReason, PvlViolation};
