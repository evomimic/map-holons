//! Deliberately-invalid authoring seams, for integrity tests only (Issue #607).
//!
//! # This is not part of the storage API
//!
//! Everything in this module bypasses `persistence_layer::holon_storage` and authors raw
//! Holochain actions directly. Nothing in production may call it, and it must never grow a
//! caller inside `holons_guest`. It is isolated in its own module so that constraint is visible
//! at the file level rather than buried among the real externs.
//!
//! # Why it has to exist
//!
//! Some integrity rules can only be proven by an operation the storage API refuses to construct.
//! `persist_holon` always addresses an update at its resolved lineage root, so it is structurally
//! incapable of producing the update-target violation that `validate_holon_node_update_target`
//! exists to reject. Without a seam that authors the invalid topology on purpose, that rule is
//! unreachable from a live conductor and its enforcement is asserted only by unit tests of the
//! pure rule — which cannot show that the rule is actually *wired in*.
//!
//! The rejection is the assertion. A probe whose call succeeds has failed its purpose.
//!
//! # Why it is not feature-gated
//!
//! Sweettests run against the DNA produced by `npm run build:happ`, the same artifact the host
//! ships. A feature gate would therefore have to be enabled in that build to be testable, making
//! the gate decorative. The honest alternative is what SL1 also chose for its test externs:
//! always present, and named and documented so its status cannot be mistaken.

use crate::persistence_layer::holon_storage_externs::to_wasm;
use hdk::prelude::*;
use holons_guest_integrity::{type_conversions::*, HolonNode};
use holons_integrity::EntryTypes;
use integrity_core_types::{HolonNodeModel, LocalId};

/// Authors an `Update` against an exact action hash, with no lineage resolution.
///
/// Used by the sweettest that proves integrity rejects an update aimed at another update rather
/// than at a lineage-root `Create`, and by its companion that proves the same seam is *accepted*
/// when aimed at a root — together showing the rejection is about topology, not about this
/// function.
///
/// Not a supported write path.
#[hdk_extern]
pub fn holon_storage_author_update_for_test(
    input: (LocalId, HolonNodeModel),
) -> ExternResult<LocalId> {
    let (target_id, holon_node) = input;
    let target_hash = try_action_hash_from_local_id(&target_id).map_err(to_wasm)?;

    let action_hash =
        update_entry(target_hash, &EntryTypes::HolonNode(HolonNode::from(holon_node)))?;

    Ok(local_id_from_action_hash(action_hash))
}
