use derive_new::new;
use hdk::prelude::*;
use core_types::PropertyName;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

use crate::reference_layer::{HolonReadable, HolonReference, HolonWritable, HolonsContextBehavior};

use crate::core_shared_objects::{
    AccessType, EssentialHolonContent, Holon, HolonCollection, HolonError, HolonState,
    NurseryAccess, RelationshipName,
};

//use shared_types_holon::{BaseValue, HolonId, MapString, PropertyValue, TemporaryId};
use base_types::{BaseValue, MapString};
use core_types::{HolonId, TemporaryId, PropertyValue};

#[derive(new, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct StagedReference {
    id: TemporaryId, // the position of the holon with CommitManager's staged_holons vector
}

impl StagedReference {
    /// Creates a new `StagedReference` from a given TemporaryId without validation.
    ///
    /// # Arguments
    ///
    /// * `id` - A TemporaryId
    ///
    /// # Returns
    ///
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

        // Return a clone of the Rc<RefCell<Holon>>
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

    fn get_temporary_id(&self) -> &TemporaryId {
        &self.id
    }
}

impl fmt::Display for StagedReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StagedReference(id: {:?})", self.id)
    }
}

impl HolonReadable for StagedReference {
    fn clone_holon(&self, context: &dyn HolonsContextBehavior) -> Result<Holon, HolonError> {
        let holon = self.get_rc_holon(context)?;
        let holon_read = holon.borrow();
        holon_read.clone_holon()
    }

    fn essential_content(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<EssentialHolonContent, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();
        borrowed_holon.essential_content()
    }

    fn get_holon_id(&self, context: &dyn HolonsContextBehavior) -> Result<HolonId, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();
        let local_id = borrowed_holon.get_local_id()?;

        Ok(HolonId::from(local_id))
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

    fn get_related_holons(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let holon = rc_holon.borrow();

        // Use the public `get_related_holons` method on the `StagedRelationshipMap`
        Ok(holon.staged_relationship_map.get_related_holons(relationship_name))
    }

    fn get_versioned_key(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<MapString, HolonError> {
        let holon = self.get_rc_holon(context)?;
        let key = holon.borrow().get_versioned_key()?;

        Ok(key)
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

impl HolonWritable for StagedReference {
    fn abandon_staged_changes(
        &mut self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<(), HolonError> {
        debug!("Entered: abandon_staged_changes for staged_id: {:#?}", self.id);
        // Get mutable access to the source holon
        let holon_refcell = self.get_rc_holon(context)?;

        // Borrow the holon from the RefCell
        let mut holon = holon_refcell.borrow_mut();

        debug!("borrowed mut for holon: {:#?}", self.id);

        holon.abandon_staged_changes()?;

        Ok(())
    }

    fn add_related_holons(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        debug!("Entered StagedReference::add_related_holons");
        // Ensure the holon is accessible for write
        self.is_accessible(context, AccessType::Write)?;

        // Get mutable access to the source holon
        let rc_holon = self.get_rc_holon(context)?;
        let mut holon = rc_holon.borrow_mut();

        debug!(
            "Here is the RelationshipMap before adding related Holons: {:#?}",
            holon.staged_relationship_map
        );

        // Delegate adding holons to the `StagedRelationshipMap`
        holon.staged_relationship_map.add_related_holons(context, relationship_name, holons)?;

        debug!(
            "Here is the RelationshipMap after adding related Holons: {:#?}",
            holon.staged_relationship_map
        );

        Ok(())
    }

    fn clone_reference(&self) -> StagedReference {
        StagedReference { id: self.get_temporary_id().clone() }
    }

    fn get_predecessor(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<HolonReference>, HolonError> {
        let relationship_name = RelationshipName(MapString("PREDECESSOR".to_string()));
        // let relationship_name = CoreSchemaRelationshipTypeName::DescribedBy.to_string();
        let collection = self.get_related_holons(context, &relationship_name)?;
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

    fn remove_related_holons(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: &RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        debug!("Entered StagedReference::remove_related_holons");

        // Ensure the holon is accessible for write
        self.is_accessible(context, AccessType::Write)?;

        // Get mutable access to the source holon
        let rc_holon = self.get_rc_holon(context)?;
        let mut holon = rc_holon.borrow_mut();

        debug!(
            "Here is the RelationshipMap before removing related Holons: {:#?}",
            holon.staged_relationship_map
        );

        // Delegate the removal of holons to the `StagedRelationshipMap`
        holon.staged_relationship_map.remove_related_holons(context, relationship_name, holons)?;

        debug!(
            "Here is the RelationshipMap after removing related Holons: {:#?}",
            holon.staged_relationship_map
        );

        Ok(())
    }

    /// Stages a new Holon by cloning an existing Holon, without retaining lineage to the Holon its cloned from.
    fn stage_new_from_clone_deprecated(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Holon, HolonError> {
        let holon = self.get_rc_holon(context)?;
        let cloned_holon = holon.borrow().clone_holon()?;
        // cloned_holon.load_all_relationships(context)?;

        Ok(cloned_holon)
    }

    fn with_descriptor(
        &self,
        context: &dyn HolonsContextBehavior,
        descriptor_reference: HolonReference,
    ) -> Result<&Self, HolonError> {
        let holon = self.get_rc_holon(context)?;
        holon.borrow().is_accessible(AccessType::Write)?;
        let existing_descriptor_option = descriptor_reference.get_descriptor(context)?;
        let relationship_name = RelationshipName(MapString("DESCRIBED_BY".to_string()));
        // let relationship_name = CoreSchemaRelationshipTypeName::DescribedBy.to_string();
        if let Some(descriptor) = existing_descriptor_option {
            self.remove_related_holons(context, &relationship_name, vec![descriptor.clone()])?;
            debug!("removed existing descriptor: {:#?}", descriptor);
            self.add_related_holons(context, relationship_name, vec![descriptor_reference])?;
            debug!("added descriptor: {:#?}", descriptor);

            Ok(self)
        } else {
            self.add_related_holons(
                context,
                relationship_name,
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
        let relationship_name = RelationshipName(MapString("PREDECESSOR".to_string()));
        let existing_predecessor_option = self.clone().get_predecessor(context)?;
        if let Some(predecessor) = existing_predecessor_option {
            self.remove_related_holons(context, &relationship_name, vec![predecessor.clone()])?;
            debug!("removed existing predecessor: {:#?}", predecessor);
        }
        if let Some(predecessor_reference) = predecessor_reference_option {
            // let relationship_name = CoreSchemaRelationshipTypeName::Predecessor.to_string();

            self.add_related_holons(
                context,
                relationship_name,
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
        value: Option<BaseValue>,
    ) -> Result<&Self, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let mut holon_refcell = rc_holon.borrow_mut();

        // Call the Holon's with_property_value method
        holon_refcell.with_property_value(property, value)?;

        Ok(self)
    }
}
