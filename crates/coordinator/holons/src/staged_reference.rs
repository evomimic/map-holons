use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use derive_new::new;
use hdk::prelude::*;

use shared_types_holon::{MapString, PropertyValue};
use shared_types_holon::holon_node::PropertyName;

use crate::context::HolonsContext;
use crate::holon::{Holon, HolonFieldGettable};
use crate::holon_errors::HolonError;
use crate::holon_reference::HolonReference;
use crate::relationship::{RelationshipMap, RelationshipName, RelationshipTarget};
use crate::staged_collection::StagedCollection;

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct StagedReference {
    pub key : Option<MapString>,
    pub rc_holon : Rc<RefCell<Holon>>,
}

impl StagedReference {
    // Method to clone the underlying Holon object
    pub fn clone_holon(&self) -> Holon {
        self.rc_holon.borrow().clone()
    }
    pub fn add_related_holons(&self, relationship_name: RelationshipName, holons: Vec<HolonReference>) -> Result<(), HolonError> {
        // Ensure the existence of an editable collection for the specified relationship name
        self.ensure_editable_collection(relationship_name.clone())?;

        // Get mutable access to the holon
        if let Ok(mut holon_ref) = self.rc_holon.try_borrow_mut() {
            // Retrieve the editable collection for the specified relationship name
            let editable_collection = holon_ref.relationship_map
                .get_mut(&relationship_name)
                .and_then(|relationship_target| relationship_target.editable.as_mut());

            // Add the holons to the editable collection
            if let Some(collection) = editable_collection {
                collection.holons.extend(holons);
                Ok(())
            } else {
                Err(HolonError::UnableToAddHolons("to staged collection".to_string()))
            }
        } else {
            // Handle the case where borrowing mutably fails
            Err(HolonError::FailedToBorrowMutably("for StagedReference".to_string()))
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
    fn ensure_editable_collection(&self, relationship_name: RelationshipName) -> Result<bool, HolonError> {
        // Get mutable access to the holon
        if let Ok(mut holon_ref) = self.rc_holon.try_borrow_mut() {
            // Access the relationship map and ensure the existence of the editable collection
            holon_ref.relationship_map
                .entry(relationship_name)
                .or_insert_with(|| RelationshipTarget {
                    editable: Some(StagedCollection {
                        source_holon: Some(Rc::downgrade(&self.rc_holon)), // Convert to Weak reference
                        relationship_descriptor: None,
                        holons: Vec::new(),
                        keyed_index: BTreeMap::new(),
                    }),
                    cursors: Vec::new(),
                });
            Ok(true) // Return true indicating success
        } else {
            // Handle the case where borrowing mutably fails
            Err(HolonError::FailedToBorrowMutably("StagedReference".to_string()))
        }
    }
    pub fn get_relationship_map(&mut self, _context: &HolonsContext) -> Result<RelationshipMap, HolonError> {
        let holon = self.rc_holon.borrow();
        holon.get_relationship_map()
    }

}


impl HolonFieldGettable for StagedReference {
    fn get_property_value(&mut self, _context: &HolonsContext, property_name: &PropertyName) -> Result<PropertyValue, HolonError> {
        let holon = self.rc_holon.borrow();
        holon.get_property_value( property_name)
    }

    fn get_key(&mut self, _context: &HolonsContext) -> Result<Option<MapString>, HolonError> {
        let holon = self.rc_holon.borrow();
        holon.get_key().clone()
    }
}
// Constructor function for creating from Holon Reference
pub fn from_holon(rc_holon: Rc<RefCell<Holon>>) -> Result<StagedReference, HolonError> {
    let key = rc_holon.borrow().get_key()?;

    Ok(StagedReference {
        key,
        rc_holon,
    })
}






