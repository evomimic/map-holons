use derive_new::new;
use std::sync::RwLock;
use std::{fmt, sync::Arc};
use tracing::info;
use type_names::relationship_names::CoreRelationshipTypeName;

use crate::core_shared_objects::holon::StagedState;
use crate::core_shared_objects::transactions::{
    TransactionContext, TransactionContextHandle, TxId,
};
use crate::reference_layer::readable_impl::ReadableHolonImpl;
use crate::reference_layer::writable_impl::WritableHolonImpl;
use crate::{
    core_shared_objects::holon::HolonCloneModel,
    reference_layer::{HolonReference, ReadableHolon, TransientReference},
};
// Provides methods for creating/transient holons
use crate::{
    core_shared_objects::{
        holon::{holon_utils::EssentialHolonContent, state::AccessType},
        transient_holon_manager::ToHolonCloneModel,
        Holon, HolonCollection, ReadableHolonState, WriteableHolonState,
    },
    RelationshipMap,
};
use base_types::{BaseValue, MapString};
use core_types::{
    HolonError, HolonId, HolonNodeModel, PropertyName, PropertyValue, RelationshipName, TemporaryId,
};

#[derive(new, Debug, Clone)]
pub struct StagedReference {
    context_handle: TransactionContextHandle,
    id: TemporaryId, // the position of the holon with CommitManager's staged_holons vector
}

/// Capability token allowing nursery access only from this module.
pub(crate) struct StagedRefAccessKey(());

impl StagedRefAccessKey {
    fn new() -> Self {
        Self(())
    }
}

