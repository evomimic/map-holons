use crate::core_shared_objects::holon::state::AccessType;
use crate::core_shared_objects::holon::EssentialHolonContent;
use crate::reference_layer::TransientReference;
use crate::{HolonCollection, HolonReference, HolonsContextBehavior, RelationshipMap};
use base_types::MapString;
use core_types::{
    HolonError, HolonId, HolonNodeModel, PropertyName, PropertyValue, RelationshipName,
};
use std::rc::Rc;

pub trait ReadableHolonImpl {
    /// Generic clone for all Holon variants. Resulting clone is always a TransientReference, regardless of source phase.
    fn clone_holon_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<TransientReference, HolonError>;

    fn all_related_holons_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<RelationshipMap, HolonError>;

    fn holon_id_impl(&self, context: &dyn HolonsContextBehavior) -> Result<HolonId, HolonError>;

    fn predecessor_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<HolonReference>, HolonError>;

    fn property_value_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError>;

    fn key_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<Option<MapString>, HolonError>;

    fn related_holons_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        relationship_name: &RelationshipName,
    ) -> Result<Rc<HolonCollection>, HolonError>;

    fn versioned_key_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<MapString, HolonError>;

    fn essential_content_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<EssentialHolonContent, HolonError>;

    fn summarize_impl(&self, context: &dyn HolonsContextBehavior) -> Result<String, HolonError>;

    fn into_model_impl(
        &self,
        context: &dyn HolonsContextBehavior,
    ) -> Result<HolonNodeModel, HolonError>;

    fn is_accessible_impl(
        &self,
        context: &dyn HolonsContextBehavior,
        access_type: AccessType,
    ) -> Result<(), HolonError>;
}
