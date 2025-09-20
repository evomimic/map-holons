use crate::{HolonReference, HolonsContextBehavior};
use base_types::BaseValue;
use core_types::HolonError;
use integrity_core_types::{PropertyName, RelationshipName};

/// Internal implementation surface for write operations (canonical, non-ergonomic).
#[doc(hidden)] // or `pub(crate)` if possible
pub trait WritableHolonImpl {
    fn add_related_holons_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError>;

    fn remove_related_holons_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError>;

    fn with_property_value_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        property: PropertyName,
        value: BaseValue,
    ) -> Result<(), HolonError>;

    fn remove_property_value_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        property: PropertyName,
    ) -> Result<(), HolonError>;

    fn with_descriptor_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        descriptor: HolonReference,
    ) -> Result<(), HolonError>;

    fn with_predecessor_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        predecessor: Option<HolonReference>,
    ) -> Result<(), HolonError>;
}
