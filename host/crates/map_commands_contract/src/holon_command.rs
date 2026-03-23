use std::sync::Arc;

use base_types::BaseValue;
use core_types::{PropertyName, RelationshipName};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::reference_layer::HolonReference;

use super::CommandDescriptor;

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
    pub fn descriptor(&self) -> CommandDescriptor {
        match self {
            HolonAction::Read(ReadableHolonAction::CloneHolon) => {
                CommandDescriptor::mutating()
            }
            HolonAction::Read(_) => CommandDescriptor::holon_read_only(),
            HolonAction::Write(_) => CommandDescriptor::mutating(),
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

    /// `ReadableHolon::essential_content()` → `EssentialHolonContent`
    EssentialContent,

    /// `ReadableHolon::summarize()` → `String`
    Summarize,

    /// `ReadableHolon::holon_id()` → `HolonId`
    HolonId,

    /// `ReadableHolon::predecessor()` → `Option<HolonReference>`
    Predecessor,

    /// `ReadableHolon::key()` → `Option<MapString>`
    Key,

    /// `ReadableHolon::versioned_key()` → `MapString`
    VersionedKey,

    /// `ReadableHolon::property_value(name)` → `Option<PropertyValue>`
    PropertyValue { name: PropertyName },

    /// `ReadableHolon::related_holons(name)` → `HolonCollection`
    RelatedHolons { name: RelationshipName },
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

    /// `WritableHolon::with_predecessor(predecessor)`
    WithPredecessor { predecessor: Option<HolonReference> },
}
