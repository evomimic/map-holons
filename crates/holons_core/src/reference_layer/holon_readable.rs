use std::rc::Rc;

use super::HolonReference;
use crate::reference_layer::{HolonsContextBehavior, TransientReference};
use crate::{
    core_shared_objects::{
        holon::{state::AccessType, EssentialHolonContent},
        HolonCollection,
    },
    RelationshipMap,
};
use base_types::MapString;
use core_types::{HolonError, HolonId};
use integrity_core_types::{HolonNodeModel, PropertyName, PropertyValue, RelationshipName};
use type_names::relationship_names::ToRelationshipName;

pub trait ReadableHolonReferenceLayer {
    /// Generic clone for all Holon variants. Resulting clone is always a TransientReference, regardless of source phase.
    fn clone_holon(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<TransientReference, HolonError>;

    /// Populates a full RelationshipMap by retrieving all related Holons for the source HolonReference.
    /// The map returned will ONLY contain entries for relationships that have at least
    /// one related holon (i.e., none of the holon collections returned via the result map will have
    /// zero members).
    ///
    /// For Transient & Staged Holons, it fetches and converts their relationship map to the CollectionState agnostic RelationshipMap type.
    /// For a Saved Holon (SmartReference), it calls the GuestHolonService to fetch all Smartlinks.
    ///
    fn get_all_related_holons(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<RelationshipMap, HolonError>;

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
    fn get_related_holons_ref_layer(
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

    fn into_model(&self, context: &dyn HolonsContextBehavior)
        -> Result<HolonNodeModel, HolonError>;

    fn is_accessible(
        &self,
        context: &dyn HolonsContextBehavior,
        access_type: AccessType,
    ) -> Result<(), HolonError>;
}

pub trait ReadableHolon: ReadableHolonReferenceLayer {
    #[inline]
    fn get_related_holons<T: ToRelationshipName>(
        &self,
        context: &dyn HolonsContextBehavior,
        name: T,
    ) -> Result<Rc<HolonCollection>, HolonError> {
        let relationship_name = name.to_relationship_name();
        self.get_related_holons_ref_layer(context, &relationship_name)
    }
}

// Empty blanket impl: all logic is in the trait’s default body
impl<T: ReadableHolonReferenceLayer + ?Sized> ReadableHolon for T {}
