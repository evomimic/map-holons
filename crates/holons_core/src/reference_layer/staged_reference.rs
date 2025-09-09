use derive_new::new;
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, fmt, rc::Rc, sync::Arc};
use tracing::debug;
use type_names::{
    relationship_names::{CoreRelationshipTypeName, ToRelationshipName},
    ToPropertyName,
};

use crate::{
    core_shared_objects::holon::HolonCloneModel,
    reference_layer::{
        HolonReference, HolonsContextBehavior, ReadableHolon, ReadableHolonReferenceLayer,
        TransientReference, WriteableHolon, WriteableHolonReferenceLayer,
    },
};
use crate::{
    core_shared_objects::{
        holon::{holon_utils::EssentialHolonContent, state::AccessType},
        transient_holon_manager::ToHolonCloneModel,
        Holon, HolonBehavior, HolonCollection, NurseryAccess, ReadableRelationship,
        WritableRelationship,
    },
    RelationshipMap,
};
use base_types::{BaseValue, MapString};
use core_types::{HolonError, HolonId, TemporaryId};
use integrity_core_types::{HolonNodeModel, PropertyName, PropertyValue, RelationshipName};

#[derive(new, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
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
        &mut self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<(), HolonError> {
        debug!("Entered: abandon_staged_changes for staged_id: {:#?}", self.id);
        // Get access to the source holon
        let rc_holon = self.get_rc_holon(context)?;

        // Borrow the holon mutably
        let mut holon_mut = rc_holon.borrow_mut();

        match &mut *holon_mut {
            Holon::Staged(staged_holon) => {
                debug!("Mutably borrowing Holon::Staged for staged_id: {:#?}", self.id);
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
    ) -> Result<Rc<RefCell<Holon>>, HolonError> {
        // Get NurseryAccess
        let nursery_access = Self::get_nursery_access(context);

        let nursery_read = nursery_access.borrow();

        // Retrieve the holon by its temporaryId
        let rc_holon = nursery_read.get_holon_by_id(&self.id)?;

        match &*rc_holon.borrow() {
            Holon::Staged(_) => {}
            Holon::Saved(_) => {
                return Err(HolonError::InvalidHolonReference(
                    "Expected Staged, got: Saved".to_string(),
                ))
            }
            Holon::Transient(_) => {
                return Err(HolonError::InvalidHolonReference(
                    "Expected Staged, got: Transient".to_string(),
                ))
            }
        }

        Ok(rc_holon.clone())
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
    fn get_nursery_access(context: &dyn HolonsContextBehavior) -> Arc<RefCell<dyn NurseryAccess>> {
        // Retrieve the space manager from the context
        let space_manager = context.get_space_manager();

        // Get the nursery access
        space_manager.get_nursery_access()
    }

    pub fn get_temporary_id(&self) -> TemporaryId {
        self.id.clone()
    }
}

impl fmt::Display for StagedReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StagedReference(id: {:?})", self.id)
    }
}

impl ReadableHolonReferenceLayer for StagedReference {
    fn clone_holon(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<TransientReference, HolonError> {
        self.is_accessible(context, AccessType::Clone)?;
        let rc_holon = self.get_rc_holon(context)?;
        let holon_clone_model = rc_holon.borrow().get_holon_clone_model();

        let transient_behavior_service =
            context.get_space_manager().get_transient_behavior_service();
        let transient_behavior = transient_behavior_service.borrow();

        let cloned_holon_transient_reference =
            transient_behavior.new_from_clone_model(holon_clone_model)?;

        Ok(cloned_holon_transient_reference)
    }

    fn essential_content(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<EssentialHolonContent, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();
        borrowed_holon.essential_content()
    }

    fn get_all_related_holons(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<RelationshipMap, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();

        Ok(RelationshipMap::from(borrowed_holon.into_staged()?.get_staged_relationship_map()?))
    }

    fn get_holon_id(&self, context: &dyn HolonsContextBehavior) -> Result<HolonId, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();
        let local_id = borrowed_holon.get_local_id()?;

        Ok(HolonId::from(local_id))
    }

