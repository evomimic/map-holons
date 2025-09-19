use crate::reference_layer::{HolonReference, HolonsContextBehavior};
use base_types::BaseValue;
use core_types::HolonError;
use integrity_core_types::{PropertyName, RelationshipName};
use type_names::{relationship_names::ToRelationshipName, ToPropertyName};

pub trait WriteableHolonReferenceLayer {
    fn add_related_holons_ref_layer(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError>;

    fn remove_property_value_ref_layer(&self, context: &dyn HolonsContextBehavior, name: PropertyName) -> Result<(), HolonError>;

    fn remove_related_holons_ref_layer(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError>;

    fn with_descriptor(
        &self,
        context: &dyn HolonsContextBehavior,
        descriptor_reference: HolonReference,
    ) -> Result<(), HolonError>;

    fn with_predecessor(
        &self,
        context: &dyn HolonsContextBehavior,
        predecessor_reference_option: Option<HolonReference>,
    ) -> Result<(), HolonError>;

    fn with_property_value_ref_layer(
        &self,
        context: &dyn HolonsContextBehavior,
        property: PropertyName,
        value: BaseValue,
    ) -> Result<(), HolonError>;
}

pub trait WriteableHolon: WriteableHolonReferenceLayer {
    fn add_related_holons<T: ToRelationshipName>(
        &self,
        context: &dyn HolonsContextBehavior,
        name: T,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError>;

    fn with_property_value<T: ToPropertyName>(
        &self,
        context: &dyn HolonsContextBehavior,
        name: T,
        value: BaseValue,
    ) -> Result<(), HolonError>;

    fn remove_property_value<T: ToPropertyName>(&self, context: &dyn HolonsContextBehavior, name: T) -> Result<(), HolonError>;

    fn remove_related_holons<T: ToRelationshipName>(
        &self,
        context: &dyn HolonsContextBehavior,
        name: T,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError>;
}
