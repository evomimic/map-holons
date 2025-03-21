use derive_new::new;
use hdk::prelude::*;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

use crate::reference_layer::{
    HolonReadable, HolonReference, HolonsContextBehavior, StagedReference,
};

use crate::core_shared_objects::cache_access::HolonCacheAccess;
use crate::core_shared_objects::{
    AccessType, EssentialHolonContent, Holon, HolonCollection, HolonError, RelationshipName,
};
use shared_types_holon::holon_node::PropertyName;
use shared_types_holon::{HolonId, MapString, PropertyMap, PropertyValue};

#[derive(new, Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SmartReference {
    holon_id: HolonId,
    smart_property_values: Option<PropertyMap>,
}

impl SmartReference {
    // *************** CONSTRUCTORS ***************

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

    // *************** ACCESSORS ***************

    /// Outside helper method for serialization purposes, that does not require a context.
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

    /// Stages a new version of an existing holon for update, retaining the linkage to the holon
    /// version it is derived from by creating a PREDECESSOR relationship.
    #[deprecated]
    pub fn stage_new_version_deprecated(
        &self,
        _context: &dyn HolonsContextBehavior,
    ) -> Result<StagedReference, HolonError> {
        // let rc_holon = self.get_rc_holon(context)?;
        //
        // let mut cloned_holon = rc_holon.borrow().new_version()?;
        // cloned_holon.fetch_all_populated(context)?;
        //
        // trace!(
        //     "Entering SmartReference::stage_new_version, here is the Cloned Holon: {:#?}",
        //     cloned_holon
        // );
        //
        // let new_version_staged_reference =
        //     { context.get_space_manager().stage_new_holon(cloned_holon)? };
        //
        // // Set PREDECESSOR
        // new_version_staged_reference
        //     .with_predecessor(context, Some(HolonReference::Smart(self.clone())))?;
        //
        // Ok(new_version_staged_reference)
        todo!()
    }

    /// Stages a new Holon by cloning an existing Holon from its HolonReference, without retaining
    /// lineage to the Holon its cloned from.
    #[deprecated]
    pub fn stage_new_from_clone_deprecated(
        &self,
        _context: &dyn HolonsContextBehavior,
    ) -> Result<Holon, HolonError> {
        // let rc_holon = self.get_rc_holon(context)?;
        // let cloned_holon = rc_holon.borrow().clone_holon()?;
        // cloned_holon.load_all_relationships(context)?;
        //
        // Ok(cloned_holon)
        todo!()
    }

    // *************** UTILITY METHODS ***************

    fn get_cache_access(&self, context: &dyn HolonsContextBehavior) -> Arc<dyn HolonCacheAccess> {
        // Retrieve the space manager from the context
        let space_manager = context.get_space_manager();

        // Get CacheAccess
        space_manager.get_cache_access()
    }
    fn get_rc_holon(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Rc<RefCell<Holon>>, HolonError> {
        // Get CacheAccess
        let cache_access = self.get_cache_access(context);

        // Retrieve the holon from the cache
        let rc_holon = cache_access.get_rc_holon(&self.holon_id)?;
        trace!("Got a reference to rc_holon from the cache manager: {:#?}", rc_holon);

        Ok(rc_holon)
    }
}

impl fmt::Display for SmartReference {
    /// Formats the `SmartReference` for human-readable display.
    ///
    /// The output includes the `HolonId` and a summary of smart property values:
    /// - If `smart_property_values` is `None` or empty, displays `"no props"`.
    /// - If there are 1–2 properties, displays them as key-value pairs.
    /// - If there are more than 2 properties, displays the first two followed by `"+N more"`.
    ///
    /// # Example Outputs
    /// - `SmartReference(Local(…ABC123), no props)`
    /// - `SmartReference(Local(…ABC123), props: [key1:value1, key2:value2])`
    /// - `SmartReference(Local(…ABC123), props: [key1:value1, key2:value2 +1 more])`
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.smart_property_values {
            Some(props) if !props.is_empty() => {
                // Display the first 2 properties as a preview, followed by a count if there are more
                let preview: Vec<String> = props
                    .iter()
                    .take(2)
                    .map(|(key, value)| format!("{}:{:?}", key, value))
                    .collect();

                let additional_count = props.len().saturating_sub(preview.len());

                if additional_count > 0 {
                    write!(
                        f,
                        "SmartReference({}, props: [{} +{} more])",
                        self.holon_id,
                        preview.join(", "),
                        additional_count
                    )
                } else {
                    write!(f, "SmartReference({}, props: [{}])", self.holon_id, preview.join(", "))
                }
            }
            _ => write!(f, "SmartReference({}, no props)", self.holon_id),
        }
    }
}

impl HolonReadable for SmartReference {
    fn clone_holon(&self, context: &dyn HolonsContextBehavior) -> Result<Holon, HolonError> {
        let holon = self.get_rc_holon(context)?;
        let holon_borrow = holon.borrow();
        holon_borrow.clone_holon()
    }

    fn get_holon_id(&self, _context: &dyn HolonsContextBehavior) -> Result<HolonId, HolonError> {
        Ok(self.holon_id.clone())
    }

    /// `get_property_value` returns the value for the specified property name
    /// It will attempt to get it from the smart_property_values map first to avoid having to
    /// retrieve the underlying holon. But, failing that, it will do a get_rc_holon from the cache
    /// manager in the context.
    ///
    /// Possible Errors:
    /// This function returns an EmptyFiled error if no value is found for the specified property
    /// Or (less likely) an InvalidHolonReference
    fn get_property_value(
        &self,
        context: &dyn HolonsContextBehavior,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError> {
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

    /// This function extracts the key from the smart_property_values or, if not found there, from
    /// its referenced holon. Returns an Option -- as even though an entry for 'key' may be present in the BTreeMap, the value could be None.
    ///
    fn get_key(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<MapString>, HolonError> {
        // Since smart_property_values is an optional PropertyMap, first check to see if one exists..
        if let Some(smart_property_values) = self.smart_property_values.clone() {
            // If found, do a get on the PropertyMap to see if it contains a value.
            if let Some(Some(inner_value)) =
                smart_property_values.get(&PropertyName(MapString("key".to_string())))
            {
                // Try to convert a Some value to String, throws an error on failure because all values for the Key 'key' should be MapString.
                let string_value: String = inner_value.try_into().map_err(|_| {
                    HolonError::UnexpectedValueType(
                        format!("{:?}", inner_value),
                        "MapString".to_string(),
                    )
                })?;
                Ok(Some(MapString(string_value)))
            } else {
                Ok(None)
            }
        }
        // Then if not, check the reference..
        else {
            let holon = self.get_rc_holon(context)?;
            let key_option = holon.borrow().get_key()?;
            if let Some(key) = key_option {
                Ok(Some(key))
            } else {
                Ok(None)
            }
        }
    }

    fn get_related_holons(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError> {
        // Get CacheAccess
        let cache_access = self.get_cache_access(context);
        cache_access.get_related_holons(&self.holon_id, relationship_name)
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
