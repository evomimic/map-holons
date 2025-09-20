use crate::reference_layer::writable_impl::WritableHolonImpl;
use crate::reference_layer::{HolonReference, HolonsContextBehavior};
use base_types::BaseValue;
use core_types::HolonError;
use type_names::{relationship_names::ToRelationshipName, ToPropertyName};

/// Public façade for write operations (ergonomic + complete).
///
/// Accepts any types implementing [`ToRelationshipName`] or [`ToPropertyName`].
/// Inputs are normalized (e.g., relationship names to SCREAMING_SNAKE_CASE)
/// and forwarded to the canonical `*_impl` methods.
///
/// This is the trait you should import and use in call sites.
/// Implementors only need to implement [`WritableHolonImpl`].
pub trait WritableHolon: WritableHolonImpl {
    /// Adds one or more related holons under the given relationship.
    ///
    /// # Ergonomics
    /// Accepts any type implementing [`ToRelationshipName`], so you can pass:
    /// - `&str` / `String` (e.g. `"friends"`)
    /// - [`RelationshipName`] or `&RelationshipName`
    /// - [`MapString`] or `&MapString` (normalized to SCREAMING_SNAKE_CASE)
    /// - [`CoreRelationshipTypeName`] or `&CoreRelationshipTypeName`
    ///
    /// # Examples
    /// ```ignore
    /// holon.add_related_holons(context, "friends", vec![other])?;
    /// holon.add_related_holons(context, CoreRelationshipTypeName::Friends, vec![other])?;
    /// ```
    #[inline]
    fn add_related_holons<T: ToRelationshipName>(
        &self,
        context: &dyn HolonsContextBehavior,
        name: T,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        WritableHolonImpl::add_related_holons_impl(
            self,
            context,
            name.to_relationship_name(),
            holons,
        )
    }

    /// Removes one or more related holons under the given relationship.
    ///
    /// # Ergonomics
    /// Accepts any type implementing [`ToRelationshipName`] (same as
    /// [`add_related_holons`]).
    ///
    /// # Examples
    /// ```ignore
    /// holon.remove_related_holons(context, "friends", vec![other])?;
    /// ```
    #[inline]
    fn remove_related_holons<T: ToRelationshipName>(
        &self,
        context: &dyn HolonsContextBehavior,
        name: T,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError> {
        WritableHolonImpl::remove_related_holons_impl(
            self,
            context,
            name.to_relationship_name(),
            holons,
        )
    }

    /// Sets or updates a property value for this holon.
    ///
    /// # Ergonomics
    /// Accepts any type implementing [`ToPropertyName`], so you can pass:
    /// - `&str` / `String` (e.g. `"title"`)
    /// - [`PropertyName`] or `&PropertyName`
    /// - Other types that implement `ToPropertyName`
    ///
    /// # Examples
    /// ```ignore
    /// holon.with_property_value(context, "title", BaseValue::from("Hello"))?;
    /// ```
    #[inline]
    fn with_property_value<T: ToPropertyName>(
        &self,
        context: &dyn HolonsContextBehavior,
        name: T,
        value: BaseValue,
    ) -> Result<(), HolonError> {
        WritableHolonImpl::with_property_value_impl(self, context, name.to_property_name(), value)
    }

    /// Removes a property value from this holon.
    ///
    /// # Ergonomics
    /// Accepts any type implementing [`ToPropertyName`].
    ///
    /// # Examples
    /// ```ignore
    /// holon.remove_property_value(context, "title")?;
    /// ```
    #[inline]
    fn remove_property_value<T: ToPropertyName>(
        &self,
        context: &dyn HolonsContextBehavior,
        name: T,
    ) -> Result<(), HolonError> {
        WritableHolonImpl::remove_property_value_impl(self, context, name.to_property_name())
    }

    /// Attaches a descriptor holon to this holon.
    ///
    /// This is a plain forwarder; no ergonomic conversion is applied.
    #[inline]
    fn with_descriptor(
        &self,
        context: &dyn HolonsContextBehavior,
        descriptor: HolonReference,
    ) -> Result<(), HolonError> {
        WritableHolonImpl::with_descriptor_impl(self, context, descriptor)
    }

    /// Attaches a predecessor holon to this holon.
    ///
    /// This is a plain forwarder; no ergonomic conversion is applied.
    #[inline]
    fn with_predecessor(
        &self,
        context: &dyn HolonsContextBehavior,
        predecessor: Option<HolonReference>,
    ) -> Result<(), HolonError> {
        WritableHolonImpl::with_predecessor_impl(self, context, predecessor)
    }
}
impl<T: WritableHolonImpl + ?Sized> WritableHolon for T {}
