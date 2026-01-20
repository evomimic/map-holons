use derive_new::new;
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use std::{fmt, sync::Arc};
use tracing::info;
use type_names::relationship_names::CoreRelationshipTypeName;

use crate::core_shared_objects::transactions::{TransactionContext, TxId};
use crate::core_shared_objects::holon::StagedState;
use crate::reference_layer::readable_impl::ReadableHolonImpl;
use crate::reference_layer::writable_impl::WritableHolonImpl;
use crate::{
    core_shared_objects::holon::HolonCloneModel,
    reference_layer::{HolonReference, HolonsContextBehavior, ReadableHolon, TransientReference},
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StagedReferenceSerializable {
    tx_id: TxId,
    id: TemporaryId,
}

impl StagedReferenceSerializable {
    pub fn new(tx_id: TxId, id: TemporaryId) -> Self {
        Self { tx_id, id }
    }

    pub fn tx_id(&self) -> TxId {
        self.tx_id
    }
}

#[derive(new, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StagedReference {
    id: TemporaryId, // the position of the holon with CommitManager's staged_holons vector
}

impl StagedReference {
    /// Marks the underlying StagedHolon that is referenced as 'Abandoned'
    ///
    /// Prevents a commit from taking place and restricts Holon to read-only access.
    ///
    /// # Arguments
    /// * `context` - A reference to an object implementing the `HolonsContextBehavior` trait.
    pub fn abandon_staged_changes(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<(), HolonError> {
        // Get access to the source holon
        let rc_holon = self.get_rc_holon(context)?;

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
    pub fn from_temporary_id(id: &TemporaryId) -> Self {
        StagedReference { id: id.clone() }
    }

    pub fn bind(
        wire: StagedReferenceSerializable,
        context: Arc<TransactionContext>,
    ) -> Result<Self, HolonError> {
        if wire.tx_id != context.tx_id() {
            return Err(HolonError::InvalidHolonReference(format!(
                "StagedReference bind failed: wire tx_id {:?} does not match active tx_id {:?}",
                wire.tx_id,
                context.tx_id()
            )));
        }

        Ok(StagedReference { id: wire.id })
    }

    /// Retrieves the underlying Holon handle for commit operations.
    ///
    /// This method simply delegates to the internal `get_rc_holon()` and exists
    /// so that guest commit functions can obtain the referenced holon for
    /// persistence.  It will later be restricted to the `guest` feature.
    pub fn get_holon_to_commit(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Arc<RwLock<Holon>>, HolonError> {
        self.get_rc_holon(context)
    }

    /// Retrieves a shared reference to the holon with interior mutability.
    ///
    /// # Arguments
    /// * `context` - A reference to an object implementing the `HolonsContextBehavior` trait.
    ///
    /// # Returns
    /// Rc<RefCell<Holon>>>
    ///
    fn get_rc_holon(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Arc<RwLock<Holon>>, HolonError> {
        // Get NurseryAccess
        let nursery_access = context.get_nursery_access();

        // Retrieve the holon by its temporaryId
        let rc_holon = nursery_access.get_holon_by_id(&self.id)?;

        Ok(rc_holon)
    }

    pub fn is_in_state(
        &self,
        context: &dyn HolonsContextBehavior,
        check_state: StagedState,
    ) -> Result<bool, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
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

    pub fn temporary_id(&self) -> TemporaryId {
        self.id.clone()
    }
}

impl fmt::Display for StagedReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StagedReference(id: {:?})", self.id)
    }
}

impl ReadableHolonImpl for StagedReference {
    fn clone_holon_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<TransientReference, HolonError> {
        self.is_accessible(context, AccessType::Clone)?;
        let rc_holon = self.get_rc_holon(context)?;
        let holon_clone_model = rc_holon
            .read()
            .map_err(|e| {
                HolonError::FailedToAcquireLock(format!(
                    "Failed to acquire read lock on staged holon: {}",
                    e
                ))
            })?
            .holon_clone_model();

        let transient_behavior = context.get_transient_behavior_service();

        let cloned_holon_transient_reference =
            transient_behavior.new_from_clone_model(holon_clone_model)?;

        Ok(cloned_holon_transient_reference)
    }

    fn all_related_holons_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<RelationshipMap, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;

        borrowed_holon.all_related_holons()
    }

    fn holon_id_impl(&self, context: &dyn HolonsContextBehavior) -> Result<HolonId, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;

        borrowed_holon.holon_id()
    }

    fn predecessor_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<HolonReference>, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let collection_arc = self.related_holons(context, CoreRelationshipTypeName::Predecessor)?;
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
        context: &dyn HolonsContextBehavior,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;

