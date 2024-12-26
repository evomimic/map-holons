use std::rc::Rc;

use shared_types_holon::{MapString, PropertyName, PropertyValue};

use crate::{
    context::HolonsContext,
    holon::{AccessType, EssentialHolonContent},
    holon_collection::HolonCollection,
    holon_error::HolonError,
    relationship::RelationshipName,
};

pub trait HolonReadable {
    fn get_property_value(
        &self,
        context: &HolonsContext,
        property_name: &PropertyName,
    ) -> Result<PropertyValue, HolonError>;

    /// This function returns the primary key value for the holon or None if there is no key value
    /// for this holon (NOTE: Not all holon types have defined keys.)
    /// If the holon has a key, but it cannot be returned as a MapString, this function
    /// returns a HolonError::UnexpectedValueType.
    fn get_key(&self, context: &HolonsContext) -> Result<Option<MapString>, HolonError>;

    // Populates the cached source holon's HolonCollection for the specified relationship if one is provided.
    // If relationship_name is None, the source holon's HolonCollections are populated for all relationships that have related holons.
    fn get_related_holons(
        &self,
        context: &HolonsContext,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError>;

    fn essential_content(
        &self,
        context: &HolonsContext,
    ) -> Result<EssentialHolonContent, HolonError>;

    fn is_accessible(
        &self,
        context: &HolonsContext,
        access_type: AccessType,
    ) -> Result<(), HolonError>;
}
