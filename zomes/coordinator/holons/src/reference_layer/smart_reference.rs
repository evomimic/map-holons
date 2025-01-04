use std::cell::RefCell;
use std::rc::Rc;

use derive_new::new;
use hdk::prelude::*;

use crate::reference_layer::{
    HolonReadable, HolonReference, HolonWritable, HolonsContextBehavior, StagedReference,
};

use crate::shared_objects_layer::cache_access::HolonCacheAccess;
use crate::shared_objects_layer::{
    AccessType, EssentialHolonContent, Holon, HolonCollection, HolonError, RelationshipMap,
    RelationshipName,
};
use shared_types_holon::holon_node::PropertyName;
use shared_types_holon::{HolonId, MapString, PropertyMap, PropertyValue};

#[hdk_entry_helper]
#[derive(new, Clone, PartialEq, Eq)]
pub struct SmartReference {
    //holon_space_id: Option<SpaceId>
    holon_id: HolonId,
    smart_property_values: Option<PropertyMap>,
}

impl SmartReference {
    /// Constructor for SmartReference that takes a HolonId and sets smart_property_values to None
    pub fn new_from_id(holon_id: HolonId) -> Self {
        SmartReference { holon_id, smart_property_values: None }
    }
    pub fn clone_reference(&self) -> SmartReference {
        SmartReference {
            holon_id: self.holon_id.clone(),
            smart_property_values: self.smart_property_values.clone(),
        }
    }

    pub fn get_id(&self) -> Result<HolonId, HolonError> {
        Ok(self.holon_id.clone())
    }

