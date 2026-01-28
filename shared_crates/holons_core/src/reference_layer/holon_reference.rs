use serde::{Deserialize, Serialize};
use tracing::info;
use type_names::relationship_names::CoreRelationshipTypeName;

use crate::core_shared_objects::transactions::TransactionContextHandle;
use crate::reference_layer::readable_impl::ReadableHolonImpl;
use crate::reference_layer::writable_impl::WritableHolonImpl;
use crate::{
    core_shared_objects::transactions::TransactionContext,
    core_shared_objects::{
        holon::{holon_utils::EssentialHolonContent, state::AccessType},
        HolonCollection,
    },
    reference_layer::{
        HolonsContextBehavior, ReadableHolon, SmartReference, SmartReferenceSerializable,
        StagedReference, StagedReferenceSerializable, TransientReference,
        TransientReferenceSerializable,
    },
    RelationshipMap,
};
use base_types::{BaseValue, MapString};
use core_types::{
    HolonError, HolonId, HolonNodeModel, PropertyMap, PropertyName, PropertyValue, RelationshipName,
};
use std::sync::{Arc, RwLock};
use type_names::CorePropertyTypeName;

#[derive(Debug, Clone)]
/// HolonReference provides a general way to access Holons without having to know whether they are in a read-only
/// state (and therefore owned by the CacheManager) or being staged for creation/update (and therefore owned by the
/// Nursery).
///
/// HolonReference also hides whether the referenced holon is in the local space or an external space
pub enum HolonReference {
    Transient(TransientReference),
    Staged(StagedReference),
    Smart(SmartReference),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum HolonReferenceSerializable {
    Transient(TransientReferenceSerializable),
    Staged(StagedReferenceSerializable),
    Smart(SmartReferenceSerializable),
}

/// Stages a new Holon by cloning an existing Holon from its HolonReference, without retaining lineage to the Holon its cloned from.
impl HolonReference {
    /// Creates a `HolonReference` wrapping a `SmartReference` for the given `HolonId`.

    #[deprecated(note = "Use `HolonReference::from(staged)` or `staged.into()` instead.")]
    /// Creates a `HolonReference::Staged` variant from a `StagedReference`.
    pub fn from_staged(staged: StagedReference) -> Self {
        HolonReference::Staged(staged)
    }

    #[deprecated(note = "Use `HolonReference::from(smart)` or `smart.into()` instead.")]
    /// Creates a `HolonReference::Smart` variant from a `SmartReference`.
    pub fn from_smart(smart: SmartReference) -> Self {
        HolonReference::Smart(smart)
    }

    /// Creates a tx-bound `HolonReference::Smart` for the given `HolonId`.
    pub fn smart_from_id(
        transaction_handle: TransactionContextHandle,
        holon_id: HolonId,
    ) -> HolonReference {
        HolonReference::Smart(SmartReference::new_from_id(transaction_handle, holon_id))
    }

    /// Binds a wire reference enum to a TransactionContext, validating tx_id.
    pub fn bind(
        wire: HolonReferenceSerializable,
        context: Arc<TransactionContext>,
    ) -> Result<Self, HolonError> {
        match wire {
            HolonReferenceSerializable::Transient(transient) => {
                TransientReference::bind(transient, Arc::clone(&context))
                    .map(HolonReference::Transient)
            }
            HolonReferenceSerializable::Staged(staged) => {
                StagedReference::bind(staged, Arc::clone(&context)).map(HolonReference::Staged)
            }
            HolonReferenceSerializable::Smart(smart) => {
                SmartReference::bind(smart, Arc::clone(&context)).map(HolonReference::Smart)
            }
        }
    }

