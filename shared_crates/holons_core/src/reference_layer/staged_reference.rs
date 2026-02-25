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

impl StagedReference {
    /// Marks the underlying StagedHolon that is referenced as 'Abandoned'
    ///
    /// Prevents a commit from taking place and restricts Holon to read-only access.
    ///
    /// # Arguments
    /// * `context` - A reference to an object implementing the `HolonsContextBehavior` trait.
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
    /// * `context` - A reference to an object implementing the `HolonsContextBehavior` trait.
    ///
    /// # Returns
    /// Rc<RefCell<Holon>>>
    ///
    fn get_rc_holon(&self) -> Result<Arc<RwLock<Holon>>, HolonError> {
        // Get NurseryAccess
        let nursery_access = self.context_handle.context().nursery_access_internal();

        // Retrieve the holon by its temporaryId
        let rc_holon = nursery_access.get_holon_by_id(&self.id)?;

        Ok(rc_holon)
    }

    pub fn is_in_state(
        &self,
        _context: &Arc<TransactionContext>,
        check_state: StagedState,
    ) -> Result<bool, HolonError> {
        use tracing::warn;
        warn!("CHECKING STATE :: {:#?}", self.clone());
        let rc_holon = self.get_rc_holon()?;
        warn!("GOT RC_HOLON");
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
}

impl fmt::Display for StagedReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StagedReference(id: {:?})", self.id)
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

        let transient_behavior = self.context_handle.context().transient_manager_access_internal();

        let cloned_holon_transient_reference =
            transient_behavior.new_from_clone_model(holon_clone_model)?;

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
}

impl WritableHolonImpl for StagedReference {
    fn add_related_holons_impl(
        &mut self,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;
        // Precompute keys before taking the holon write lock to avoid re-entrant locking on self-edges.
        let holons_with_keys: Vec<(HolonReference, Option<MapString>)> = holons
            .into_iter()
            .map(|h| {
                let key = h.key()?;
                Ok((h, key))
            })
            .collect::<Result<_, HolonError>>()?;

        let rc_holon = self.get_rc_holon()?;
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
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;
        let holons_with_keys: Vec<(HolonReference, Option<MapString>)> = holons
            .into_iter()
            .map(|h| {
                let key = h.key()?;
                Ok((h, key))
            })
            .collect::<Result<_, HolonError>>()?;
        info!(
            "Removing {:?} related holons from relationship: {:?}",
            holons_with_keys.len(),
            relationship_name
        );
        let rc_holon = self.get_rc_holon()?;
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
            self.remove_related_holons_impl(
                CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
                vec![existing_descriptor.clone()],
            )?;
        }

        // Attach the new descriptor edge
        self.add_related_holons_impl(
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
            self.remove_related_holons_impl(
                CoreRelationshipTypeName::Predecessor.as_relationship_name(),
                vec![predecessor.clone()],
            )?;
        }
        if let Some(predecessor_reference) = predecessor_reference_option {
            self.add_related_holons_impl(
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
