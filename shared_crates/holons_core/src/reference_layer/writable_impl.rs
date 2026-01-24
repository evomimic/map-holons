use crate::HolonReference;
use base_types::BaseValue;
use core_types::{HolonError, PropertyName, RelationshipName};

/// Internal implementation surface for write operations (canonical, non-ergonomic).
#[doc(hidden)] // or `pub(crate)` if possible
pub trait WritableHolonImpl {
    fn add_related_holons_impl(
        &mut self,
        relationship: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError>;

    fn remove_related_holons_impl(
        &mut self,
        relationship: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<&mut Self, HolonError>;

    fn with_property_value_impl(
        &mut self,
        property: PropertyName,
        value: BaseValue,
    ) -> Result<&mut Self, HolonError>;

    fn remove_property_value_impl(
        &mut self,
        property: PropertyName,
    ) -> Result<&mut Self, HolonError>;

    fn with_descriptor_impl(
        &mut self,
        descriptor: HolonReference,
    ) -> Result<(), HolonError>;

    fn with_predecessor_impl(
        &mut self,
        predecessor: Option<HolonReference>,
    ) -> Result<(), HolonError>;
}
