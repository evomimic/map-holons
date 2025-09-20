use derive_new::new;
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, fmt, rc::Rc, sync::Arc};
use tracing::trace;
use type_names::relationship_names::CoreRelationshipTypeName;

use crate::reference_layer::readable_impl::ReadableHolonImpl;
use crate::reference_layer::writable_impl::WritableHolonImpl;
use crate::{
    core_shared_objects::{
        cache_access::HolonCacheAccess,
        holon::{state::AccessType, EssentialHolonContent, HolonCloneModel},
        relationship_behavior::ReadableRelationship,
        transient_holon_manager::ToHolonCloneModel,
        Holon, HolonBehavior, HolonCollection,
    },
    reference_layer::{HolonReference, HolonsContextBehavior, ReadableHolon, TransientReference},
    RelationshipMap,
};
use base_types::{BaseValue, MapString};
use core_types::{
    HolonError, HolonId, HolonNodeModel, PropertyMap, PropertyName, PropertyValue, RelationshipName,
};

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

impl ReadableHolonImpl for SmartReference {
    fn clone_holon_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<TransientReference, HolonError> {
        self.is_accessible(context, AccessType::Clone)?;
        let transient_behavior_service =
            context.get_space_manager().get_transient_behavior_service();
        let transient_behavior = transient_behavior_service.borrow();

        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();

        // HolonCloneModel for SavedHolon will have 'None' for relationships, as populating its RelationshipMap
        // is deferred to the reference layer, because context is needed that is only available in reference layer.
        let cloned_holon_transient_reference =
            transient_behavior.new_from_clone_model(borrowed_holon.get_holon_clone_model())?;

        let relationships = self.all_related_holons(context)?;

        cloned_holon_transient_reference
            .update_relationship_map(context, relationships.clone_for_new_source()?)?;

        Ok(cloned_holon_transient_reference)
    }

    fn all_related_holons_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<RelationshipMap, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let cache_access = self.get_cache_access(context);
        let relationship_map = cache_access.get_all_related_holons(context, &self.get_id()?)?;

        Ok(relationship_map)
    }

    fn holon_id_impl(&self, context: &dyn HolonsContextBehavior) -> Result<HolonId, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        Ok(self.holon_id.clone())
    }

    fn predecessor_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<HolonReference>, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let collection = self.related_holons(context, CoreRelationshipTypeName::Predecessor)?;
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
    fn property_value_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
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
    fn key_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<MapString>, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        // Since smart_property_values is an optional PropertyMap, first check to see if one exists.
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
        // Then if not, check the reference.
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

    fn related_holons_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        // Get CacheAccess
        let cache_access = self.get_cache_access(context);
        cache_access.get_related_holons(&self.holon_id, relationship_name)
    }

    fn versioned_key_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<MapString, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let holon = self.get_rc_holon(context)?;
        let key = holon.borrow().get_versioned_key()?;

        Ok(key)
    }

    fn essential_content_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<EssentialHolonContent, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();
        borrowed_holon.essential_content()
    }

    fn into_model_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonNodeModel, HolonError> {
        self.is_accessible(context, AccessType::Read)?;
        let rc_holon = self.get_rc_holon(context)?;
        let borrowed_holon = rc_holon.borrow();

        Ok(borrowed_holon.into_node())
    }

    fn is_accessible_impl(
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

// Convenience trait implementation for working with HolonReference wrappers.
// Functions will always fail since SmartReferences are immutable.
impl WritableHolonImpl for SmartReference {
    fn add_related_holons_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        _relationship_name: RelationshipName,
        _holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        self.is_accessible(context, AccessType::Write)?;

        Ok(())
    }

    fn remove_related_holons_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        _relationship_name: RelationshipName,
        _holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        self.is_accessible(context, AccessType::Write)?;

        Ok(())
    }

    fn with_property_value_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        _property: PropertyName,
        _value: BaseValue,
    ) -> Result<(), HolonError> {
        self.is_accessible(context, AccessType::Write)?;

        Ok(())
    }

    fn remove_property_value_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        _name: PropertyName,
    ) -> Result<(), HolonError> {
        self.is_accessible(context, AccessType::Write)?;

        Ok(())
    }

    fn with_descriptor_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        _descriptor_reference: HolonReference,
    ) -> Result<(), HolonError> {
        self.is_accessible(context, AccessType::Write)?;

        Ok(())
    }

    fn with_predecessor_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        _predecessor_reference_option: Option<HolonReference>,
    ) -> Result<(), HolonError> {
        self.is_accessible(context, AccessType::Write)?;

        Ok(())
    }
}

impl ToHolonCloneModel for SmartReference {
    fn get_holon_clone_model(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonCloneModel, HolonError> {
        let rc_holon = self.get_rc_holon(context)?;
        let model = rc_holon.borrow().get_holon_clone_model();

        Ok(model)
    }
}