impl StagedReference {
    /// Marks the underlying StagedHolon that is referenced as 'Abandoned'
    ///
    /// Prevents a commit from taking place and restricts Holon to read-only access.
    ///
    /// # Arguments
    /// * `context` - A reference to a `TransactionContext`.
    pub fn abandon_staged_changes(
        &self,
        _context: &Arc<TransactionContext>,
    ) -> Result<(), HolonError> {
        // Get access to the source holon
        let rc_holon = self.get_rc_holon()?;

        // Borrow the holon mutably
        let mut holon_mut = rc_holon.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!("Failed to acquire write lock on holon: {}", e))
        })?;

        match &mut *holon_mut {
            Holon::Staged(staged_holon) => {
                staged_holon.abandon_staged_changes()?;
            }
            _ => {
                unreachable!()
            }
        }

        Ok(())
    }

    /// Creates a new `StagedReference` from a given TemporaryId without validation.
    ///
    /// # Arguments
    /// * `id` - A TemporaryId
    ///
    /// # Returns
    /// A new `StagedReference` wrapping the provided id.
    pub fn from_temporary_id(context_handle: TransactionContextHandle, id: &TemporaryId) -> Self {
        StagedReference { context_handle, id: id.clone() }
    }

    /// Retrieves the underlying Holon handle for commit operations.
    ///
    /// This method simply delegates to the internal `get_rc_holon()` and exists
    /// so that guest commit functions can obtain the referenced holon for
    /// persistence.  It will later be restricted to the `guest` feature.
    pub fn get_holon_to_commit(
        &self,
        _context: &Arc<TransactionContext>,
    ) -> Result<Arc<RwLock<Holon>>, HolonError> {
        self.get_rc_holon()
    }

    /// Retrieves a shared reference to the holon with interior mutability.
    ///
    /// # Arguments
    /// * `context` - A reference to a `TransactionContext`.
    ///
    /// # Returns
    ///  Result<Arc<RwLock<Holon>>, HolonError>
    ///
    fn get_rc_holon(&self) -> Result<Arc<RwLock<Holon>>, HolonError> {
        // Get NurseryAccess
        let nursery_access =
            self.context_handle.context().nursery_access(StagedRefAccessKey::new());

        // Retrieve the holon by its temporaryId
        let rc_holon = nursery_access.get_holon_by_id(&self.id)?;

        Ok(rc_holon)
    }

    pub fn is_in_state(
        &self,
        _context: &Arc<TransactionContext>,
        check_state: StagedState,
    ) -> Result<bool, HolonError> {
        let rc_holon = self.get_rc_holon()?;

        let holon = rc_holon
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on nursery: {}",
                    e
                ))
            })?
            .clone();
        let current_state = match holon {
            Holon::Staged(holon) => Ok(holon.get_staged_state()),
            _ => Err(HolonError::InvalidType(
                "StagedReference should point to a StagedHolon".to_string(),
            )),
        }?;
        if current_state == check_state {
            return Ok(true);
        } else {
            Ok(false)
        }
    }

    /// Returns whether this staged holon has been committed.
    ///
    /// Unlike [`Self::is_in_state`], this matches any `StagedState::Committed`
    /// without requiring the exact committed `LocalId`.
    pub fn is_committed(&self) -> Result<bool, HolonError> {
        let rc_holon = self.get_rc_holon()?;

        let holon = rc_holon
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on nursery: {}",
                    e
                ))
            })?
            .clone();
        match holon {
            Holon::Staged(holon) => {
                Ok(matches!(holon.get_staged_state(), StagedState::Committed(_)))
            }
            _ => Err(HolonError::InvalidType(
                "StagedReference should point to a StagedHolon".to_string(),
            )),
        }
    }

    // ****** Accessors ******

    /// Returns the temporary id for this staged holon.
    pub fn temporary_id(&self) -> TemporaryId {
        self.id.clone()
    }

    /// Returns the transaction id this reference is bound to.
    pub fn tx_id(&self) -> TxId {
        self.context_handle.tx_id()
    }

    // Simple string representations for errors/logging
    pub fn reference_kind_string(&self) -> String {
        "StagedReference".to_string()
    }

    pub fn reference_id_string(&self) -> String {
        format!("TemporaryId={}", self.id)
    }

    fn classify_relationship_mutation(
        &self,
        relationship_name: &RelationshipName,
    ) -> Result<Option<bool>, HolonError> {
        let rc_holon = self.get_rc_holon()?;
        let staged_state = {
            let holon = rc_holon.read().map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on staged holon: {}",
                    e
                ))
            })?;

            match &*holon {
                Holon::Staged(staged_holon) => staged_holon.get_staged_state(),
                _ => {
                    return Err(HolonError::InvalidType(
                        "StagedReference should point to a StagedHolon".to_string(),
                    ))
                }
            }
        };

        if staged_state == StagedState::ForCreate {
            return Ok(None);
        }

        let source_ref = HolonReference::Staged(self.clone());
        let relationship_descriptor =
            source_ref.holon_descriptor()?.get_relationship_by_name(relationship_name.clone())?;

        Ok(Some(relationship_descriptor.is_definitional()?))
    }

    fn related_holons_with_keys(
        holons: Vec<HolonReference>,
    ) -> Result<Vec<(HolonReference, Option<MapString>)>, HolonError> {
        holons
            .into_iter()
            .map(|h| {
                let key = h.key()?;
                Ok((h, key))
            })
            .collect()
    }

    fn add_related_holons_with_classification(
        &self,
        relationship_name: RelationshipName,
        entries: Vec<(HolonReference, Option<MapString>)>,
        is_definitional: Option<bool>,
    ) -> Result<(), HolonError> {
        let rc_holon = self.get_rc_holon()?;
        let mut holon_mut = rc_holon.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on staged holon: {}",
                e
            ))
        })?;

        match &mut *holon_mut {
            Holon::Staged(staged_holon) => {
                staged_holon.add_related_holons_with_keys(relationship_name, entries)?;
                if let Some(is_definitional) = is_definitional {
                    staged_holon.note_relationship_mutation(is_definitional)?;
                }
            }
            _ => {
                return Err(HolonError::InvalidType(
                    "StagedReference should point to a StagedHolon".to_string(),
                ))
            }
        }

        Ok(())
    }

    fn remove_related_holons_with_classification(
        &self,
        relationship_name: &RelationshipName,
        entries: Vec<(HolonReference, Option<MapString>)>,
        is_definitional: Option<bool>,
    ) -> Result<(), HolonError> {
        let rc_holon = self.get_rc_holon()?;
        let mut holon_mut = rc_holon.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on staged holon: {}",
                e
            ))
        })?;

        match &mut *holon_mut {
            Holon::Staged(staged_holon) => {
                staged_holon.remove_related_holons_with_keys(relationship_name, entries)?;
                if let Some(is_definitional) = is_definitional {
                    staged_holon.note_relationship_mutation(is_definitional)?;
                }
            }
            _ => {
                return Err(HolonError::InvalidType(
                    "StagedReference should point to a StagedHolon".to_string(),
                ))
            }
        }

        Ok(())
    }

    fn add_related_holons_without_classification(
        &self,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        let holons_with_keys = Self::related_holons_with_keys(holons)?;
        self.add_related_holons_with_classification(relationship_name, holons_with_keys, None)
    }

    fn remove_related_holons_without_classification(
        &self,
        relationship_name: &RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        let holons_with_keys = Self::related_holons_with_keys(holons)?;
        self.remove_related_holons_with_classification(relationship_name, holons_with_keys, None)
    }
}

