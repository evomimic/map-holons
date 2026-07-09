use std::sync::Arc;

use base_types::BaseValue;
use core_types::{PropertyName, RelationshipName};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::reference_layer::HolonReference;

use super::CommandLifecyclePolicy;

/// Holon-scoped domain command.
///
/// Targets a specific holon via a bound runtime reference.
/// The `context` field enables dispatch-level lifecycle enforcement
/// (e.g. mutation entry checks). References are still self-resolving
/// for their own operations.
#[derive(Debug)]
pub struct HolonCommand {
    pub context: Arc<TransactionContext>,
    pub target: HolonReference,
    pub action: HolonAction,
}

/// Domain-level holon actions.
#[derive(Debug)]
pub enum HolonAction {
    Read(ReadableHolonAction),
    Write(WritableHolonAction),
}

impl HolonAction {
    pub fn policy(&self) -> CommandLifecyclePolicy {
        match self {
            HolonAction::Read(ReadableHolonAction::CloneHolon) => {
                CommandLifecyclePolicy::mutating()
            }
            HolonAction::Read(_) => CommandLifecyclePolicy::holon_read_only(),
            HolonAction::Write(_) => CommandLifecyclePolicy::mutating(),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            HolonAction::Read(ReadableHolonAction::CloneHolon) => "clone_holon",
            HolonAction::Read(ReadableHolonAction::Summarize) => "summarize",
            HolonAction::Read(ReadableHolonAction::GetHolonId) => "get_holon_id",
            HolonAction::Read(ReadableHolonAction::GetPredecessor) => "get_predecessor",
            HolonAction::Read(ReadableHolonAction::GetKey) => "get_key",
            HolonAction::Read(ReadableHolonAction::GetVersionedKey) => "get_versioned_key",
            HolonAction::Read(ReadableHolonAction::GetPropertyValue { .. }) => "get_property_value",
            HolonAction::Read(ReadableHolonAction::GetRelatedHolons { .. }) => "get_related_holons",
            HolonAction::Write(_) => "holon_write",
        }
    }
}

/// Non-mutating holon actions.
///
/// Maps 1:1 to the `ReadableHolon` trait methods in
/// `shared_crates/holons_core/src/reference_layer/readable_holon.rs`.
///
/// Lifecycle validated via descriptor. Does not trigger snapshot persistence.
#[derive(Debug)]
pub enum ReadableHolonAction {
    /// `ReadableHolon::clone_holon()` → `TransientReference`
    CloneHolon,

    /// `ReadableHolon::summarize()` → `String`
    Summarize,

    /// `ReadableHolon::holon_id()` → `HolonId`
    GetHolonId,

    /// `ReadableHolon::predecessor()` → `Option<HolonReference>`
    GetPredecessor,

    /// `ReadableHolon::key()` → `Option<MapString>`
    GetKey,

    /// `ReadableHolon::versioned_key()` → `MapString`
    GetVersionedKey,

    /// `ReadableHolon::property_value(name)` → `Option<PropertyValue>`
    GetPropertyValue { name: PropertyName },

    /// `ReadableHolon::related_holons(name)` → `HolonCollection`
    GetRelatedHolons { name: RelationshipName },
}

/// Mutating holon actions.
///
/// Maps 1:1 to the `WritableHolon` trait methods in
/// `shared_crates/holons_core/src/reference_layer/writable_holon.rs`.
///
/// Requires `Open` lifecycle. May require commit guard.
/// May trigger snapshot persistence (descriptor-driven).
#[derive(Debug)]
pub enum WritableHolonAction {
    /// `WritableHolon::with_property_value(name, value)`
    WithPropertyValue { name: PropertyName, value: BaseValue },

    /// `WritableHolon::remove_property_value(name)`
    RemovePropertyValue { name: PropertyName },

    /// `WritableHolon::add_related_holons(name, holons)`
    AddRelatedHolons { name: RelationshipName, holons: Vec<HolonReference> },

    /// `WritableHolon::remove_related_holons(name, holons)`
    RemoveRelatedHolons { name: RelationshipName, holons: Vec<HolonReference> },

    /// `WritableHolon::with_descriptor(descriptor)`
    WithDescriptor { descriptor: HolonReference },
}