    pub fn get_descriptor(&self) -> Result<Option<HolonReference>, HolonError> {
        self.is_accessible(AccessType::Read)?;

        // Locally Scoped Helper: extract a single DESCRIBED_BY reference (cardinality <= 1)
        fn from_collection_arc(
            collection_arc: Arc<RwLock<HolonCollection>>,
        ) -> Result<Option<HolonReference>, HolonError> {
            let collection = collection_arc.read().map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on holon collection: {}",
                    e
                ))
            })?;
            collection.is_accessible(AccessType::Read)?;
            let members = collection.get_members();

            if members.len() > 1 {
                return Err(HolonError::Misc(format!(
                    "related_holons for DESCRIBED_BY returned multiple members: {:#?}",
                    members
                )));
            }

            Ok(members.get(0).cloned())
        }

        match self {
            HolonReference::Transient(transient_reference) => {
                let collection_arc =
                    transient_reference.related_holons(CoreRelationshipTypeName::DescribedBy)?;
                from_collection_arc(collection_arc)
            }
            HolonReference::Staged(staged_reference) => {
                let collection_arc =
                    staged_reference.related_holons(CoreRelationshipTypeName::DescribedBy)?;
                from_collection_arc(collection_arc)
            }
            HolonReference::Smart(smart_reference) => {
                let collection_arc =
                    smart_reference.related_holons(CoreRelationshipTypeName::DescribedBy)?;
                from_collection_arc(collection_arc)
            }
        }
    }

    pub fn is_transient(&self) -> bool {
        match self {
            Self::Transient(_) => true,
            _ => false,
        }
    }

    pub fn is_staged(&self) -> bool {
        match self {
            Self::Staged(_) => true,
            _ => false,
        }
    }

    pub fn is_saved(&self) -> bool {
        match self {
            Self::Smart(_) => true,
            _ => false,
        }
    }

    pub fn predecessor(&self) -> Result<Option<HolonReference>, HolonError> {
        self.is_accessible(AccessType::Read)?;
        match self {
            HolonReference::Transient(transient_reference) => transient_reference.predecessor(),
            HolonReference::Staged(staged_reference) => staged_reference.predecessor(),
            HolonReference::Smart(smart_reference) => smart_reference.predecessor(),
        }
    }
    /// Constructs a `HolonReference::Smart` for a holon that has been
    /// successfully committed to persistent storage, embedding the holon's
    /// logical key directly into the reference.
    ///
    /// This helper is intended for situations where the holon’s key is already known and should be cached
    /// locally inside the `SmartReference`.
    ///
    /// # Parameters
    /// - `holon_id`: The committed holon's persistent `HolonId`.
    /// - `key`: The holon's key as a `MapString`, which will be wrapped
    ///          as a `BaseValue::StringValue` and inserted into the
    ///          `smart_property_values` map under the `Key` property name.
    ///
    /// # Returns
    /// A fully-initialized `HolonReference::Smart`, containing both the
    /// holon’s ID and its cached key.
    ///
    /// # Notes
    /// - This method is a convenience wrapper around
    ///   [`smart_with_properties`](Self::smart_with_properties).
    /// - The resulting reference can resolve the holon’s key locally,
    ///   without any additional guest-side interaction.
    /// - Other properties may be added later through the generic
    ///   `smart_with_properties` constructor.
    pub fn smart_with_key(
        transaction_handle: TransactionContextHandle,
        holon_id: HolonId,
        key: MapString,
    ) -> Self {
        let mut smart_properties = PropertyMap::new();
        smart_properties
            .insert(CorePropertyTypeName::Key.as_property_name(), BaseValue::StringValue(key));
        Self::smart_with_properties(transaction_handle, holon_id, smart_properties)
    }

    /// Constructs a `HolonReference::Smart` with an explicit set of cached
    /// smart properties.
    ///
    /// This is the most general constructor for creating a `SmartReference`.
    /// It embeds a committed holon's `HolonId` together with a caller-supplied
    /// `PropertyMap` that caches selected property values locally. These cached
    /// properties allow clients to resolve frequently-needed values—such as
    /// keys, names, or other scalar attributes—without requiring a fetch of the referenced holon
    ///
    /// # Parameters
    /// - `holon_id`: The persistent `HolonId` of the holon being referenced.
    /// - `smart_properties`: A `PropertyMap` of pre-cached values. Keys are
    ///   `PropertyName`s, and values are runtime `PropertyValue`s
    ///   (`BaseValue`).
    ///
    /// # Returns
    /// A `HolonReference::Smart` variant whose internal `SmartReference`
    /// stores both the `holon_id` and the provided smart property map.
    ///
    /// # Use Cases
    /// - Returning committed holon references from the `commit()` dance with
    ///   keys or other identifiers pre-cached.
    /// - Constructing references during loader or migration operations.
    /// - Optimizing client behavior by eliminating unnecessary context-level
    ///   property resolution steps.
    ///
    /// # Notes
    /// - This constructor does not perform validation of property names.
    /// - If you only need to embed the holon's key, prefer
    ///   [`smart_with_key`](Self::smart_with_key) for convenience.
    pub fn smart_with_properties(
        transaction_handle: TransactionContextHandle,
        holon_id: HolonId,
        smart_properties: PropertyMap,
    ) -> Self {
        HolonReference::Smart(SmartReference::new_with_properties(
            transaction_handle,
            holon_id,
            smart_properties,
        ))
    }

    // Simple string representations for errors/logging

    pub fn reference_kind_string(&self) -> String {
        match self {
            HolonReference::Transient(reference) => reference.reference_kind_string(),
            HolonReference::Staged(reference) => reference.reference_kind_string(),
            HolonReference::Smart(reference) => reference.reference_kind_string(),
        }
    }

    pub fn reference_id_string(&self) -> String {
        match self {
            HolonReference::Transient(reference) => reference.reference_id_string(),
            HolonReference::Staged(reference) => reference.reference_id_string(),
            HolonReference::Smart(reference) => reference.reference_id_string(),
        }
    }
}