impl fmt::Display for StagedReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StagedReference({})", self.reference_id_string())
    }
}

impl ReadableHolonImpl for StagedReference {
    fn clone_holon_impl(&self) -> Result<TransientReference, HolonError> {
        self.is_accessible(AccessType::Clone)?;
        let rc_holon = self.get_rc_holon()?;
        let holon_clone_model = rc_holon
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on staged holon: {}",
                    e
                ))
            })?
            .holon_clone_model();

        let cloned_holon_transient_reference =
            self.context_handle.context().new_transient_from_clone_model(holon_clone_model)?;

        Ok(cloned_holon_transient_reference)
    }

    fn all_related_holons_impl(&self) -> Result<RelationshipMap, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;

        borrowed_holon.all_related_holons()
    }

    fn holon_id_impl(&self) -> Result<HolonId, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;

        borrowed_holon.holon_id()
    }

    fn predecessor_impl(&self) -> Result<Option<HolonReference>, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let collection_arc = self.related_holons(CoreRelationshipTypeName::Predecessor)?;
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
                "related_holons for PREDECESSOR returned multiple members: {:#?}",
                members
            )));
        }
        if members.is_empty() {
            Ok(None)
        } else {
            Ok(Some(members[0].clone()))
        }
    }

    fn property_value_impl(
        &self,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;

        borrowed_holon.property_value(property_name)
    }

    fn key_impl(&self) -> Result<Option<MapString>, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;

        Ok(borrowed_holon.key())
    }

    fn related_holons_impl(
        &self,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;

        holon.related_holons(relationship_name)
    }

    fn versioned_key_impl(&self) -> Result<MapString, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let holon = self.get_rc_holon()?;
        let key = holon
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on staged holon: {}",
                    e
                ))
            })?
            .versioned_key()?;

        Ok(key)
    }

    fn essential_content_impl(&self) -> Result<EssentialHolonContent, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;

        Ok(borrowed_holon.essential_content())
    }

    fn summarize_impl(&self) -> Result<String, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;
        Ok(borrowed_holon.summarize())
    }

    fn into_model_impl(&self) -> Result<HolonNodeModel, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;

        Ok(borrowed_holon.into_node_model())
    }

    fn is_accessible_impl(&self, access_type: AccessType) -> Result<(), HolonError> {
        let rc_holon = self.get_rc_holon()?;
        let holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;
        holon.is_accessible(access_type)?;

        Ok(())
    }

    fn is_committed_source_impl(&self) -> Result<bool, HolonError> {
        self.is_committed()
    }

    fn holon_reference_impl(&self) -> HolonReference {
        self.into()
    }
}

impl WritableHolonImpl for StagedReference {
    fn add_related_holons_impl(
        &mut self,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;
        let is_definitional = self.classify_relationship_mutation(&relationship_name)?;
        // Precompute keys before taking the holon write lock to avoid re-entrant locking on self-edges.
        let holons_with_keys = Self::related_holons_with_keys(holons)?;

        self.add_related_holons_with_classification(
            relationship_name,
            holons_with_keys,
            is_definitional,
        )?;

        Ok(self)
    }

    fn remove_related_holons_impl(
        &mut self,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;
        let is_definitional = self.classify_relationship_mutation(&relationship_name)?;
        let holons_with_keys = Self::related_holons_with_keys(holons)?;
        info!(
            "Removing {:?} related holons from relationship: {:?}",
            holons_with_keys.len(),
            relationship_name
        );
        self.remove_related_holons_with_classification(
            &relationship_name,
            holons_with_keys,
            is_definitional,
        )?;

        Ok(self)
    }

