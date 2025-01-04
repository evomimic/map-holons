use derive_new::new;
use hdk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use shared_types_holon::holon_node::PropertyName;

use crate::reference_layer::{HolonReadable, HolonReference, HolonWritable, HolonsContextBehavior};
use crate::shared_objects_layer::nursery_access::NurseryAccess;
use crate::shared_objects_layer::{
    AccessType, EssentialHolonContent, Holon, HolonCollection, HolonError, HolonState,
    RelationshipName,
};
use shared_types_holon::{BaseValue, HolonId, MapString, PropertyValue};

/// a StagedIndex identifies a StagedHolon by its position within the staged_holons vector
pub type StagedIndex = usize;

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct StagedReference {
    // pub rc_holon: Rc<RefCell<Holon>>, // Ownership moved to CommitManager
    pub holon_index: StagedIndex, // the position of the holon with CommitManager's staged_holons vector
}

impl StagedReference {
    /// Creates a new `StagedReference` from a given index without validation.
    ///
    /// # Arguments
    ///
    /// * `index` - A `usize` representing the staged index of the holon.
    ///
    /// # Returns
    ///
    /// A new `StagedReference` wrapping the provided index.
    pub fn from_index(index: usize) -> Self {
        StagedReference { holon_index: index }
    }
    fn get_rc_holon(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Rc<RefCell<Holon>>, HolonError> {
        debug!("Entered: get_rc_holon, trying to get the space_manager");
        // Borrow the space manager immutably
        let space_manager = context.get_space_manager();

        debug!("Space manager borrowed successfully");

        // // Attempt to get the holon at the specified index
        // let rc_holon = &space_manager.get_holon_by_index(self.holon_index)?;
        // // Return a clone of the holon reference
        // Ok(rc_holon.clone())

        // Downcast to NurseryAccess
        let nursery_access = space_manager
            .as_any()
            .downcast_ref::<&dyn NurseryAccess>() // Downcast to NurseryAccess
            .ok_or_else(|| {
                error!("Failed to downcast space_manager to NurseryAccess");
                HolonError::DowncastFailure("NurseryAccess".to_string())
            })?;

        // Retrieve the holon by its index
        let rc_holon = nursery_access.get_holon_by_index(self.holon_index)?;

        // Return a clone of the Rc<RefCell<Holon>>
        Ok(rc_holon.clone())
    }
}

impl HolonReadable for StagedReference {
    fn get_property_value(
        &self,
        context: &dyn HolonsContextBehavior,
        property_name: &PropertyName,
    ) -> Result<PropertyValue, HolonError> {
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
        let mut holon = rc_holon.borrow_mut();
        let map = Rc::clone(&holon.get_related_holons(relationship_name)?);
        Ok(map)
    }

    fn essential_content(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<EssentialHolonContent, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();
        borrowed_holon.essential_content()
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
        debug!("Entered: abandon_staged_changes for staged_index: {:#?}", self.holon_index);
        // Get mutable access to the source holon
        let holon_refcell = self.get_rc_holon(context)?;

        // Borrow the holon from the RefCell
        let mut holon = holon_refcell.borrow_mut();

        debug!("borrowed mut for holon: {:#?}", self.holon_index);

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
        // Ensure is accessible for Write
        self.is_accessible(context, AccessType::Write)?;

        // Get mutable access to the source holon
        let rc_holon = self.get_rc_holon(context)?;

        // Borrow the holon from the RefCell
        let mut holon = rc_holon.borrow_mut();
        trace!(
            "Here is the RelationshipMap before adding related Holons: {:#?} \n\n",
            holon.relationship_map
        );
        debug!("In StagedReference::add_related_holons, getting collection for relationship name");

        debug!("In StagedReference::add_related_holons, about to add the holons to the editable collections:");

        // Retrieve the editable collection for the specified relationship name
        if let Some(collection) = holon.relationship_map.0.get_mut(&relationship_name) {
            debug!("Collection after to_staged: {:?}", collection);
            collection.add_references(context, holons)?;
        } else {
            let mut collection = HolonCollection::new_staged();
            collection.add_references(context, holons)?;
            holon.relationship_map.0.insert(relationship_name, collection);
        }
        debug!(
            "Here is the RelationshipMap after adding related Holons: {:#?}",
            holon.relationship_map
        );

        Ok(())
    }

    fn commit(&self, context: &dyn HolonsContextBehavior) -> Result<Holon, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let mut borrowed_holon = rc_holon.borrow_mut();
        borrowed_holon.commit()
    }

    fn clone_reference(&self) -> StagedReference {
        StagedReference { holon_index: self.holon_index.clone() }
    }

    fn get_id(&self, context: &dyn HolonsContextBehavior) -> Result<HolonId, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();
        if borrowed_holon.state == HolonState::Saved {
            Ok(HolonId::from(borrowed_holon.get_local_id()?))
        } else {
            Err(HolonError::NotAccessible("Id".to_string(), format!("{:?}", borrowed_holon.state)))
        }
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
        // Ensure is accessible for Write
        self.is_accessible(context, AccessType::Write)?;

        // Get mutable access to the source holon
        let rc_holon = self.get_rc_holon(context)?;

        // Borrow the holon from the RefCell
        let mut holon = rc_holon.borrow_mut();
        debug!(
            "In StagedReference::remove_related_holons, getting collection for relationship name"
        );

        debug!("In StagedReference::remove_related_holons, about to remove the holons from the editable collections:");

        // Retrieve the editable collection for the specified relationship name
        if let Some(collection) = holon.relationship_map.0.get_mut(&relationship_name) {
            collection.is_accessible(AccessType::Write)?;
            collection.remove_references(context, holons)?;
        } else {
            return Err(HolonError::InvalidRelationship(
                format!("Invalid relationship: {}", &relationship_name),
                format!("For holon {:?}", holon),
            ));
        }
        Ok(())
    }

    /// Stages a new Holon by cloning an existing Holon, without retaining lineage to the Holon its cloned from.
    fn stage_new_from_clone(
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
        value: BaseValue,
    ) -> Result<&Self, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let mut holon_refcell = rc_holon.borrow_mut();

        // Call the Holon's with_property_value method
        holon_refcell.with_property_value(property, value)?;

        Ok(self)
    }
}