    fn get_predecessor(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<HolonReference>, HolonError> {
        let collection = self.get_related_holons(context, CoreRelationshipTypeName::Predecessor)?;
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

    fn get_property_value(
        &self,
        context: &dyn HolonsContextBehavior,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();
        borrowed_holon.get_property_value(property_name)
    }

    fn get_key(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<MapString>, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();
        borrowed_holon.get_key().clone()
    }

    fn get_related_holons_ref_layer(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;

        let holon = rc_holon.borrow();

        match &*holon {
            Holon::Staged(staged_holon) => {
                let staged_relationship_map = staged_holon.get_staged_relationship_map()?;
                // Get collection for related holons
                let collection = staged_relationship_map.get_related_holons(relationship_name);
                Ok(collection)
            }
            _ => {
                unreachable!()
            }
        }
    }

    fn get_versioned_key(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<MapString, HolonError> {
        let holon = self.get_rc_holon(context)?;
        let key = holon.borrow().get_versioned_key()?;

        Ok(key)
    }

    fn into_model(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonNodeModel, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();

        Ok(borrowed_holon.into_node())
    }

    fn is_accessible(
        &self,
        context: &dyn HolonsContextBehavior,
        access_type: AccessType,
    ) -> Result<(), HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let holon = rc_holon.borrow();
        holon.is_accessible(access_type)?;

        Ok(())
    }
}

impl WriteableHolonReferenceLayer for StagedReference {
    fn add_related_holons_ref_layer(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        debug!("Entered StagedReference::add_related_holons");
        // Ensure the holon is accessible for write
        self.is_accessible(context, AccessType::Write)?;

        // Get access to the source holon and its relationshp map
        let rc_holon = self.get_rc_holon(context)?;

        // Mutably borrow the inner Holon and match it
        let mut holon_mut = rc_holon.borrow_mut();

        match &mut *holon_mut {
            Holon::Staged(staged_holon) => {
                let mut staged_relationship_map = staged_holon.get_staged_relationship_map()?;
                // Delegate the addition of holons to the `StagedRelationshipMap`
                staged_relationship_map.add_related_holons(context, relationship_name, holons)?;
                staged_holon.update_relationship_map(staged_relationship_map)?;
            }
            _ => {
                unreachable!()
            }
        }

        Ok(())
    }

    fn remove_property_value_ref_layer(
        &self,
        context: &dyn HolonsContextBehavior,
        name: PropertyName,
    ) -> Result<&Self, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let mut holon_refcell = rc_holon.borrow_mut();

        match &mut *holon_refcell {
            Holon::Staged(staged_holon) => {
                staged_holon.remove_property_value(&name)?;
            }
            _ => {
                unreachable!()
            }
        }

        Ok(self)
    }

    fn remove_related_holons_ref_layer(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        debug!("Entered StagedReference::remove_related_holons");

        // Ensure the holon is accessible for write
        self.is_accessible(context, AccessType::Write)?;

        // Get access to the source holon and its relationship map
        let rc_holon = self.get_rc_holon(context)?;

        // Mutably borrow the inner Holon and match it
        let mut holon_mut = rc_holon.borrow_mut();

        match &mut *holon_mut {
            Holon::Staged(staged_holon) => {
                let mut staged_relationship_map = staged_holon.get_staged_relationship_map()?;
                // Delegate the removal of holons to the `StagedRelationshipMap`
                staged_relationship_map.remove_related_holons(
                    context,
                    &relationship_name,
                    holons,
                )?;
                staged_holon.update_relationship_map(staged_relationship_map)?;
            }
            _ => {
                unreachable!()
            }
        }

        Ok(())
    }

    fn with_descriptor(
        &self,
        context: &dyn HolonsContextBehavior,
        descriptor_reference: HolonReference,
    ) -> Result<&Self, HolonError> {
        let holon = self.get_rc_holon(context)?;
        holon.borrow().is_accessible(AccessType::Write)?;
        let existing_descriptor_option = descriptor_reference.get_descriptor(context)?;
        if let Some(descriptor) = existing_descriptor_option {
            self.remove_related_holons_ref_layer(
                context,
                CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
                vec![descriptor.clone()],
            )?;
            debug!("removed existing descriptor: {:#?}", descriptor);
            self.add_related_holons_ref_layer(
                context,
                CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
                vec![descriptor_reference],
            )?;
            debug!("added descriptor: {:#?}", descriptor);

            Ok(self)
        } else {
            self.add_related_holons_ref_layer(
                context,
                CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
                vec![descriptor_reference.clone()],
            )?;
            debug!("added descriptor: {:#?}", descriptor_reference);

            Ok(self)
        }
    }

    fn with_predecessor(
        &self,
        context: &dyn HolonsContextBehavior,
        predecessor_reference_option: Option<HolonReference>, // None passed just removes predecessor
    ) -> Result<(), HolonError> {
        self.is_accessible(context, AccessType::Write)?;
        let existing_predecessor_option = self.clone().get_predecessor(context)?;
        if let Some(predecessor) = existing_predecessor_option {
            self.remove_related_holons_ref_layer(
                context,
                CoreRelationshipTypeName::Predecessor.as_relationship_name(),
                vec![predecessor.clone()],
            )?;
            debug!("removed existing predecessor: {:#?}", predecessor);
        }
        if let Some(predecessor_reference) = predecessor_reference_option {
            self.add_related_holons_ref_layer(
                context,
                CoreRelationshipTypeName::Predecessor.as_relationship_name(),
                vec![predecessor_reference.clone()],
            )?;
            debug!("added predecessor: {:#?}", predecessor_reference);
        }

        Ok(())
    }

    fn with_property_value(
        &self,
        context: &dyn HolonsContextBehavior,
        property: PropertyName,
        value: BaseValue,
    ) -> Result<&Self, HolonError> {
        self.is_accessible(context, AccessType::Write)?;
        let rc_holon = self.get_rc_holon(context)?;

        // Mutably borrow the inner Holon and match it
        let mut holon_mut = rc_holon.borrow_mut();

        match &mut *holon_mut {
            Holon::Staged(staged_holon) => {
                staged_holon.with_property_value(property, value)?;
            }
            _ => {
                unreachable!()
            }
        }

        Ok(self)
    }
}

impl WriteableHolon for StagedReference {
    fn add_related_holons<T: ToRelationshipName>(
        &self,
        context: &dyn HolonsContextBehavior,
        name: T,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        self.add_related_holons_ref_layer(context, name.to_relationship_name(), holons)
    }

    fn remove_property_value<T: ToPropertyName>(
        &self,
        context: &dyn HolonsContextBehavior,
        name: T,
    ) -> Result<&Self, HolonError> {
        self.remove_property_value_ref_layer(context, name.to_property_name())
    }

    fn remove_related_holons<T: ToRelationshipName>(
        &self,
        context: &dyn HolonsContextBehavior,
        name: T,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        self.remove_related_holons_ref_layer(context, name.to_relationship_name(), holons)
    }
}

impl ToHolonCloneModel for StagedReference {
    fn get_holon_clone_model(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonCloneModel, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let holon_clone_model = rc_holon.borrow().get_holon_clone_model();

        Ok(holon_clone_model)
    }
}
