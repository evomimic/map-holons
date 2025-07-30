
use crate::core_shared_objects::Holon;
use crate::reference_layer::{HolonReference, HolonsContextBehavior};
use base_types::BaseValue;
use core_types::HolonError;
use integrity_core_types::{PropertyName, RelationshipName};

pub trait WriteableHolon {

    fn add_related_holons(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError>;

    fn remove_related_holons(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: &RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError>;

    #[deprecated]
    fn stage_new_from_clone_deprecated(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Holon, HolonError>;

    fn with_descriptor(
        &self,
        context: &dyn HolonsContextBehavior,
        descriptor_reference: HolonReference,
    ) -> Result<&Self, HolonError>;

    fn with_predecessor(
        &self,
        context: &dyn HolonsContextBehavior,
        predecessor_reference_option: Option<HolonReference>,
    ) -> Result<(), HolonError>;

    fn with_property_value(
        &self,
        context: &dyn HolonsContextBehavior,
        property: PropertyName,
        value: Option<BaseValue>,
    ) -> Result<&Self, HolonError>;
}