impl From<&HolonReference> for HolonReferenceSerializable {
    fn from(reference: &HolonReference) -> Self {
        match reference {
            HolonReference::Transient(transient) => HolonReferenceSerializable::Transient(
                TransientReferenceSerializable::from(transient),
            ),
            HolonReference::Staged(staged) => {
                HolonReferenceSerializable::Staged(StagedReferenceSerializable::from(staged))
            }
            HolonReference::Smart(smart) => {
                HolonReferenceSerializable::Smart(SmartReferenceSerializable::from(smart))
            }
        }
    }
}

impl From<StagedReference> for HolonReference {
    fn from(staged: StagedReference) -> Self {
        HolonReference::Staged(staged)
    }
}

impl From<&StagedReference> for HolonReference {
    fn from(staged: &StagedReference) -> Self {
        HolonReference::Staged(staged.clone())
    }
}

impl From<SmartReference> for HolonReference {
    fn from(smart: SmartReference) -> Self {
        HolonReference::Smart(smart)
    }
}

impl From<&SmartReference> for HolonReference {
    fn from(smart: &SmartReference) -> Self {
        HolonReference::Smart(smart.clone())
    }
}

impl From<TransientReference> for HolonReference {
    fn from(transient: TransientReference) -> Self {
        HolonReference::Transient(transient)
    }
}

impl From<&TransientReference> for HolonReference {
    fn from(transient: &TransientReference) -> Self {
        HolonReference::Transient(transient.clone())
    }
}

impl From<HolonReference> for HolonReferenceSerializable {
    fn from(reference: HolonReference) -> Self {
        HolonReferenceSerializable::from(&reference)
    }
}

