use derive_new::new;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::{
    fmt,
    sync::{Arc, RwLock},
};
use type_names::relationship_names::CoreRelationshipTypeName;

use base_types::{BaseValue, MapString};
use core_types::{
    HolonError, HolonId, HolonNodeModel, PropertyMap, PropertyName, PropertyValue,
    RelationshipName, TemporaryId,
};

use crate::reference_layer::readable_impl::ReadableHolonImpl;
use crate::reference_layer::writable_impl::WritableHolonImpl;
use crate::{
    core_shared_objects::{
        holon::{holon_utils::EssentialHolonContent, state::AccessType, Holon, HolonCloneModel},
        transient_holon_manager::ToHolonCloneModel,
        ReadableHolonState, WriteableHolonState,
    },
    reference_layer::{HolonReference, HolonsContextBehavior, ReadableHolon},
    HolonCollection, RelationshipMap,
};

#[derive(new, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransientReference {
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
    pub fn from_temporary_id(id: &TemporaryId) -> Self {
        TransientReference::new(id.clone())
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
        // Get TransientManagerAccess
        let transient_behavior = context.get_space_manager().get_transient_manager_access();

        // Retrieve the holon by its TemporaryId
        let rc_holon = transient_behavior.get_holon_by_id(&self.id)?;

        Ok(rc_holon)
    }

    pub fn get_temporary_id(&self) -> TemporaryId {
        self.id.clone()
    }

    pub fn reset_original_id(&self, context: &dyn HolonsContextBehavior) -> Result<(), HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let mut borrow = rc_holon.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!("Failed to acquire write lock on holon: {}", e))
        })?;
        borrow.update_original_id(None)
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
        context: &dyn HolonsContextBehavior,
    ) -> Result<PropertyMap, HolonError> {
        // Enforce read access
        self.is_accessible(context, AccessType::Read)?;

        // Get read access
        let arc_lock = self.get_rc_holon(context)?;
        let guard = arc_lock.read().map_err(|_| {
            HolonError::FailedToBorrow("TransientHolon RwLock poisoned (read)".into())
        })?;

        // Only the Transient variant exposes raw_property_map_clone()
        match guard.deref() {
            Holon::Transient(t) => Ok(t.raw_property_map_clone()),
            _ => Err(HolonError::InvalidHolonReference(
                "get_raw_property_map is only valid for Transient holons".into(),
            )),
        }
    }
}

impl fmt::Display for TransientReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TransientReference(id: {:?})", self.id)
    }
}

// ==========================
//   TRAIT IMPLEMENTATIONS
// ==========================

impl ReadableHolonImpl for TransientReference {
    fn clone_holon_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<TransientReference, HolonError> {
        self.is_accessible(context, AccessType::Clone)?;
        let rc_holon = self.get_rc_holon(context)?;
        let holon_clone_model = rc_holon.read().unwrap().holon_clone_model();

        let transient_behavior = context.get_space_manager().get_transient_behavior_service();

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
        let borrowed_holon = rc_holon.read().unwrap();

        Ok(borrowed_holon.all_related_holons()?)
    }

    fn essential_content_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<EssentialHolonContent, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.read().unwrap();
        borrowed_holon.essential_content()
    }

    fn holon_id_impl(&self, _context: &dyn HolonsContextBehavior) -> Result<HolonId, HolonError> {
        Err(HolonError::NotImplemented("TransientHolons do not have a HolonId".to_string()))
    }

    fn into_model_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonNodeModel, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.read().unwrap();

        Ok(borrowed_holon.into_node_model())
    }

    fn is_accessible_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        access_type: AccessType,
    ) -> Result<(), HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let holon = rc_holon.read().unwrap();
        holon.is_accessible(access_type)?;

        Ok(())
    }

    fn key_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<MapString>, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.read().unwrap();
        borrowed_holon.key().clone()
    }

    fn predecessor_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<HolonReference>, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let collection_arc = self.related_holons(context, CoreRelationshipTypeName::Predecessor)?;
        let collection = collection_arc.read().unwrap();
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
        let borrowed_holon = rc_holon.read().unwrap();
        borrowed_holon.property_value(property_name)
    }

    fn related_holons_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let holon = rc_holon.read().unwrap();

        Ok(holon.related_holons(relationship_name)?)
    }

    fn summarize_impl(&self, context: &dyn HolonsContextBehavior) -> Result<String, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon
            .read()
            .map_err(|e| HolonError::FailedToAcquireLock(format!("Failed to read holon: {}", e)))?;
        Ok(borrowed_holon.summarize())
    }

    fn versioned_key_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<MapString, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let holon = self.get_rc_holon(context)?;
        let key = holon.read().unwrap().versioned_key()?;

        Ok(key)
    }
}

impl WritableHolonImpl for TransientReference {
    fn add_related_holons_impl(
        &mut self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(context, AccessType::Write)?;
        let rc_holon = self.get_rc_holon(context)?;
        let mut holon_mut = rc_holon.write().unwrap();
        holon_mut.add_related_holons(context, relationship_name, holons)?;

        Ok(self)
    }

    fn remove_related_holons_impl(
        &mut self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        self.is_accessible(context, AccessType::Write)?;
        let rc_holon = self.get_rc_holon(context)?;
        let mut holon_mut = rc_holon.write().unwrap();
        holon_mut.remove_related_holons(context, relationship_name, holons)?;

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
        let mut holon_mut = rc_holon.write().unwrap();

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
        let mut holon_mut = rc_holon.write().unwrap();

        holon_mut.remove_property_value(&name)?;

        Ok(self)
    }

    fn with_descriptor_impl(
        &mut self,
        context: &dyn HolonsContextBehavior,
        descriptor_reference: HolonReference,
    ) -> Result<(), HolonError> {
        self.is_accessible(context, AccessType::Write)?;

        // Get the descriptor of the current holon, if any
        let self_ref = HolonReference::Transient(self.clone());
        let existing_descriptor_option = self_ref.get_descriptor(context)?;

        if let Some(existing_descriptor) = existing_descriptor_option {
            // Remove the current descriptor edge from this holon
            self.remove_related_holons_impl(
                context,
                CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
                vec![existing_descriptor],
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

impl ToHolonCloneModel for TransientReference {
    fn holon_clone_model(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonCloneModel, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let holon_clone_model = rc_holon.read().unwrap().holon_clone_model();

        Ok(holon_clone_model)
    }
}
