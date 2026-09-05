use crate::reference_layer::writable_impl::WritableHolonImpl;
use crate::reference_layer::HolonReference;
use crate::reference_layer::ReadableHolon;
use base_types::ToBaseValue;
use core_types::HolonError;
use type_names::{relationship_names::ToRelationshipName, ToPropertyName};

/// Whether construction defaults could be fully resolved during this pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionOutcome {
    /// All applicable defaults were considered; this does not establish validity.
    Completed,
    /// A governing descriptor is not attached yet; retry after reference resolution.
    DeferredNoDescriptor,
}

/// Public façade for write operations (ergonomic + complete).
///
/// Accepts any types implementing [`ToRelationshipName`] or [`ToPropertyName`].
/// Inputs are normalized (e.g., relationship names to SCREAMING_SNAKE_CASE)
/// and forwarded to the canonical `*_impl` methods.
///
/// This is the trait you should import and use in call sites.
/// Implementors only need to implement [`WritableHolonImpl`].
pub trait WritableHolon: WritableHolonImpl {
    /// Populates applicable effective defaults without overwriting authored
    /// values.
    ///
    /// Missing descriptors, including those needed by property descriptors, defer
    /// completion while preserving progress on other properties. All other errors
    /// propagate. Retrying after reference resolution is safe and idempotent.
    fn populate_defaults(&mut self) -> Result<CompletionOutcome, HolonError>
    where
        Self: ReadableHolon,
    {
        let properties =
            match self.holon_descriptor().and_then(|descriptor| descriptor.instance_properties()) {
                Ok(properties) => properties,
                Err(HolonError::MissingDescribedBy { .. }) => {
                    return Ok(CompletionOutcome::DeferredNoDescriptor);
                }
                Err(error) => return Err(error),
            };
        let mut outcome = CompletionOutcome::Completed;
        for property in properties {
            match property.populate_default_if_required_and_absent(self) {
                Ok(()) => {}
                Err(HolonError::MissingDescribedBy { .. }) => {
                    outcome = CompletionOutcome::DeferredNoDescriptor;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(outcome)
    }

    /// Adds one or more related holons under the given relationship.
    ///
    /// # Ergonomics
    /// Accepts any type implementing [`ToRelationshipName`], so you can pass:
    /// - `&str` / `String` (e.g. `"friends"`)
    /// - [`RelationshipName`] or `&RelationshipName`
    /// - [`MapString`] or `&MapString` (normalized to SCREAMING_SNAKE_CASE)
    /// - [`CoreRelationshipTypeName`] or `&CoreRelationshipTypeName`
    #[inline]
    fn add_related_holons<T: ToRelationshipName>(
        &mut self,
        name: T,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        WritableHolonImpl::add_related_holons_impl(self, name.to_relationship_name(), holons)
    }

    /// Removes one or more related holons under the given relationship.
    ///
    /// # Ergonomics
    /// Accepts any type implementing [`ToRelationshipName`] (same as
    /// [`add_related_holons`]).
    ///
    /// # Examples
    /// ```ignore
    /// holon.remove_related_holons("friends", vec![other])?;
    /// ```
    #[inline]
    fn remove_related_holons<T: ToRelationshipName>(
        &mut self,
        name: T,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError> {
        WritableHolonImpl::remove_related_holons_impl(self, name.to_relationship_name(), holons)
    }

    /// Sets or updates a property value for this holon.
    ///
    /// # Ergonomics
    /// Accepts any type implementing [`ToPropertyName`], so you can pass:
    /// - `&str` / `String` (e.g. `"title"`)
    /// - [`PropertyName`] or `&PropertyName`
    /// - Other types that implement `ToPropertyName`
    #[inline]
    fn with_property_value<N: ToPropertyName, V: ToBaseValue>(
        &mut self,
        name: N,
        value: V,
    ) -> Result<&mut Self, HolonError> {
        WritableHolonImpl::with_property_value_impl(
            self,
            name.to_property_name(),
            value.to_base_value(),
        )
    }

    /// Removes a property value from this holon.
    ///
    /// # Ergonomics
    /// Accepts any type implementing [`ToPropertyName`].
    ///
    /// # Examples
    /// ```ignore
    /// holon.remove_property_value("title")?;
    /// ```
    #[inline]
    fn remove_property_value<T: ToPropertyName>(
        &mut self,
        name: T,
    ) -> Result<&mut Self, HolonError> {
        WritableHolonImpl::remove_property_value_impl(self, name.to_property_name())
    }

    /// Attaches a descriptor holon to this holon.
    ///
    /// This is a plain forwarder; no ergonomic conversion is applied.
    #[inline]
    fn with_descriptor(&mut self, descriptor: HolonReference) -> Result<(), HolonError> {
        WritableHolonImpl::with_descriptor_impl(self, descriptor)
    }

    /// Attaches a predecessor holon to this holon.
    ///
    /// This is a plain forwarder; no ergonomic conversion is applied.
    #[inline]
    fn with_predecessor(&mut self, predecessor: Option<HolonReference>) -> Result<(), HolonError> {
        WritableHolonImpl::with_predecessor_impl(self, predecessor)
    }
}
impl<T: WritableHolonImpl + ?Sized> WritableHolon for T {}
