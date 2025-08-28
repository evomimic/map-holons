use derive_new::new;
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, fmt, rc::Rc, sync::Arc};
use tracing::trace;
use type_names::relationship_names::CoreRelationshipTypeName;

use crate::{
    core_shared_objects::{
        cache_access::HolonCacheAccess, holon::{state::AccessType, EssentialHolonContent}, transient_holon_manager::ToTransientHolon, Holon, HolonBehavior, HolonCollection, TransientHolon
    },
    reference_layer::{
        HolonReference, HolonsContextBehavior, ReadableHolon, ReadableHolonReferenceLayer, TransientReference,
    }, RelationshipMap,
};
use base_types::MapString;
use core_types::{HolonError, HolonId};
use integrity_core_types::{HolonNodeModel, PropertyMap, PropertyName, PropertyValue, RelationshipName};

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

    // *************** ACCESSORS ***************

    /// Outside helper method for serialization purposes, that does not require a context.
    pub fn get_id(&self) -> Result<HolonId, HolonError> {
        Ok(self.holon_id.clone())
    }

    pub fn get_smart_properties(&self) -> Option<PropertyMap> {
        self.smart_property_values.clone()
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

impl ReadableHolonReferenceLayer for SmartReference {
    fn clone_holon(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<TransientHolon, HolonError> {
        self.clone_into_transient(context)
    }

    fn get_all_related_holons(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<RelationshipMap, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();

        Ok(RelationshipMap::from(borrowed_holon.into_staged()?.get_staged_relationship_map()?))
    }

    fn get_holon_id(&self, _context: &dyn HolonsContextBehavior) -> Result<HolonId, HolonError> {
        Ok(self.holon_id.clone())
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

    /// `get_property_value` returns the value for the specified property name
    /// It will attempt to get it from the smart_property_values map first to avoid having to
    /// retrieve the underlying holon. But, failing that, it will do a get_rc_holon from the cache
    /// manager in the context.
    ///
    /// Returns: Option, None if property for given name does not exist in its PropertyMap.
    fn get_property_value(
        &self,
        context: &dyn HolonsContextBehavior,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError> {
        // Check if the property value is available in smart_property_values
        if let Some(smart_map) = &self.smart_property_values {
            if let Some(value) = smart_map.get(property_name) {
                return Ok(Some(value.clone()));
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
            if let Some(inner_value) =
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

    fn get_related_holons_ref_layer(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError> {
        // Get CacheAccess
        let cache_access = self.get_cache_access(context);
        cache_access.get_related_holons(&self.holon_id, relationship_name)
    }

    fn get_versioned_key(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<MapString, HolonError> {
        let holon = self.get_rc_holon(context)?;
        let key = holon.borrow().get_versioned_key()?;

        Ok(key)
    }

    fn essential_content(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<EssentialHolonContent, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();
        borrowed_holon.essential_content()
    }

    fn into_model(&self, context: &dyn HolonsContextBehavior) -> Result<HolonNodeModel, HolonError> {
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


impl ToTransientHolon for SmartReference {
    fn clone_into_transient(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<TransientHolon, HolonError> {
        // Get access for space_manager.transient_manager via HolonSpaceBehavior, TransientHolonBehavior
        let transient_manager_access = context.get_space_manager().get_transient_behavior_service();
        let transient_manager = transient_manager_access.borrow();

        // Create TransientHolon from Node data and add to transient manager
        let transient_reference = transient_manager.create_from_model(self.into_model(context)?)?;

        // Retrieve rc_holon by temporary id
        let transient_manager_access = TransientReference::get_transient_manager_access(context);
        let transient_manager = transient_manager_access.borrow();
        let transient_holon =
            transient_manager.get_holon_by_id(&transient_reference.get_temporary_id())?;
        let mut rc_holon = transient_holon.borrow_mut();

        // Bump version for tracking un-persisted holons, does an is_accessible check
        rc_holon.increment_version()?;

        // Clone relationships
        match &mut *rc_holon {
            Holon::Transient(transient_holon) => {
                // TODO: fetch relationships
            }
            _ => {
                return Err(HolonError::InvalidHolonReference(format!(
                    "Expected TransientHolon, got: {:#?}",
                    rc_holon
                )))
            }
        }

        Ok(rc_holon.clone().into_transient()?)
    }
}