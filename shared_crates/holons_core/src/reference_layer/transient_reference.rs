use derive_new::new;
use std::ops::Deref;
use std::{
    fmt,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};
use type_names::relationship_names::CoreRelationshipTypeName;

use base_types::{BaseValue, MapString};
use core_types::{
    HolonError, HolonId, HolonNodeModel, PropertyMap, PropertyName, PropertyValue,
    RelationshipName, TemporaryId,
};

use crate::core_shared_objects::transactions::{
    TransactionContext, TransactionContextHandle, TxId,
};
use crate::reference_layer::readable_impl::ReadableHolonImpl;
use crate::reference_layer::writable_impl::WritableHolonImpl;
use crate::{
    core_shared_objects::{
        holon::{holon_utils::EssentialHolonContent, state::AccessType, Holon, HolonCloneModel},
        transient_holon_manager::ToHolonCloneModel,
        ReadableHolonState, WriteableHolonState,
    },
    reference_layer::{HolonReference, ReadableHolon},
    HolonCollection, RelationshipMap,
};

#[derive(new, Debug, Clone)]
pub struct TransientReference {
    context_handle: TransactionContextHandle,
    id: TemporaryId,
}

impl TransientReference {
    /// Creates a new `TransientReference` from a given TemporaryId without validation.
    ///
    /// # Arguments
    /// * `id` - A TemporaryId
    ///
    /// # Returns
    /// A new `TransientReference` wrapping the provided id.
    ///
    pub fn from_temporary_id(context_handle: TransactionContextHandle, id: &TemporaryId) -> Self {
        TransientReference::new(context_handle, id.clone())
    }

    pub fn reset_original_id(&self) -> Result<(), HolonError> {
        let rc_holon = self.get_rc_holon()?;
        let mut borrow = rc_holon.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!("Failed to acquire write lock on holon: {}", e))
        })?;
        borrow.update_original_id(None)
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
        // Get TransientManagerAccess
        let transient_behavior = self.context_handle.context().transient_manager_access_internal();

        // Retrieve the holon by its TemporaryId
        let rc_holon = transient_behavior.get_holon_by_id(&self.id)?;

        Ok(rc_holon)
    }

    // Locking policy: use these helpers whenever acquiring a lock on a holon in this type.
    // This keeps lock-poisoning behavior consistent and ensures failures are surfaced as
    // HolonError::FailedToAcquireLock instead of panicking.
    fn read_holon_guard<'a>(
        &self,
        rc_holon: &'a Arc<RwLock<Holon>>,
        operation: &str,
    ) -> Result<RwLockReadGuard<'a, Holon>, HolonError> {
        rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on transient holon for {}: {}",
                operation, e
            ))
        })
    }

    fn write_holon_guard<'a>(
        &self,
        rc_holon: &'a Arc<RwLock<Holon>>,
        operation: &str,
    ) -> Result<RwLockWriteGuard<'a, Holon>, HolonError> {
        rc_holon.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire write lock on transient holon for {}: {}",
                operation, e
            ))
        })
    }

    // ***** ACCESSORS *****

    pub fn temporary_id(&self) -> TemporaryId {
        self.id.clone()
    }

    /// Returns the transaction id this reference is bound to.
    pub fn tx_id(&self) -> TxId {
        self.context_handle.tx_id()
    }

    /// ⚠️ Returns a snapshot of the raw property map of this holon.
    ///
    /// Intended **only** for the holon loader, specifically for LoaderHolons whose
    /// property set is unknown at load time. Do **NOT** use outside the loader context.
    ///
    /// We return an owned `PropertyMap` rather than `&PropertyMap` because the holon is
    /// accessed via `Rc<RefCell<...>>`; any reference would be tied to a temporary borrow.
    pub fn get_raw_property_map(
        &self,
        _context: &Arc<TransactionContext>,
    ) -> Result<PropertyMap, HolonError> {
        // Enforce read access
        self.is_accessible(AccessType::Read)?;

        // Get read access
        let arc_lock = self.get_rc_holon()?;
        let guard = self.read_holon_guard(&arc_lock, "get_raw_property_map")?;

        // Only the Transient variant exposes raw_property_map_clone()
        match guard.deref() {
            Holon::Transient(t) => Ok(t.raw_property_map_clone()),
            _ => Err(HolonError::InvalidHolonReference(
                "get_raw_property_map is only valid for Transient holons".into(),
            )),
        }
    }

    // Simple string representations for errors/logging
    pub fn reference_kind_string(&self) -> String {
        "TransientReference".to_string()
    }

    pub fn reference_id_string(&self) -> String {
        format!("TemporaryId={}", self.id)
    }
}

impl fmt::Display for TransientReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TransientReference({})", self.reference_id_string())
    }
}

// ==========================
//   TRAIT IMPLEMENTATIONS
// ==========================

