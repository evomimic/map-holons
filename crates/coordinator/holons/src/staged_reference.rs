use derive_new::new;
use hdk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

//use crate::commit_manager::StagedIndex;
use crate::context::HolonsContext;
use crate::holon::{AccessType, EssentialHolonContent, Holon, HolonState};
use crate::holon_collection::HolonCollection;
use crate::holon_error::HolonError;
use crate::holon_reference::{HolonGettable, HolonReference};
use crate::relationship::{RelationshipMap, RelationshipName};
use crate::space_manager::HolonStageQuery;
use shared_types_holon::holon_node::PropertyName;

use shared_types_holon::{BaseValue, HolonId, MapString, PropertyValue};
/// a StagedIndex identifies a StagedHolon by its position within the staged_holons vector
pub type StagedIndex = usize;

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct StagedReference {
    // pub rc_holon: Rc<RefCell<Holon>>, // Ownership moved to CommitManager
    pub holon_index: StagedIndex, // the position of the holon with CommitManager's staged_holons vector
}
impl HolonGettable for StagedReference {
    fn get_property_value(
        &self,
        context: &HolonsContext,
        property_name: &PropertyName,
    ) -> Result<PropertyValue, HolonError> {
        let binding = context.space_manager.borrow();
        let holon = binding.get_holon(&self)?;
        let borrowedholon = holon.try_borrow()
        .map_err(|e| {
            HolonError::FailedToBorrow(format!("Unable to borrow holon immutably: {}", e))
        })?;
        borrowedholon.get_property_value(property_name)
    }

    fn get_key(&self, context: &HolonsContext) -> Result<Option<MapString>, HolonError> {
        let binding = context.space_manager.borrow();
        let holon = binding.get_holon(&self)?;
        let borrowedholon = holon.try_borrow()
        .map_err(|e| {
            HolonError::FailedToBorrow(format!("Unable to borrow holon immutably: {}", e))
        })?;
        borrowedholon.get_key().clone()
    }

    // Populates the cached source holon's HolonCollection for the specified relationship if one is provided.
    // If relationship_name is None, the source holon's HolonCollections are populated for all relationships that have related holons.
    fn get_related_holons(
        &self,
        context: &HolonsContext,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError> {
        let holon = self.get_rc_holon(context)?;
        let map = {
            let mut holon_refcell = holon.borrow_mut();
            Rc::clone(&holon_refcell.get_related_holons(relationship_name)?)
        };
        Ok(map)
    }
}

impl StagedReference {
    pub fn abandon_staged_changes(&mut self, context: &HolonsContext) -> Result<(), HolonError> {
        debug!("Entered: abandon_staged_changes for staged_index: {:#?}", self.holon_index);
        // Get mutable access to the source holon
        let holon_refcell = self.get_rc_holon(context)?;

        // Borrow the holon from the RefCell
        let mut holon = holon_refcell.borrow_mut();

        debug!("borrowed mut for holon: {:#?}", self.holon_index);

        holon.abandon_staged_changes()?;

        Ok(())
    }

    pub fn add_related_holons(
        &self,
        context: &HolonsContext,
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

    pub fn commit(&self, context: &HolonsContext) -> Result<Holon, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let mut borrowed_holon = rc_holon.borrow_mut();
        borrowed_holon.commit()
    }

    pub fn clone_reference(&self) -> StagedReference {
        StagedReference { holon_index: self.holon_index.clone() }
    }

    pub fn essential_content(
        &self,
        context: &HolonsContext,
    ) -> Result<EssentialHolonContent, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();
        borrowed_holon.essential_content()
    }

    pub fn get_id(&self, context: &HolonsContext) -> Result<HolonId, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();
        if borrowed_holon.state == HolonState::Saved {
            Ok(HolonId::from(borrowed_holon.get_local_id()?))
        } else {
            Err(HolonError::NotAccessible("Id".to_string(), format!("{:?}", borrowed_holon.state)))
        }
    }

    pub fn get_predecessor(
        &self,
        context: &HolonsContext,
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

    pub fn get_rc_holon(&self, context: &HolonsContext) -> Result<Rc<RefCell<Holon>>, HolonError> {
        debug!("Entered: get_rc_holon, trying to get the space_manager");
        let space_manager = match context.space_manager.try_borrow() {
            Ok(space_manager) => space_manager,
            Err(borrow_error) => {
                error!(
                    "Failed to borrow space_manager, it is already borrowed mutably: {:?}",
                    borrow_error
                );
                return Err(HolonError::FailedToBorrow(format!("{:?}", borrow_error)));
            }
        };

        debug!("Space manager borrowed successfully");

        // Attempt to get the holon at the specified index
        let rc_holon = &space_manager.get_holon_by_index(self.holon_index)?; 
        // Return a clone of the holon reference
        Ok(rc_holon.clone())
    
    }

    pub fn get_relationship_map(
        &self,
        context: &HolonsContext,
    ) -> Result<RelationshipMap, HolonError> {
        let binding = context.space_manager.borrow();
        let holon = binding.get_holon(&self)?;
        let borrowedholon = holon.try_borrow()
        .map_err(|e| {
            HolonError::FailedToBorrow(format!("Unable to borrow holon immutably: {}", e))
        })?;
        Ok(borrowedholon.relationship_map.clone())
    }

    pub fn is_accessible(
        &self,
        context: &HolonsContext,
        access_type: AccessType,
    ) -> Result<(), HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let holon = rc_holon.borrow();
        holon.is_accessible(access_type)?;

        Ok(())
    }

    pub fn remove_related_holons(
        &self,
        context: &HolonsContext,
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
    pub fn stage_new_from_clone(&self, context: &HolonsContext) -> Result<Holon, HolonError> {
        let holon = self.get_rc_holon(context)?;
        let cloned_holon = holon.borrow().clone_holon()?;
        // cloned_holon.load_all_relationships(context)?;

        Ok(cloned_holon)
    }

    pub fn with_descriptor(
        &self,
        context: &HolonsContext,
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

    pub fn with_predecessor(
        &self,
        context: &HolonsContext,
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

    pub fn with_property_value(
        &self,
        context: &HolonsContext,
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
