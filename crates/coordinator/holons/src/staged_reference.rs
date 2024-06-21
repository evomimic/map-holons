use derive_new::new;
use hdk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::commit_manager::StagedIndex;
use crate::context::HolonsContext;
use crate::holon::{AccessType, Holon, HolonGettable};
use crate::holon_collection::HolonCollection;
use crate::holon_error::HolonError;
use crate::holon_reference::HolonReference;
use crate::relationship::{RelationshipMap, RelationshipName};
use shared_types_holon::holon_node::PropertyName;

use shared_types_holon::{HolonId, MapString, PropertyValue};

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
        let binding = context.commit_manager.borrow();
        let holon = binding.get_holon(&self)?;
        holon.get_property_value(property_name)
    }

    fn get_key(&self, context: &HolonsContext) -> Result<Option<MapString>, HolonError> {
        let binding = context.commit_manager.borrow();
        let holon = binding.get_holon(&self)?;
        holon.get_key().clone()
    }
}

impl StagedReference {
    pub fn get_id(&self, context: &HolonsContext) -> Result<HolonId, HolonError> {
        let binding = context.commit_manager.borrow();
        let holon = binding.get_holon(&self)?;
        holon.get_id()
    }
    pub fn commit(&self, context: &HolonsContext) -> Result<Holon, HolonError> {
        let holon_ref = self.get_mut_holon(context)?;
        let mut borrowed_holon = holon_ref.borrow_mut();
        borrowed_holon.commit()
    }

    pub fn clone_reference(&self) -> StagedReference {
        StagedReference {
            holon_index: self.holon_index.clone(),
        }
    }


    /// Use this method to get a copy of the staged holon referenced by this StagedReference.
    /// NOTE: The cloned holon is NOT, itself, staged by the CommitManager
    pub fn clone_holon(&self, context: &HolonsContext) -> Result<Holon, HolonError> {
        let commit_manager = context.commit_manager
            .try_borrow()
            .map_err(|_| HolonError::FailedToBorrow("commit_manager".to_string()))?;

        let holon_rc = commit_manager.staged_holons.get(self.holon_index)
            .ok_or(HolonError::IndexOutOfRange(self.holon_index.to_string()))?;

        let holon_ref = holon_rc
            .try_borrow()
            .map_err(|_| HolonError::FailedToBorrow("holon".to_string()))?;

        Ok(holon_ref.clone())
    }


    pub fn add_related_holons(
        &self,
        context: &HolonsContext,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        debug!("Entered StagedReference::add_related_holons");

        // Get mutable access to the source holon
        let holon_ref = self.get_mut_holon(context)?;

        // Borrow the holon from the RefCell
        let mut holon = holon_ref.borrow_mut();
        debug!("In StagedReference::add_related_holons, getting collection for relationship name");

        // Ensure is accessible for Write
        holon.is_accessible(AccessType::Write)?;

        debug!("In StagedReference::add_related_holons, about to add the holons to the editable collections:");

        // Retrieve the editable collection for the specified relationship name
        if let Some(collection) = holon.relationship_map.0.get_mut(&relationship_name) {
            collection.is_accessible(AccessType::Write)?;
            collection.add_references(context, holons)?;
        } else {
            let mut collection = HolonCollection::new_staged();
            collection.is_accessible(AccessType::Write)?;
            collection.add_references(context, holons)?;
            holon
                .relationship_map
                .0
                .insert(relationship_name, collection);
        }

        Ok(())
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
        debug!(
            "Entered: abandon_staged_changes for staged_index: {:#?}",
            self.holon_index
        );
        // Get mutable access to the source holon
        let holon_ref = self.get_mut_holon(context)?;

        // Borrow the holon from the RefCell
        let mut holon = holon_ref.borrow_mut();

        debug!("borrowed mut for holon: {:#?}", self.holon_index);

        holon.abandon_staged_changes();

        Ok(())
    }
}