        borrowed_holon.property_value(property_name)
    }

    fn key_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<MapString>, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
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
        context: &dyn HolonsContextBehavior,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;

        holon.related_holons(relationship_name)
    }

    fn versioned_key_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<MapString, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let holon = self.get_rc_holon(context)?;
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

    fn essential_content_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<EssentialHolonContent, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;

        Ok(borrowed_holon.essential_content())
    }

    fn summarize_impl(&self, context: &dyn HolonsContextBehavior) -> Result<String, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;
        Ok(borrowed_holon.summarize())
    }

    fn into_model_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonNodeModel, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;

        Ok(borrowed_holon.into_node_model())
    }

    fn is_accessible_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        access_type: AccessType,
    ) -> Result<(), HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on staged holon: {}",
                e
            ))
        })?;
        holon.is_accessible(access_type)?;

        Ok(())
    }
}

impl WritableHolonImpl for StagedReference {
    fn add_related_holons_impl(
        &mut self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(context, AccessType::Write)?;

        // Precompute keys before taking the holon write lock to avoid re-entrant locking on self-edges.
        let holons_with_keys: Vec<(HolonReference, Option<MapString>)> = holons
            .into_iter()
            .map(|h| {
                let key = h.key(context)?;
                Ok((h, key))
            })
            .collect::<Result<_, HolonError>>()?;

        let rc_holon = self.get_rc_holon(context)?;
        let mut holon_mut = rc_holon.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on staged holon: {}",
                e
            ))
        })?;

        holon_mut.add_related_holons_with_keys(relationship_name, holons_with_keys)?;
        Ok(self)
    }

    fn remove_related_holons_impl(
        &mut self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(context, AccessType::Write)?;
        let holons_with_keys: Vec<(HolonReference, Option<MapString>)> = holons
            .into_iter()
            .map(|h| {
                let key = h.key(context)?;
                Ok((h, key))
            })
            .collect::<Result<_, HolonError>>()?;
        info!(
            "Removing {:?} related holons from relationship: {:?}",
            holons_with_keys.len(),
            relationship_name
        );
        let rc_holon = self.get_rc_holon(context)?;
        let mut holon_mut = rc_holon.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on staged holon: {}",
                e
            ))
        })?;
        holon_mut.remove_related_holons_with_keys(&relationship_name, holons_with_keys)?;

        Ok(self)
    }

    fn with_property_value_impl(
        &mut self,
        context: &dyn HolonsContextBehavior,
        property: PropertyName,
        value: BaseValue,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(context, AccessType::Write)?;
        let rc_holon = self.get_rc_holon(context)?;
        let mut holon_mut = rc_holon.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on staged holon: {}",
                e
            ))
        })?;

        holon_mut.with_property_value(property, value)?;

        Ok(self)
    }

    fn remove_property_value_impl(
        &mut self,
        context: &dyn HolonsContextBehavior,
        name: PropertyName,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(context, AccessType::Write)?;
        let rc_holon = self.get_rc_holon(context)?;
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
        context: &dyn HolonsContextBehavior,
        descriptor_reference: HolonReference,
    ) -> Result<(), HolonError> {
        self.is_accessible(context, AccessType::Write)?;

        // Look up the existing descriptor(s) on THIS holon, not on the descriptor.
        let self_ref = HolonReference::Staged(self.clone());
        let existing_descriptor_option = self_ref.get_descriptor(context)?;

        if let Some(existing_descriptor) = existing_descriptor_option {
            // Remove the current descriptor edge
            self.remove_related_holons_impl(
                context,
                CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
                vec![existing_descriptor.clone()],
            )?;
        }

        // Attach the new descriptor edge
        self.add_related_holons_impl(
            context,
            CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
            vec![descriptor_reference],
        )?;

        Ok(())
    }

    fn with_predecessor_impl(
        &mut self,
        context: &dyn HolonsContextBehavior,
        predecessor_reference_option: Option<HolonReference>, // None passed just removes predecessor
    ) -> Result<(), HolonError> {
        self.is_accessible(context, AccessType::Write)?;
        let existing_predecessor_option = self.clone().predecessor(context)?;
        if let Some(predecessor) = existing_predecessor_option {
            self.remove_related_holons_impl(
                context,
                CoreRelationshipTypeName::Predecessor.as_relationship_name(),
                vec![predecessor.clone()],
            )?;
        }
        if let Some(predecessor_reference) = predecessor_reference_option {
            self.add_related_holons_impl(
                context,
                CoreRelationshipTypeName::Predecessor.as_relationship_name(),
                vec![predecessor_reference.clone()],
            )?;
        }

        Ok(())
    }
}

impl ToHolonCloneModel for StagedReference {
    fn holon_clone_model(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonCloneModel, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
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