    fn with_property_value_impl(
        &mut self,
        property: PropertyName,
        value: BaseValue,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;
        let rc_holon = self.get_rc_holon()?;
        let mut holon_mut = rc_holon.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on staged holon: {}",
                e
            ))
        })?;

        holon_mut.with_property_value(property, value)?;

        Ok(self)
    }

    fn remove_property_value_impl(&mut self, name: PropertyName) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;
        let rc_holon = self.get_rc_holon()?;
        let mut holon_mut = rc_holon.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on staged holon: {}",
                e
            ))
        })?;
        holon_mut.remove_property_value(&name)?;

        Ok(self)
    }

    fn with_descriptor_impl(
        &mut self,
        descriptor_reference: HolonReference,
    ) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;

        // Look up the existing descriptor(s) on THIS holon, not on the descriptor.
        let self_ref = HolonReference::Staged(self.clone());
        let existing_descriptor_option = self_ref.get_descriptor()?;

        if let Some(existing_descriptor) = existing_descriptor_option {
            // Remove the current descriptor edge
            self.remove_related_holons_without_classification(
                &CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
                vec![existing_descriptor.clone()],
            )?;
        }

        // Attach the new descriptor edge
        self.add_related_holons_without_classification(
            CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
            vec![descriptor_reference],
        )?;

        Ok(())
    }

    fn with_predecessor_impl(
        &mut self,
        predecessor_reference_option: Option<HolonReference>, // None passed just removes predecessor
    ) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;
        let existing_predecessor_option = self.clone().predecessor()?;
        if let Some(predecessor) = existing_predecessor_option {
            self.remove_related_holons_without_classification(
                &CoreRelationshipTypeName::Predecessor.as_relationship_name(),
                vec![predecessor.clone()],
            )?;
        }
        if let Some(predecessor_reference) = predecessor_reference_option {
            self.add_related_holons_without_classification(
                CoreRelationshipTypeName::Predecessor.as_relationship_name(),
                vec![predecessor_reference.clone()],
            )?;
        }

        Ok(())
    }
}

impl ToHolonCloneModel for StagedReference {
    fn holon_clone_model(&self) -> Result<HolonCloneModel, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let holon_clone_model = rc_holon
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on staged holon: {}",
                    e
                ))
            })?
            .holon_clone_model();

        Ok(holon_clone_model)
    }
}

// ---------- StagedReference equality ----------
//
// Staged holons are transaction-scoped, so the combination of (tx_id, TemporaryId)
// is the stable identity.
impl PartialEq for StagedReference {
    fn eq(&self, other: &Self) -> bool {
        self.context_handle.tx_id() == other.context_handle.tx_id() && self.id == other.id
    }
}

