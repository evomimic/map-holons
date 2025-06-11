use crate::reference_layer::{HolonReference, HolonsContextBehavior, StagedReference};

use crate::core_shared_objects::{Holon, HolonError, RelationshipName};

use base_types::BaseValue;
use integrity_core_types::PropertyName;

pub trait WriteableHolon {
    fn abandon_staged_changes(
        &mut self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<(), HolonError>;

    fn add_related_holons(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError>;

    fn clone_reference(&self) -> StagedReference;

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
