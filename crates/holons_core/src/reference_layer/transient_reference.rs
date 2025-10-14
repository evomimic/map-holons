use derive_new::new;
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    sync::{Arc, RwLock},
};
use tracing::debug;
use type_names::relationship_names::CoreRelationshipTypeName;

use base_types::{BaseValue, MapString};
use core_types::{
    HolonError, HolonId, HolonNodeModel, PropertyName, PropertyValue, RelationshipName, TemporaryId,
};

use crate::reference_layer::readable_impl::ReadableHolonImpl;
use crate::reference_layer::transient_holon_behavior::TransientHolonBehavior;
use crate::reference_layer::writable_impl::WritableHolonImpl;
use crate::{
    core_shared_objects::{
        holon::{
            holon_utils::EssentialHolonContent, state::AccessType, Holon, HolonBehavior,
            HolonCloneModel, TransientHolon,
        },
        transient_holon_manager::ToHolonCloneModel,
        TransientManagerAccess, TransientRelationshipMap,
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
    /// Rc<RefCell<TransientHolon>>>
    ///
    fn get_rc_holon(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Arc<RwLock<TransientHolon>>, HolonError> {
        // Get TransientManagerAccess
        let transient_manager_access = context.get_space_manager().get_transient_manager_access();
        let transient_read = transient_manager_access.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on transient_manager: {}",
                e
            ))
        })?;

        // Retrieve the holon by its TemporaryId
        let rc_holon = transient_read.get_holon_by_id(&self.id)?;

        // Confirm it references a TransientHolon and return an Rc<RefCell
        let holon = rc_holon.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!("Failed to acquire read lock on holon: {}", e))
        })?;
        match holon.clone() {
            Holon::Transient(transient_holon) => Ok(Arc::new(RwLock::new(transient_holon))),
            _ => Err(HolonError::InvalidHolonReference("The TemporaryId associated with a TransientReference must return a TransientHolon!".to_string()))

        }
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

    pub fn update_relationship_map(
        &self,
        context: &dyn HolonsContextBehavior,
        map: TransientRelationshipMap,
    ) -> Result<(), HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let mut borrow = rc_holon.write().map_err(|e| {
            HolonError::FailedToAcquireLock(format!("Failed to acquire write lock on holon: {}", e))
        })?;
        borrow.update_relationship_map(map)
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
        let holon_clone_model = rc_holon.read().unwrap().get_holon_clone_model();

        let transient_behavior_service =
            context.get_space_manager().get_transient_behavior_service();
        let transient_behavior = transient_behavior_service.read().unwrap();

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

        Ok(RelationshipMap::from(borrowed_holon.get_transient_relationship_map()?))
    }

    fn holon_id_impl(&self, context: &dyn HolonsContextBehavior) -> Result<HolonId, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.read().unwrap();
        let local_id = borrowed_holon.get_local_id()?;

        Ok(HolonId::from(local_id))
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
                "get_related_holons for PREDECESSOR returned multiple members: {:#?}",
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
        borrowed_holon.get_property_value(property_name)
    }

    fn key_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<MapString>, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.read().unwrap();
        borrowed_holon.get_key().clone()
    }

    fn related_holons_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let holon = rc_holon.read().unwrap();

        // Use the public `get_related_holons` method on the `TransientRelationshipMap`
        Ok(holon.get_related_holons(relationship_name)?)
    }

    fn versioned_key_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<MapString, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let holon = self.get_rc_holon(context)?;
        let key = holon.read().unwrap().get_versioned_key()?;

        Ok(key)
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

    fn summarize_impl(&self, context: &dyn HolonsContextBehavior) -> Result<String, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon
            .read()
            .map_err(|e| HolonError::FailedToAcquireLock(format!("Failed to read holon: {}", e)))?;
        Ok(borrowed_holon.summarize())
    }

    fn into_model_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonNodeModel, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.read().unwrap();

        Ok(borrowed_holon.into_node())
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
}