impl Eq for StagedReference {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core_shared_objects::StagedHolon,
        descriptors::test_support::{
            build_context, new_descriptor_holon, new_holon_type_descriptor,
            new_relationship_descriptor_holon, new_test_holon,
        },
        reference_layer::WritableHolon,
    };
    use core_types::LocalId;
    use type_names::CorePropertyTypeName;

    fn force_staged_reference_for_update(
        context: &Arc<TransactionContext>,
        staged_reference: &StagedReference,
    ) -> Result<(), HolonError> {
        let rc_holon = staged_reference.get_holon_to_commit(context)?;
        let clone_model = {
            let holon = rc_holon.read().map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on staged holon: {}",
                    e
                ))
            })?;
            holon.holon_clone_model()
        };
        let update_holon =
            StagedHolon::new_for_update_from_clone_model(clone_model, LocalId(vec![1, 2, 3]))?;

        let mut holon = rc_holon.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on staged holon: {}",
                e
            ))
        })?;
        *holon = Holon::Staged(update_holon);

        Ok(())
    }

    fn staged_relationship_descriptor(
        context: &Arc<TransactionContext>,
        relationship_name: &str,
        is_definitional: Option<bool>,
    ) -> Result<(StagedReference, StagedReference), HolonError> {
        let source_type = new_holon_type_descriptor(context, "source-type", "SourceType")?;
        let target_type = new_holon_type_descriptor(context, "target-type", "TargetType")?;
        let staged_source_type = context.mutation().stage_new_holon(source_type)?;
        let staged_target_type = context.mutation().stage_new_holon(target_type)?;

        let relationship_descriptor = if let Some(is_definitional) = is_definitional {
            let mut relationship_descriptor = new_relationship_descriptor_holon(
                context,
                "relationship-descriptor",
                relationship_name,
                staged_source_type.clone().into(),
                staged_target_type.clone().into(),
            )?;
            relationship_descriptor
                .with_property_value(CorePropertyTypeName::IsDefinitional, is_definitional)?;
            relationship_descriptor
        } else {
            new_descriptor_holon(
                context,
                "relationship-descriptor",
                relationship_name,
                "Relationship",
            )?
        };
        let staged_relationship_descriptor =
            context.mutation().stage_new_holon(relationship_descriptor)?;

        let mut staged_source_type = staged_source_type;
        staged_source_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![staged_relationship_descriptor.into()],
        )?;

        Ok((staged_source_type, staged_target_type))
    }

    fn staged_relationship_descriptor_with_invalid_is_definitional(
        context: &Arc<TransactionContext>,
        relationship_name: &str,
    ) -> Result<(StagedReference, StagedReference), HolonError> {
        let source_type = new_holon_type_descriptor(context, "source-type", "SourceType")?;
        let target_type = new_holon_type_descriptor(context, "target-type", "TargetType")?;
        let staged_source_type = context.mutation().stage_new_holon(source_type)?;
        let staged_target_type = context.mutation().stage_new_holon(target_type)?;

        let mut relationship_descriptor = new_relationship_descriptor_holon(
            context,
            "relationship-descriptor",
            relationship_name,
            staged_source_type.clone().into(),
            staged_target_type.clone().into(),
        )?;
        relationship_descriptor
            .with_property_value(CorePropertyTypeName::IsDefinitional, "not-a-boolean")?;
        let staged_relationship_descriptor =
            context.mutation().stage_new_holon(relationship_descriptor)?;

        let mut staged_source_type = staged_source_type;
        staged_source_type.add_related_holons(
            CoreRelationshipTypeName::InstanceRelationships,
            vec![staged_relationship_descriptor.into()],
        )?;

        Ok((staged_source_type, staged_target_type))
    }

    fn staged_update_source(
        context: &Arc<TransactionContext>,
        source_descriptor: StagedReference,
    ) -> Result<StagedReference, HolonError> {
        let source = new_test_holon(context, "source-instance")?;
        let mut staged_source = context.mutation().stage_new_holon(source)?;
        staged_source.add_related_holons(
            CoreRelationshipTypeName::DescribedBy,
            vec![source_descriptor.into()],
        )?;
        force_staged_reference_for_update(context, &staged_source)?;
        Ok(staged_source)
    }

    fn staged_target(
        context: &Arc<TransactionContext>,
        key: &str,
    ) -> Result<StagedReference, HolonError> {
        context.mutation().stage_new_holon(new_test_holon(context, key)?)
    }

    #[test]
    fn is_committed_matches_any_committed_local_id() -> Result<(), HolonError> {
        let context = build_context();
        let staged = context.mutation().stage_new_holon(new_test_holon(&context, "commit-me")?)?;

        assert!(!staged.is_committed()?);

        let rc_holon = staged.get_holon_to_commit(&context)?;
        {
            let mut holon = rc_holon.write().map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire write lock on staged holon: {}",
                    e
                ))
            })?;
            match &mut *holon {
                Holon::Staged(staged_holon) => staged_holon.to_committed(LocalId(vec![4, 5, 6]))?,
                _ => unreachable!("stage_new_holon produces a StagedHolon"),
            }
        }

        assert!(staged.is_committed()?);
        assert!(staged.is_in_state(&context, StagedState::Committed(LocalId(vec![4, 5, 6])))?);

        Ok(())
    }

    #[test]
    fn for_create_relationship_mutation_skips_descriptor_classification() -> Result<(), HolonError>
    {
        let context = build_context();
        let source = new_test_holon(&context, "new-source")?;
        let mut staged_source = context.mutation().stage_new_holon(source)?;
        let target = staged_target(&context, "new-target")?;

        staged_source.add_related_holons("UndeclaredRelationship", vec![target.into()])?;

        assert!(staged_source.is_in_state(&context, StagedState::ForCreate)?);
        Ok(())
    }

    #[test]
    fn non_definitional_relationship_mutation_sets_graph_only() -> Result<(), HolonError> {
        let context = build_context();
        let (source_descriptor, _) =
            staged_relationship_descriptor(&context, "AuthoredBy", Some(false))?;
        let mut staged_source = staged_update_source(&context, source_descriptor)?;
        let target = staged_target(&context, "author")?;

        staged_source.add_related_holons("AuthoredBy", vec![target.into()])?;

        assert!(staged_source.is_in_state(&context, StagedState::ForUpdateGraphOnly)?);
        Ok(())
    }

    #[test]
    fn definitional_relationship_mutation_sets_new_version() -> Result<(), HolonError> {
        let context = build_context();
        let (source_descriptor, _) =
            staged_relationship_descriptor(&context, "AuthoredBy", Some(true))?;
        let mut staged_source = staged_update_source(&context, source_descriptor)?;
        let target = staged_target(&context, "author")?;

        staged_source.add_related_holons("AuthoredBy", vec![target.into()])?;

        assert!(staged_source.is_in_state(&context, StagedState::ForUpdateNewVersion)?);
        Ok(())
    }

    #[test]
    fn relationship_removal_uses_definitional_classification() -> Result<(), HolonError> {
        let context = build_context();
        let (source_descriptor, _) =
            staged_relationship_descriptor(&context, "AuthoredBy", Some(false))?;
        let mut staged_source = staged_update_source(&context, source_descriptor)?;
        let target = staged_target(&context, "author")?;
        staged_source.add_related_holons("AuthoredBy", vec![target.clone().into()])?;
        force_staged_reference_for_update(&context, &staged_source)?;

        staged_source.remove_related_holons("AuthoredBy", vec![target.into()])?;

        assert!(staged_source.is_in_state(&context, StagedState::ForUpdateGraphOnly)?);
        let collection = staged_source.related_holons("AuthoredBy")?;
        assert!(collection.read().unwrap().get_members().is_empty());
        Ok(())
    }

    #[test]
    fn missing_relationship_descriptor_errors_before_mutation() -> Result<(), HolonError> {
        let context = build_context();
        let (source_descriptor, _) =
            staged_relationship_descriptor(&context, "AuthoredBy", Some(false))?;
        let mut staged_source = staged_update_source(&context, source_descriptor)?;
        let target = staged_target(&context, "author")?;

        let result = staged_source.add_related_holons("MissingRelationship", vec![target.into()]);

        assert!(matches!(
            result,
            Err(HolonError::DescriptorDeclarationNotFound { kind, name, .. })
                if kind == "relationship" && name == "MissingRelationship"
        ));
        assert!(staged_source.is_in_state(&context, StagedState::ForUpdate)?);
        let collection = staged_source.related_holons("MissingRelationship")?;
        assert!(collection.read().unwrap().get_members().is_empty());
        Ok(())
    }

    #[test]
    fn missing_is_definitional_errors_before_mutation() -> Result<(), HolonError> {
        let context = build_context();
        let (source_descriptor, _) = staged_relationship_descriptor(&context, "AuthoredBy", None)?;
        let mut staged_source = staged_update_source(&context, source_descriptor)?;
        let target = staged_target(&context, "author")?;

        let result = staged_source.add_related_holons("AuthoredBy", vec![target.into()]);

        assert!(matches!(result, Err(HolonError::EmptyField(field)) if field == "IsDefinitional"));
        assert!(staged_source.is_in_state(&context, StagedState::ForUpdate)?);
        let collection = staged_source.related_holons("AuthoredBy")?;
        assert!(collection.read().unwrap().get_members().is_empty());
        Ok(())
    }

    #[test]
    fn invalid_is_definitional_errors_before_mutation() -> Result<(), HolonError> {
        let context = build_context();
        let (source_descriptor, _) =
            staged_relationship_descriptor_with_invalid_is_definitional(&context, "AuthoredBy")?;
        let mut staged_source = staged_update_source(&context, source_descriptor)?;
        let target = staged_target(&context, "author")?;

        let result = staged_source.add_related_holons("AuthoredBy", vec![target.into()]);

        assert!(
            matches!(result, Err(HolonError::UnexpectedValueType(_, expected)) if expected == "Boolean")
        );
        assert!(staged_source.is_in_state(&context, StagedState::ForUpdate)?);
        let collection = staged_source.related_holons("AuthoredBy")?;
        assert!(collection.read().unwrap().get_members().is_empty());
        Ok(())
    }
}
