use derive_new::new;
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, fmt, rc::Rc, sync::Arc};
use std::{
    fmt,
    sync::{Arc, RwLock},
};
use tracing::debug;
use type_names::relationship_names::CoreRelationshipTypeName;

use crate::reference_layer::readable_impl::ReadableHolonImpl;
use crate::reference_layer::transient_holon_behavior::TransientHolonBehavior;
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
        Holon, Holon, HolonCollection, HolonCollection, NurseryAccess, NurseryAccess,
        ReadableHolonState, ReadableHolonState, WriteableHolonState, WriteableHolonState,
    },
    Nursery, RelationshipMap,
};
use base_types::{BaseValue, MapString};
use core_types::{
    HolonError, HolonId, HolonNodeModel, PropertyName, PropertyValue, RelationshipName, TemporaryId,
};

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
        let nursery_access = context.get_space_manager().get_nursery_access();
        let pool = nursery_access.read().map_err(|e| {
            HolonError::FailedToAcquireLock(format!(
                "Failed to acquire read lock on nursery: {}",
                e
            ))
        })?;

        // Retrieve the holon by its temporaryId
        let rc_holon = pool.get_holon_by_id(&self.id)?;

        Ok(rc_holon)
    }

    /// Retrieves access to the nursery via the provided context.
    ///
    /// # Arguments
    /// * `context` - A reference to an object implementing the `HolonsContextBehavior` trait.
    ///
    /// # Returns
    /// A reference to an object implementing the `NurseryAccess` trait.
    ///
    /// # Panics
    /// This function assumes that the context and space manager will always return valid references.
    fn get_nursery_access(context: &dyn HolonsContextBehavior) -> Arc<RwLock<Nursery>> {
        // Retrieve the space manager from the context
        let space_manager = context.get_space_manager();

        // Get the nursery access
        space_manager.get_nursery_access()
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
        let holon_clone_model = rc_holon.read().unwrap().holon_clone_model();

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

        borrowed_holon.all_related_holons()
    }

    fn essential_content_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<EssentialHolonContent, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.read().unwrap();

        borrowed_holon.all_related_holons()
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

    fn holon_id_impl(&self, context: &dyn HolonsContextBehavior) -> Result<HolonId, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.read().unwrap();

        borrowed_holon.holon_id()
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

        holon.related_holons(relationship_name)
    }

    fn summarize_impl(&self, context: &dyn HolonsContextBehavior) -> Result<String, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.read().unwrap();
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

impl WritableHolonImpl for StagedReference {
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

    fn with_descriptor_impl(
        &mut self,
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
            self.add_related_holons_impl(
                context,
                CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
                vec![descriptor_reference],
            )?;

            Ok(())
        } else {
            self.add_related_holons_impl(
                context,
                CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
                vec![descriptor_reference.clone()],
            )?;

            Ok(())
        }
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
}

impl ToHolonCloneModel for StagedReference {
    fn holon_clone_model(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonCloneModel, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let holon_clone_model = rc_holon.read().unwrap().holon_clone_model();

        Ok(holon_clone_model)
    }
}