impl WritableHolonImpl for TransientReference {
    fn add_related_holons_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        debug!("Entered TransientReference::add_related_holons");
        // Ensure the holon is accessible for write
        self.is_accessible(context, AccessType::Write)?;

        // Get access to the source holon and its relationship map
        let rc_holon = self.get_rc_holon(context)?;
        let mut holon = rc_holon.write().unwrap();
        let mut transient_relationship_map = holon.get_transient_relationship_map()?;

        // Delegate adding holons to the `TransientRelationshipMap`
        transient_relationship_map.add_related_holons(context, relationship_name, holons)?;
        holon.update_relationship_map(transient_relationship_map)?;

        Ok(())
    }

    fn remove_related_holons_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        debug!("Entered TransientReference::remove_related_holons");

        // Ensure the holon is accessible for write
        self.is_accessible(context, AccessType::Write)?;

        // Get access to the source holon and its relationship map
        let rc_holon = self.get_rc_holon(context)?;
        let holon = rc_holon.write().unwrap();
        let mut staged_relationship_map = holon.get_transient_relationship_map()?;

        debug!(
            "Here is the RelationshipMap before removing related Holons: {:#?}",
            staged_relationship_map
        );

        // Delegate the removal of holons to the `StagedRelationshipMap`
        staged_relationship_map.remove_related_holons(context, &relationship_name, holons)?;

        debug!(
            "Here is the RelationshipMap after removing related Holons: {:#?}",
            staged_relationship_map
        );

        Ok(())
    }

    fn with_property_value_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        property: PropertyName,
        value: BaseValue,
    ) -> Result<(), HolonError> {
        self.is_accessible(context, AccessType::Write)?;
        let rc_holon = self.get_rc_holon(context)?;
        let mut holon_refcell = rc_holon.write().unwrap();

        // Call the Holon's with_property_value method
        holon_refcell.with_property_value(property, value)?;

        Ok(())
    }

    fn remove_property_value_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        name: PropertyName,
    ) -> Result<(), HolonError> {
        self.is_accessible(context, AccessType::Write)?;
        let rc_holon = self.get_rc_holon(context)?;
        let mut holon_refcell = rc_holon.write().unwrap();

        holon_refcell.remove_property_value(&name)?;

        Ok(())
    }

    fn with_descriptor_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        descriptor_reference: HolonReference,
    ) -> Result<(), HolonError> {
        self.is_accessible(context, AccessType::Write)?;
        let existing_descriptor_option = descriptor_reference.get_descriptor(context)?;
        if let Some(descriptor) = existing_descriptor_option {
            self.remove_related_holons_impl(
                context,
                CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
                vec![descriptor.clone()],
            )?;
            debug!("removed existing descriptor: {:#?}", descriptor);
            self.add_related_holons_impl(
                context,
                CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
                vec![descriptor_reference],
            )?;
            debug!("added descriptor: {:#?}", descriptor);

            Ok(())
        } else {
            self.add_related_holons_impl(
                context,
                CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
                vec![descriptor_reference.clone()],
            )?;
            debug!("added descriptor: {:#?}", descriptor_reference);

            Ok(())
        }
    }

    fn with_predecessor_impl(
        &self,
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
            debug!("removed existing predecessor: {:#?}", predecessor);
        }
        if let Some(predecessor_reference) = predecessor_reference_option {
            self.add_related_holons_impl(
                context,
                CoreRelationshipTypeName::Predecessor.as_relationship_name(),
                vec![predecessor_reference.clone()],
            )?;
            debug!("added predecessor: {:#?}", predecessor_reference);
        }

        Ok(())
    }
}

impl ToHolonCloneModel for TransientReference {
    fn get_holon_clone_model(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonCloneModel, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let holon_clone_model = rc_holon.read().unwrap().get_holon_clone_model();

        Ok(holon_clone_model)
    }
}
