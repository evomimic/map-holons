use derive_new::new;
use hdk::prelude::*;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::commit_manager::StagedIndex;
use crate::context::HolonsContext;
use crate::holon::{AccessType, Holon, HolonFieldGettable};
use crate::holon_error::HolonError;
use crate::holon_reference::HolonReference;
use crate::relationship::{RelationshipMap, RelationshipName, RelationshipTarget};
use crate::staged_collection::StagedCollection;
use shared_types_holon::holon_node::PropertyName;
use shared_types_holon::{MapString, PropertyValue};

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct StagedReference {
    pub key: Option<MapString>,
    // pub rc_holon: Rc<RefCell<Holon>>, // Ownership moved to CommitManager
    pub holon_index: StagedIndex, // the position of the holon with CommitManager's staged_holons vector
}

impl StagedReference {
    // Constructor function for creating StagedReference index into CommitManagers StagedHolons
    // pub fn from_holon(rc_holon: Rc<RefCell<Holon>>) -> Result<StagedReference, HolonError> {
    //     let key = rc_holon.borrow().get_key()?;
    //
    //     Ok(StagedReference { key, holon_index })
    // }

    // // Method to clone the underlying Holon object
    // pub fn clone_holon(&self, context: &HolonsContext) -> Holon {
    //     context.commit_manager.borrow().staged_holons[self.holon_index]
    //         .borrow()
    //         .clone()
    // }

    pub fn commit(&self, context: &HolonsContext) -> Result<Holon, HolonError> {
        let holon_ref = self.get_mut_holon(context)?;
        let mut borrowed_holon = holon_ref.borrow_mut();
        borrowed_holon.commit()
    }

    pub fn clone_reference(&self) -> StagedReference {
        StagedReference {
            key: self.key.clone(),
            holon_index: self.holon_index.clone(),
        }
    }

    pub fn add_related_holons(
        &self,
        context: &HolonsContext,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        debug!("Entered StagedReference::add_related_holons");

        // Ensure the existence of an editable collection for the specified relationship name
        self.ensure_editable_collection(context, relationship_name.clone())?;
        debug!("In StagedReference::add_related_holons, got the editable_collection");

        // Get mutable access to the source holon
        let holon_ref = self.get_mut_holon(context)?;

        // Borrow the holon from the RefCell
        let mut holon = holon_ref.borrow_mut();
        debug!("In StagedReference::add_related_holons, getting collection for relationship name");

        // Ensure is accessbile for Write
        holon.is_accessible(AccessType::Write)?;

        // Retrieve the editable collection for the specified relationship name
        let editable_collection = match holon.relationship_map.0.get_mut(&relationship_name) {
            Some(relationship_target) => relationship_target.editable.as_mut(),
            None => None,
        };
        debug!("In StagedReference::add_related_holons, about to add the holons to the editable collections:");

        // Add the holons to the editable collection
        if let Some(collection) = editable_collection {
            collection.holons.extend(holons);
            Ok(())
        } else {
            Err(HolonError::UnableToAddHolons(
                "to staged collection".to_string(),
            ))
        }
    }

    /// This function confirms that a RelationshipTarget with an editable collection has been created
    /// for the specified relationship. If so, it returns true.
    /// Otherwise, create a RelationshipTarget with an editable collection for this relationship, add it to the
    /// source_holon's relationship_map and return true.
    ///
    /// TODO: Add validation_status to either RelationshipTarget or StagedCollection and, before adding the
    /// RelationshipTarget, verify that a relationship with the specified relationship_name is valid for this holon type
    ///

    // HERE"S THE CHATGPT VERSION
    fn ensure_editable_collection(
        &self,
        context: &HolonsContext,
        relationship_name: RelationshipName,
    ) -> Result<bool, HolonError> {
        // Get mutable access to the holon
        let holon_ref = self.get_mut_holon(context)?;
        debug!("Got mutable holon reference in ensure_editable_collection");

        // Access the relationship map and ensure the existence of the editable collection
        let mut holon = holon_ref.borrow_mut();
        debug!("Borrowed holon mutably in ensure_editable_collection");

        holon
            .relationship_map
            .0
            .entry(relationship_name.clone())
            .or_insert_with(|| {
                debug!(
                    "Creating new editable collection for relationship: {:?}",
                    relationship_name
                );
                // Create a StagedCollection
                let staged_collection = StagedCollection {
                    source_holon: Some(self.clone()), // Set source_holon to a StagedReference to the same holon
                    relationship_descriptor: None,
                    holons: Vec::new(),
                    keyed_index: BTreeMap::new(),
                };

                // Return the RelationshipTarget with the created StagedCollection
                RelationshipTarget {
                    editable: Some(staged_collection),
                    cursors: Vec::new(),
                }
            });

        debug!(
            "ensure_editable_collection completed successfully for relationship: {:?}",
            relationship_name
        );
        Ok(true) // Return true indicating success
    }

    pub fn get_relationship_map(
        &mut self,
        context: &HolonsContext,
    ) -> Result<RelationshipMap, HolonError> {
        let binding = context.commit_manager.borrow();
        let holon = binding.get_holon(&self)?;
        Ok(holon.relationship_map.clone())
    }

    pub fn get_mut_holon(&self, context: &HolonsContext) -> Result<Rc<RefCell<Holon>>, HolonError> {
        debug!("Entered: get_mut_holon, trying to get the commit_manager");
        // let mut commit_manager = context.commit_manager.borrow_mut();
        // Attempt to borrow commit_manager mutably
        let mut commit_manager = match context.commit_manager.try_borrow_mut() {
            Ok(cm) => cm,
            Err(e) => {
                error!("Failed to borrow commit_manager mutably: {:?}", e);
                return Err(HolonError::FailedToBorrow(format!("{:?}", e)));
            }
        };

        debug!("Commit manager borrowed successfully");

        // Obtain the staged_holons vector from the CommitManager
        let staged_holons = &mut commit_manager.staged_holons;
        debug!("Got a reference to staged_holons from the commit manager");

        // Attempt to get the holon at the specified index
        if let Some(holon_ref) = staged_holons.get(self.holon_index) {
            // Return a clone of the holon reference
            Ok(holon_ref.clone())
        } else {
            // If index is out of range, return an error
            Err(HolonError::InvalidHolonReference(format!(
                "Invalid holon index: {}",
                self.holon_index
            )))
        }
    }

    pub fn abandon_staged_changes(&mut self, context: &HolonsContext) -> Result<(), HolonError> {
        // Get mutable access to the source holon
        let holon_ref = self.get_mut_holon(context)?;

        // Borrow the holon from the RefCell
        let mut holon = holon_ref.borrow_mut();

        holon.abandon_staged_changes();

        Ok(())
    }
}

impl HolonFieldGettable for StagedReference {
    fn get_property_value(
        &mut self,
        context: &HolonsContext,
        property_name: &PropertyName,
    ) -> Result<PropertyValue, HolonError> {
        let binding = context.commit_manager.borrow();
        let holon = binding.get_holon(&self)?;
        holon.get_property_value(property_name)
    }

    fn get_key(&mut self, context: &HolonsContext) -> Result<Option<MapString>, HolonError> {
        let binding = context.commit_manager.borrow();
        let holon = binding.get_holon(&self)?;
        holon.get_key().clone()
    }
}