    pub fn get_predecessor(
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

    pub fn get_property_map(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<PropertyMap, HolonError> {
        let holon = self.get_rc_holon(context)?;
        let holon_refcell = holon.borrow();
        Ok(holon_refcell.property_map.clone())
    }
    fn get_rc_holon(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Rc<RefCell<Holon>>, HolonError> {
        debug!("Entered: get_rc_holon, trying to get the space_manager");

        // Retrieve the space manager from the context
        let space_manager = context.get_space_manager();

        // Attempt to downcast the space manager to HolonCacheAccess
        let cache_access =
            space_manager.as_any().downcast_ref::<&dyn HolonCacheAccess>().ok_or_else(|| {
                error!("Failed to downcast space_manager to HolonCacheAccess");
                HolonError::DowncastFailure("HolonCacheAccess".to_string())
            })?;

        debug!("Successfully downcasted to HolonCacheAccess");

        // Retrieve the holon from the cache
        let rc_holon = cache_access.get_rc_holon(&self.holon_id)?;
        trace!("Got a reference to rc_holon from the cache manager: {:#?}", rc_holon);

        Ok(rc_holon)
    }

    // Private function for getting a mutable reference from the context
    // fn get_rc_holon(
    //     &self,
    //     context: &dyn HolonsContextBehavior,
    // ) -> Result<Rc<RefCell<Holon>>, HolonError> {
    //     debug!("Entered: get_rc_holon, trying to get the cache_manager");
    //     let space_manager = match context.get_space_manager_mut() {
    //         Ok(space_manager) => space_manager,
    //         Err(borrow_error) => {
    //             error!(
    //                 "Failed to borrow cache_manager, it is already borrowed mutably: {:?}",
    //                 borrow_error
    //             );
    //             return Err(HolonError::FailedToBorrow(format!("{:?}", borrow_error)));
    //         }
    //     };
    //     debug!("Cache manager borrowed successfully");
    //
    //     let rc_holon = space_manager.get_rc_holon(&self.holon_id)?;
    //     trace!("Got a reference to rc_holon from the cache manager: {:#?}", rc_holon);
    //
    //     Ok(rc_holon)
    // }

    pub fn get_relationship_map(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<RelationshipMap, HolonError> {
        let holon = self.get_rc_holon(context)?;
        let holon_refcell = holon.borrow();
        Ok(holon_refcell.relationship_map.clone())
    }

    pub fn get_smart_properties(&self) -> Option<PropertyMap> {
        self.smart_property_values.clone()
    }

    // /// Populates a full RelationshipMap by retrieving all SmartLinks for which this SmartReference is the
    // /// source. The map returned will ONLY contain entries for relationships that have at least
    // /// one related holon (i.e., none of the holon collections returned via the result map will have
    // /// zero members).
    // pub fn get_all_related_holons(&self) -> Result<RelationshipMap, HolonError> {
    //     let mut relationship_map: BTreeMap<RelationshipName, HolonCollection> = BTreeMap::new();

    //     let mut reference_map: BTreeMap<RelationshipName, Vec<HolonReference>> = BTreeMap::new();
    //     let smartlinks = get_all_relationship_links(self.get_local_id()?)?;
    //     debug!("Retrieved {:?} smartlinks", smartlinks.len());

    //     for smartlink in smartlinks {
    //         let reference = smartlink.to_holon_reference();

    //         // The following:
    //         // 1) adds an entry for relationship name if not already present (via `entry` API)
    //         // 2) adds a value (Vec<HolonReference>) for the entry, if not already present (`.or_insert_with`)
    //         // 3) pushes the new HolonReference into the vector -- without having to clone the vector

    //         reference_map
    //             .entry(smartlink.relationship_name)
    //             .or_insert_with(Vec::new)
    //             .push(reference);
    //     }

    //     // Now create the result

    //     for (map_name, holon_reference) in reference_map {
    //         let mut collection = HolonCollection::new_existing();
    //         let key = holon_reference.get_key()?.ok_or_else(|| {
    //             HolonError::Misc("Expected Smartlink to have a key, didn't get one.".to_string())
    //         })?; // At least for now, all SmartLinks should be encoded with a key
    //         collection.add_reference_with_key(key, holon_reference)?;
    //         relationship_map.insert(map_name, collection);
    //     }

    //     Ok(relationship_map)
    // }

    /// Stages a new version of an existing holon for update, retaining the linkage to the holon version it is derived from by creating a PREDECESSOR relationship.
    pub fn stage_new_version(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<StagedReference, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;

        let mut cloned_holon = rc_holon.borrow().new_version()?;
        cloned_holon.load_all_relationships(context)?;

        trace!(
            "Entering SmartReference::stage_new_version, here is the Cloned Holon: {:#?}",
            cloned_holon
        );

        let new_version_staged_reference =
            { context.get_space_manager().stage_new_holon(cloned_holon)? };

        // Set PREDECESSOR
        new_version_staged_reference
            .with_predecessor(context, Some(HolonReference::Smart(self.clone())))?;

        Ok(new_version_staged_reference)
    }

    /// Stages a new Holon by cloning an existing Holon from its HolonReference, without retaining lineage to the Holon its cloned from.
    pub fn stage_new_from_clone(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Holon, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let cloned_holon = rc_holon.borrow().clone_holon()?;
        // cloned_holon.load_all_relationships(context)?;

        Ok(cloned_holon)
    }
}

impl HolonReadable for SmartReference {
    /// This function gets the value for the specified property name
    /// It will attempt to get it from the smart_property_values map first to avoid having to
    /// retrieve the underlying holon. But, failing that, it will do a get_rc_holon from the cache manager in the context.
    ///
    /// Possible Errors:
    /// This function returns an EmptyFiled error if no value is found for the specified property
    /// Or (less likely) an InvalidHolonReference
    fn get_property_value(
        &self,
        context: &dyn HolonsContextBehavior,
        property_name: &PropertyName,
    ) -> Result<PropertyValue, HolonError> {
        // Check if the property value is available in smart_property_values
        if let Some(smart_map) = &self.smart_property_values {
            if let Some(value) = smart_map.get(property_name) {
                return Ok(value.clone());
            }
        }

        // Get rc_holon from HolonCacheManager
        let holon = self.get_rc_holon(context)?;
        let prop_val = holon.borrow().get_property_value(property_name)?;
        Ok(prop_val)
    }

    /// This function extracts the key from the smart_property_values,
    ///
    fn get_key(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<MapString>, HolonError> {
        return if let Some(smart_prop_vals) = self.smart_property_values.clone() {
            let key_option = smart_prop_vals.get(&PropertyName(MapString("key".to_string())));
            if let Some(key) = key_option {
                Ok(Some(MapString(key.into())))
            } else {
                let holon = self.get_rc_holon(context)?;
                let key = holon.borrow().get_key()?;
                Ok(key)
            }
        } else {
            let holon = self.get_rc_holon(context)?;
            let key = holon.borrow().get_key()?;
            Ok(key)
        };
    }

    fn get_related_holons(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError> {
        let holon = self.get_rc_holon(context)?;
        let map = {
            let mut holon_refcell = holon.try_borrow_mut().map_err(|e| {
                HolonError::FailedToBorrow(format!("Unable to borrow holon mutably: {}", e))
            })?;
            holon_refcell.get_related_holons(relationship_name)?.clone()
        };
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
