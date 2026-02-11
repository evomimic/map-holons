use crate::core_shared_objects::holon::state::AccessType;
use crate::core_shared_objects::holon::EssentialHolonContent;
use crate::reference_layer::TransientReference;
use crate::{HolonCollection, HolonReference, RelationshipMap};
use base_types::MapString;
use core_types::{
    HolonError, HolonId, HolonNodeModel, PropertyName, PropertyValue, RelationshipName,
};
use std::sync::{Arc, RwLock};

pub trait ReadableHolonImpl {
    /// Generic clone for all Holon variants. Resulting clone is always a TransientReference, regardless of source phase.
    fn clone_holon_impl(&self) -> Result<TransientReference, HolonError>;

    fn all_related_holons_impl(&self) -> Result<RelationshipMap, HolonError>;

    fn holon_id_impl(&self) -> Result<HolonId, HolonError>;

    fn predecessor_impl(&self) -> Result<Option<HolonReference>, HolonError>;

    fn property_value_impl(
        &self,
        property_name: &PropertyName,
    ) -> Result<Option<PropertyValue>, HolonError>;

    fn key_impl(&self) -> Result<Option<MapString>, HolonError>;

    fn related_holons_impl(
        &self,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError>;

    fn versioned_key_impl(&self) -> Result<MapString, HolonError>;

    fn essential_content_impl(&self) -> Result<EssentialHolonContent, HolonError>;

    fn summarize_impl(&self) -> Result<String, HolonError>;

    fn into_model_impl(&self) -> Result<HolonNodeModel, HolonError>;

    fn is_accessible_impl(&self, access_type: AccessType) -> Result<(), HolonError>;
}
