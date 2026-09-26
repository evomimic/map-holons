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
            HolonAction::Read(ReadableHolonAction::GetHolonDescriptor) => "get_holon_descriptor",
            HolonAction::Read(ReadableHolonAction::GetAvailableProperties) => {
                "get_available_properties"
            }
            HolonAction::Read(ReadableHolonAction::GetAvailableRelationships) => {
                "get_available_relationships"
            }
            HolonAction::Read(ReadableHolonAction::GetEffectiveCardinality) => {
                "get_effective_cardinality"
            }
            HolonAction::Read(ReadableHolonAction::GetPropertyIsArray) => "get_property_is_array",
            HolonAction::Read(ReadableHolonAction::GetHasInstanceKey) => "get_has_instance_key",
            HolonAction::Read(ReadableHolonAction::GetRelationshipIsOrdered) => {
                "get_relationship_is_ordered"
            }
            HolonAction::Read(ReadableHolonAction::GetAvailableDances) => "get_available_dances",
            HolonAction::Read(ReadableHolonAction::GetDescribedRelatedHolons { .. }) => {
                "get_described_related_holons"
            }
            HolonAction::Read(ReadableHolonAction::GetInstanceProperties) => {
                "get_instance_properties"
            }
            HolonAction::Read(ReadableHolonAction::GetPropertyValueKind) => {
                "get_property_value_kind"
            }
            HolonAction::Write(_) => "holon_write",
        }
    }
}

/// Non-mutating holon actions.
///
/// Exposes reference reads and bounded descriptor queries through the same ingress.
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
    GetRelatedHolons { name: RelationshipName, hint: holons_core::RelationshipReadHint },

    /// `ReadableHolon::holon_descriptor()` → descriptor reference.
    GetHolonDescriptor,

    /// `ReadableHolon::available_properties()` → ordered descriptor collection.
    GetAvailableProperties,

    /// `ReadableHolon::available_relationships()` → qualified descriptors.
    GetAvailableRelationships,

    /// Inclusive bounds of the target relationship descriptor.
    GetEffectiveCardinality,
    /// Whether the target property descriptor declares an array ValueType.
    GetPropertyIsArray,
    /// Whether the target HolonType defines a non-keyless effective instance key rule.
    GetHasInstanceKey,
    /// Whether the target relationship descriptor declares significant member order.
    GetRelationshipIsOrdered,
    /// Effective afforded Dances of the target holon.
    GetAvailableDances,
    /// Collection membership together with its declared target type.
    GetDescribedRelatedHolons { name: RelationshipName },
    /// Effective properties of the target HolonType, not its metatype.
    GetInstanceProperties,
    /// Declared scalar representation of a property descriptor.
    GetPropertyValueKind,
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