impl ReadableHolonImpl for HolonReference {
    fn clone_holon_impl(&self) -> Result<TransientReference, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.clone_holon_impl()
            }
            HolonReference::Staged(staged_reference) => staged_reference.clone_holon_impl(),
            HolonReference::Smart(smart_reference) => smart_reference.clone_holon_impl(),
        }
    }

    fn all_related_holons_impl(&self) -> Result<RelationshipMap, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.all_related_holons_impl()
            }
            HolonReference::Staged(staged_reference) => staged_reference.all_related_holons_impl(),
            HolonReference::Smart(smart_reference) => smart_reference.all_related_holons_impl(),
        }
    }

    fn holon_id_impl(&self) -> Result<HolonId, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => transient_reference.holon_id_impl(),
            HolonReference::Staged(staged_reference) => staged_reference.holon_id_impl(),
            HolonReference::Smart(smart_reference) => smart_reference.holon_id_impl(),
        }
    }

    fn predecessor_impl(&self) -> Result<Option<HolonReference>, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.predecessor_impl()
            }
            HolonReference::Staged(staged_reference) => staged_reference.predecessor_impl(),
            HolonReference::Smart(smart_reference) => smart_reference.predecessor_impl(),
        }
    }

    fn property_value_impl(
        &self,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.property_value_impl(property_name)
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.property_value_impl(property_name)
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.property_value_impl(property_name)
            }
        }
    }

    fn key_impl(&self) -> Result<Option<MapString>, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => transient_reference.key_impl(),
            HolonReference::Staged(staged_reference) => staged_reference.key_impl(),
            HolonReference::Smart(smart_reference) => smart_reference.key_impl(),
        }
    }

    fn related_holons_impl(
        &self,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.related_holons_impl(relationship_name)
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.related_holons_impl(relationship_name)
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.related_holons_impl(relationship_name)
            }
        }
    }

    fn versioned_key_impl(&self) -> Result<MapString, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.versioned_key_impl()
            }
            HolonReference::Staged(staged_reference) => staged_reference.versioned_key_impl(),
            HolonReference::Smart(smart_reference) => smart_reference.versioned_key_impl(),
        }
    }

    fn essential_content_impl(&self) -> Result<EssentialHolonContent, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.essential_content_impl()
            }
            HolonReference::Staged(staged_reference) => staged_reference.essential_content_impl(),
            HolonReference::Smart(smart_reference) => smart_reference.essential_content_impl(),
        }
    }

    fn summarize_impl(&self) -> Result<String, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => transient_reference.summarize_impl(),
            HolonReference::Staged(staged_reference) => staged_reference.summarize_impl(),
            HolonReference::Smart(smart_reference) => smart_reference.summarize_impl(),
        }
    }

    fn into_model_impl(&self) -> Result<HolonNodeModel, HolonError> {
        match self {
            Self::Transient(reference) => reference.into_model_impl(),
            Self::Staged(reference) => reference.into_model_impl(),
            Self::Smart(reference) => reference.into_model_impl(),
        }
    }

    fn is_accessible_impl(&self, access_type: AccessType) -> Result<(), HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.is_accessible_impl(access_type)
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.is_accessible_impl(access_type)
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.is_accessible_impl(access_type)
            }
        }
    }
}

impl WritableHolonImpl for HolonReference {
    fn add_related_holons_impl(
        &mut self,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.add_related_holons_impl(relationship_name, holons)?;
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.add_related_holons_impl(relationship_name, holons)?;
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.add_related_holons_impl(relationship_name, holons)?;
            }
        }

        Ok(self)
    }

    fn remove_related_holons_impl(
        &mut self,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.remove_related_holons_impl(relationship_name, holons)?;
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.remove_related_holons_impl(relationship_name, holons)?;
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.remove_related_holons_impl(relationship_name, holons)?;
            }
        }

        Ok(self)
    }

    fn with_property_value_impl(
        &mut self,
        property: PropertyName,
        value: BaseValue,
    ) -> Result<&mut Self, HolonError> {
        info!("Entered HolonReference::with_property_value_impl");
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.with_property_value_impl(property, value)?;
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.with_property_value_impl(property, value)?;
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.with_property_value_impl(property, value)?;
            }
        }

        Ok(self)
    }

    fn remove_property_value_impl(&mut self, name: PropertyName) -> Result<&mut Self, HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.remove_property_value_impl(name)?;
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.remove_property_value_impl(name)?;
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.remove_property_value_impl(name)?;
            }
        }
        Ok(self)
    }

    fn with_descriptor_impl(
        &mut self,
        descriptor_reference: HolonReference,
    ) -> Result<(), HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.with_descriptor_impl(descriptor_reference)
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.with_descriptor_impl(descriptor_reference)
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.with_descriptor_impl(descriptor_reference)
            }
        }
    }

    fn with_predecessor_impl(
        &mut self,
        predecessor_reference_option: Option<HolonReference>,
    ) -> Result<(), HolonError> {
        match self {
            HolonReference::Transient(transient_reference) => {
                transient_reference.with_predecessor_impl(predecessor_reference_option)
            }
            HolonReference::Staged(staged_reference) => {
                staged_reference.with_predecessor_impl(predecessor_reference_option)
            }
            HolonReference::Smart(smart_reference) => {
                smart_reference.with_predecessor_impl(predecessor_reference_option)
            }
        }
    }
}
