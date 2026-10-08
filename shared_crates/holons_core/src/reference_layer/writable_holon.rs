use crate::reference_layer::writable_impl::WritableHolonImpl;
use crate::reference_layer::HolonReference;
use crate::reference_layer::ReadableHolon;
use base_types::ToBaseValue;
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
    /// Attempts to populate defaults on a best-effort, idempotent basis.
    ///
    /// Preserves every existing property value, including earlier defaults, and
    /// fills only absent required properties with an available effective default.
    /// Optional properties and required properties without defaults stay absent.
    /// Repeated attempts over unchanged inputs perform no further property writes.
    ///
    /// A missing subject descriptor (`HolonError::MissingDescribedBy`) makes this
    /// attempt a no-op. The same error while assessing a property skips that
    /// property and continues with independent properties. Skipped properties are
    /// not recorded; Commit validation assesses any resulting omission.
    /// Every other descriptor-read or property-write error propagates immediately;
    /// partial writes are not rolled back. Success does not establish completeness
    /// or validity, and this operation does not change validation state.
    ///
    /// A later explicit call can reconsider omissions, including refilling a
    /// removed property. Already populated values remain explicit even if the
    /// descriptor changes. Commit, reads, and restoration never invoke this
    /// operation; Commit validates the actual explicit state without supplying
    /// defaults.
    fn populate_defaults(&mut self) -> Result<(), HolonError>
    where
        Self: ReadableHolon,
    {
        let properties =
            match self.holon_descriptor().and_then(|descriptor| descriptor.instance_properties()) {
                Ok(properties) => properties,
                Err(HolonError::MissingDescribedBy { .. }) => {
                    return Ok(());
                }
                Err(error) => return Err(error),
            };
        for property in properties {
            match property.populate_default_if_required_and_absent(self) {
                Ok(()) | Err(HolonError::MissingDescribedBy { .. }) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    /// Adds one or more related holons under the given relationship.
    ///
    /// # Descriptor attachment
    ///
    /// Authoring `DescribedBy` directly here attaches a descriptor without itself
    /// attempting default population. This supports internal assembly, such as
    /// loader relationship assembly, and tests that need attachment without
    /// defaults. Normal creation seeking descriptor-assisted initialization should
    /// use [`Self::with_descriptor`]. Later staging (`stage_new_holon`), cloning,
    /// an explicit [`Self::populate_defaults`] call, or the loader's final pass may
    /// still populate defaults on this holon.
    ///
    /// # Ergonomics
    ///
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

    /// Replaces any existing descriptor, then attempts [`Self::populate_defaults`].
    ///
    /// Use this path for descriptor-assisted initialization. Direct `DescribedBy`
    /// authoring through [`Self::add_related_holons`] attaches without itself
    /// attempting defaults.
    /// Changing the descriptor of an existing staged holon produces a new version.
    /// Reattaching the same descriptor preserves the edge and still attempts defaults.
    ///
    /// An attachment error is returned without attempting defaults. After a
    /// successful attachment, default population follows its best-effort error
    /// contract: an error may leave the descriptor attached and some defaults
    /// written. These changes are not rolled back.
    fn with_descriptor(&mut self, descriptor: HolonReference) -> Result<(), HolonError>
    where
        Self: ReadableHolon,
    {
        WritableHolonImpl::with_descriptor_impl(self, descriptor)?;
        // Attachment locks are released here; defaults use ordinary property writes so
        // staged mutation accounting and versioning apply unchanged.
        self.populate_defaults()
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