impl ReadableHolonImpl for TransientReference {
    fn clone_holon_impl(&self) -> Result<TransientReference, HolonError> {
        self.is_accessible(AccessType::Clone)?;
        let rc_holon = self.get_rc_holon()?;
        let holon_clone_model =
            self.read_holon_guard(&rc_holon, "clone_holon_impl")?.holon_clone_model();

        let transient_behavior = self.context_handle.context().transient_manager_access_internal();

        let cloned_holon_transient_reference =
            transient_behavior.new_from_clone_model(holon_clone_model)?;

        Ok(cloned_holon_transient_reference)
    }

    fn all_related_holons_impl(&self) -> Result<RelationshipMap, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = self.read_holon_guard(&rc_holon, "all_related_holons_impl")?;

        Ok(borrowed_holon.all_related_holons()?)
    }

    fn essential_content_impl(&self) -> Result<EssentialHolonContent, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = self.read_holon_guard(&rc_holon, "essential_content_impl")?;

        Ok(borrowed_holon.essential_content())
    }

    fn holon_id_impl(&self) -> Result<HolonId, HolonError> {
        Err(HolonError::InvalidTransition(
            "Transient holons do not have a HolonId; persistent identity is only available after commit".to_string(),
        ))
    }

    fn into_model_impl(&self) -> Result<HolonNodeModel, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = self.read_holon_guard(&rc_holon, "into_model_impl")?;

        Ok(borrowed_holon.into_node_model())
    }

    fn is_accessible_impl(&self, access_type: AccessType) -> Result<(), HolonError> {
        let rc_holon = self.get_rc_holon()?;
        let holon = self.read_holon_guard(&rc_holon, "is_accessible_impl")?;
        holon.is_accessible(access_type)?;

        Ok(())
    }

    fn key_impl(&self) -> Result<Option<MapString>, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = self.read_holon_guard(&rc_holon, "key_impl")?;

        Ok(borrowed_holon.key().clone())
    }

    fn predecessor_impl(&self) -> Result<Option<HolonReference>, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let collection_arc = self.related_holons(CoreRelationshipTypeName::Predecessor)?;
        let collection = collection_arc.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on predecessor collection: {}",
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
        let borrowed_holon = self.read_holon_guard(&rc_holon, "property_value_impl")?;
        borrowed_holon.property_value(property_name)
    }

    fn related_holons_impl(
        &self,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let holon = self.read_holon_guard(&rc_holon, "related_holons_impl")?;

        Ok(holon.related_holons(relationship_name)?)
    }

    fn summarize_impl(&self) -> Result<String, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let borrowed_holon = self.read_holon_guard(&rc_holon, "summarize_impl")?;
        Ok(borrowed_holon.summarize())
    }

    fn versioned_key_impl(&self) -> Result<MapString, HolonError> {
        self.is_accessible(AccessType::Read)?;
        let rc_holon = self.get_rc_holon()?;
        let key = self.read_holon_guard(&rc_holon, "versioned_key_impl")?.versioned_key()?;

        Ok(key)
    }
}

impl WritableHolonImpl for TransientReference {
    fn add_related_holons_impl(
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
        let rc_holon = self.get_rc_holon()?;
        let mut holon_mut = self.write_holon_guard(&rc_holon, "add_related_holons_impl")?;
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
        let rc_holon = self.get_rc_holon()?;
        let mut holon_mut = self.write_holon_guard(&rc_holon, "remove_related_holons_impl")?;
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
        let mut holon_mut = self.write_holon_guard(&rc_holon, "with_property_value_impl")?;

        holon_mut.with_property_value(property, value)?;

        Ok(self)
    }

    fn remove_property_value_impl(&mut self, name: PropertyName) -> Result<&mut Self, HolonError> {
        self.is_accessible(AccessType::Write)?;
        let rc_holon = self.get_rc_holon()?;
        let mut holon_mut = self.write_holon_guard(&rc_holon, "remove_property_value_impl")?;

        holon_mut.remove_property_value(&name)?;

        Ok(self)
    }

    fn with_descriptor_impl(
        &mut self,
        descriptor_reference: HolonReference,
    ) -> Result<(), HolonError> {
        self.is_accessible(AccessType::Write)?;

        // Get the descriptor of the current holon, if any
        let self_ref = HolonReference::Transient(self.clone());
        let existing_descriptor_option = self_ref.get_descriptor()?;

        if let Some(existing_descriptor) = existing_descriptor_option {
            // Remove the current descriptor edge from this holon
            self.remove_related_holons_impl(
                CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
                vec![existing_descriptor],
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

impl ToHolonCloneModel for TransientReference {
    fn holon_clone_model(&self) -> Result<HolonCloneModel, HolonError> {
        let rc_holon = self.get_rc_holon()?;
        let holon_clone_model =
            self.read_holon_guard(&rc_holon, "holon_clone_model")?.holon_clone_model();

        Ok(holon_clone_model)
    }
}

impl PartialEq for TransientReference {
    fn eq(&self, other: &Self) -> bool {
        self.context_handle.tx_id() == other.context_handle.tx_id() && self.id == other.id
    }
}

impl Eq for TransientReference {}
