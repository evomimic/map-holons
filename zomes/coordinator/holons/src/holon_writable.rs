use shared_types_holon::{BaseValue, HolonId, PropertyName};

use crate::{
    context::HolonsContext, holon::Holon, holon_error::HolonError, holon_reference::HolonReference,
    relationship::RelationshipName, staged_reference::StagedReference,
};

pub trait HolonWritable {
    fn abandon_staged_changes(&mut self, context: &HolonsContext) -> Result<(), HolonError>;

    fn add_related_holons(
        &self,
        context: &HolonsContext,
        relationship_name: RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError>;

    fn commit(&self, context: &HolonsContext) -> Result<Holon, HolonError>;

    fn clone_reference(&self) -> StagedReference;

    fn get_id(&self, context: &HolonsContext) -> Result<HolonId, HolonError>;

    fn get_predecessor(
        &self,
        context: &HolonsContext,
    ) -> Result<Option<HolonReference>, HolonError>;

    fn remove_related_holons(
        &self,
        context: &HolonsContext,
        relationship_name: &RelationshipName,
        holons: Vec<HolonReference>,
    ) -> Result<(), HolonError>;

    fn stage_new_from_clone(&self, context: &HolonsContext) -> Result<Holon, HolonError>;

    fn with_descriptor(
        &self,
        context: &HolonsContext,
        descriptor_reference: HolonReference,
    ) -> Result<&Self, HolonError>;

    fn with_predecessor(
        &self,
        context: &HolonsContext,
        predecessor_reference_option: Option<HolonReference>,
    ) -> Result<(), HolonError>;

    fn with_property_value(
        &self,
        context: &HolonsContext,
        property: PropertyName,
        value: BaseValue,
    ) -> Result<&Self, HolonError>;
}
