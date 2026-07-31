//! Holochain substrate adapter for MAP Integrity validation.
//!
//! This crate forms the boundary between substrate-independent MAP domain rules
//! and the underlying Holochain platform. It resolves Holochain-specific inputs
//! and dependencies, projects them into the narrow domain models consumed by
//! shared validation, and preserves the distinction between platform execution
//! failures and completed consensus validation verdicts.
//!
//! Normative validation policy belongs in substrate-free shared crates. This
//! adapter owns only the Holochain-aware extraction, classification, and error
//! propagation required to execute that policy inside an Integrity zome.

pub mod holon_node;
pub mod holon_node_envelope;
pub mod holon_node_lifecycle;
pub mod smartlink_envelope;
pub mod type_conversions;

pub use holon_node::{
    HolonNode, LOCAL_HOLON_SPACE_DESCRIPTION, LOCAL_HOLON_SPACE_NAME, LOCAL_HOLON_SPACE_PATH,
};
pub use holon_node_envelope::{prepare_holon_node_envelope, HolonNodeEnvelope};
pub use holon_node_lifecycle::{
    validate_holon_node_delete_target, validate_holon_node_update_target,
};
pub use smartlink_envelope::{
    resolve_link_delete_target, validate_smartlink_create, validate_smartlink_delete,
};

pub use type_conversions::*;
