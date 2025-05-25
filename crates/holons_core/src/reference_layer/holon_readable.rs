use std::rc::Rc;

use crate::core_shared_objects::holon::TransientHolon;
use crate::reference_layer::HolonsContextBehavior;

use crate::core_shared_objects::{
    holon::{state::AccessType, holon_utils::EssentialHolonContent}, HolonCollection, HolonError, RelationshipName,
};

use shared_types_holon::{HolonId, MapString, PropertyName, PropertyValue};

use super::HolonReference;

pub trait HolonReadable {
    fn clone_holon(&self, context: &dyn HolonsContextBehavior) -> Result<TransientHolon, HolonError>;

    /// Generally used to get a Holon id for a SmartReference, but will also return a Holon id for a StagedReference if the staged Holon has been committed.
    fn get_holon_id(&self, context: &dyn HolonsContextBehavior) -> Result<HolonId, HolonError>;

    fn get_predecessor(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<HolonReference>, HolonError>;

    /// Returns the value for the specified property
    fn get_property_value(
        &self,
        context: &dyn HolonsContextBehavior,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError>;

    /// This function returns the primary key value for the holon or None if there is no key value
    /// for this holon (NOTE: Not all holon types have defined keys.)
    /// If the holon has a key, but it cannot be returned as a MapString, this function
    /// returns a HolonError::UnexpectedValueType.
    fn get_key(&self, context: &dyn HolonsContextBehavior)
        -> Result<Option<MapString>, HolonError>;

    /// Retrieves the collection of holons related to the referenced holon via the specified relationship.
    ///
    /// This method fetches the set of holons that are connected to the holon referenced by the implementer
    /// through the given `relationship_name`. The relationships are resolved using the provided `context`.
    ///
    /// # Parameters
    /// - `context`: A reference to an object implementing the `HolonsContextBehavior` trait, which provides
    ///   the necessary context for resolving holon relationships.
    /// - `relationship_name`: The name of the relationship to query, represented by a `RelationshipName`.
    ///
    /// # Returns
    /// - `Ok(Rc<HolonCollection>)`: A reference-counted `HolonCollection` containing HolonReferences
    ///   to the related holons. If no holons are related via the specified relationship,
    ///   an empty `HolonCollection` is returned.
    /// - `Err(HolonError)`: An error indicating why the retrieval of related holons failed (e.g., invalid
    ///   relationship name, context-related errors).
    ///
    /// # Notes
    /// - The method ensures that the returned `HolonCollection` is never `None`; it is guaranteed to
    ///   contain either related holons or be empty.

    fn get_related_holons(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError>;

    fn get_versioned_key(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<MapString, HolonError>;

    fn essential_content(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<EssentialHolonContent, HolonError>;

    fn is_accessible(
        &self,
        context: &dyn HolonsContextBehavior,
        access_type: AccessType,
    ) -> Result<(), HolonError>;
}
