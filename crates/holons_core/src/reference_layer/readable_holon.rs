use std::rc::Rc;

use super::{HolonReference, TransientReference};
use crate::reference_layer::readable_impl::ReadableHolonImpl;
use crate::reference_layer::HolonsContextBehavior;
use crate::{
    core_shared_objects::{
        holon::{state::AccessType, EssentialHolonContent},
        HolonCollection,
    },
    RelationshipMap,
};
use base_types::MapString;
use core_types::{HolonError, HolonId, HolonNodeModel, PropertyValue};
use type_names::relationship_names::ToRelationshipName;
use type_names::ToPropertyName;

// Façade: ergonomic + complete; default bodies delegate to *_impl.
pub trait ReadableHolon: ReadableHolonImpl {
    // Plain forwards
    /// Generic clone for all Holon variants. Resulting clone is always a TransientReference, regardless of source phase.
    fn clone_holon(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<TransientReference, HolonError> {
        ReadableHolonImpl::clone_holon_impl(self, context)
    }
    #[inline]
    fn essential_content(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<EssentialHolonContent, HolonError> {
        ReadableHolonImpl::essential_content_impl(self, context)
    }

    /// Returns a String summary of the Holon.
    ///
    /// -Only used for logging. Provides a more concise message to avoid log bloat.
    fn summarize(&self, context: &dyn HolonsContextBehavior) -> Result<String, HolonError> {
        ReadableHolonImpl::summarize_impl(self, context)
    }

    /// Generally used to get a Holon id for a SmartReference, but will also return a Holon id for a StagedReference if the staged Holon has been committed.
    #[inline]
    fn holon_id(&self, context: &dyn HolonsContextBehavior) -> Result<HolonId, HolonError> {
        ReadableHolonImpl::holon_id_impl(self, context)
    }

    #[inline]
    fn predecessor(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<HolonReference>, HolonError> {
        ReadableHolonImpl::predecessor_impl(self, context)
    }

    /// This function returns the primary key value for the holon or None if there is no key value
    /// for this holon (NOTE: Not all holon types have defined keys.)
    /// If the holon has a key, but it cannot be returned as a MapString, this function
    /// returns a HolonError::UnexpectedValueType.
    #[inline]
    fn key(&self, context: &dyn HolonsContextBehavior) -> Result<Option<MapString>, HolonError> {
        ReadableHolonImpl::key_impl(self, context)
    }

    #[inline]
    fn versioned_key(&self, context: &dyn HolonsContextBehavior) -> Result<MapString, HolonError> {
        ReadableHolonImpl::versioned_key_impl(self, context)
    }
    /// Populates a full RelationshipMap by retrieving all related Holons for the source HolonReference.
    /// The map returned will ONLY contain entries for relationships that have at least
    /// one related holon (i.e., none of the holon collections returned via the result map will have
    /// zero members).
    ///
    /// For Transient & Staged Holons, it fetches and converts their relationship map to the CollectionState agnostic RelationshipMap type.
    /// For a Saved Holon (SmartReference), it calls the GuestHolonService to fetch all Smartlinks.
    ///
    #[inline]
    fn all_related_holons(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<RelationshipMap, HolonError> {
        ReadableHolonImpl::all_related_holons_impl(self, context)
    }

    #[inline]
    fn into_model(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonNodeModel, HolonError> {
        ReadableHolonImpl::into_model_impl(self, context)
    }

    #[inline]
    fn is_accessible(
        &self,
        context: &dyn HolonsContextBehavior,
        access: AccessType,
    ) -> Result<(), HolonError> {
        ReadableHolonImpl::is_accessible_impl(self, context, access)
    }

    /// Retrieves the value of the specified property, if present.
    ///
    /// # Ergonomics
    /// This method accepts any type that implements [`ToPropertyName`]. That means you can call it
    /// with:
    ///
    /// - a `&str`
    /// - a `String`
    /// - a `MapString`
    /// - a `&MapString`
    /// - a `PropertyName`
    /// - a `CorePropertyTypeName` -- any variant from the CorePropertyTypeName enum
    /// - a `&CorePropertyTypeName` -- any variant from the CorePropertyTypeName enum
    /// - or any other type that implements [`ToPropertyName`]
    ///
    ///
    /// Returns `Ok(Some(value))` if the property is defined, `Ok(None)` if it is absent,
    /// or an error if the context cannot resolve the property.
    #[inline]
    fn property_value<T: ToPropertyName>(
        &self,
        context: &dyn HolonsContextBehavior,
        name: T,
    ) -> Result<Option<PropertyValue>, HolonError> {
        let prop = name.to_property_name();
        ReadableHolonImpl::property_value_impl(self, context, &prop)
    }

    /// Retrieves the collection of holons related to this holon via the specified relationship.
    ///
    /// Resolves the relationship using the provided `context` and returns an
    /// [`Rc<HolonCollection>`] of related holons. If no related holons exist for the
    /// given relationship, the collection is empty (never `None`).
    ///
    /// # Ergonomics
    /// This façade method accepts **any type implementing [`ToRelationshipName`]**,
    /// allowing you to pass a variety of argument types:
    ///
    /// - `&str` or `String` (e.g., `"friends"`)
    /// - [`RelationshipName`] or `&RelationshipName`
    /// - [`MapString`] or `&MapString`
    /// - [`CoreRelationshipTypeName`] or `&CoreRelationshipTypeName`
    ///
    /// All inputs are normalized to **SCREAMING_SNAKE_CASE** internally, so
    /// `"friends"`, `"Friends"`, and `"FRIENDS"` are treated equivalently.
    ///
    /// # Examples
    ///
    /// # Returns
    /// - `Ok(Rc<HolonCollection>)`: a collection of related holons (possibly empty).
    /// - `Err(HolonError)`: if resolution fails (e.g., invalid relationship, context errors).
    ///
    /// # Guarantees
    /// - Never returns `None`; an empty `HolonCollection` indicates no related holons.
    ///
    /// # See also
    /// - [`ToRelationshipName`] for supported input conversions.
    #[inline]
    fn related_holons<T: ToRelationshipName>(
        &self,
        context: &dyn HolonsContextBehavior,
        name: T,
    ) -> Result<Rc<HolonCollection>, HolonError> {
        let rel = name.to_relationship_name();
        ReadableHolonImpl::related_holons_impl(self, context, &rel)
    }
}

/// Blanket impl: anything that implements [`ReadableHolonImpl`]
/// automatically implements [`ReadableHolon`].
///
/// This avoids duplicate impls: implement the lower-level trait once, and
/// use the higher-level `ReadableHolon` at call sites (default methods provide the logic).
impl<T: ReadableHolonImpl + ?Sized> ReadableHolon for T {}
