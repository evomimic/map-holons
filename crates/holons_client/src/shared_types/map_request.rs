use base_types::MapString;
use core_types::{ContentSet, HolonId, PropertyMap, RelationshipName};
use holons_core::{HolonReference, StagedReference, TransientHolonBehavior, core_shared_objects::{Holon, TransientHolonManager, TransientManagerAccess}, dances::{DanceType, RequestBody}, query_layer::QueryExpression, reference_layer::TransientReference};
use serde::{Deserialize, Serialize};

use crate::shared_types::holon_space::HolonSpace;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum MapRequestBody {
    None,
    Holon(Holon),
    TargetHolons(RelationshipName, Vec<HolonReference>),
    TransientReference(TransientReference),
    HolonId(HolonId),
    ParameterValues(PropertyMap),
    StagedRef(StagedReference),
    QueryExpression(QueryExpression),
    LoadHolons(ContentSet),
}

impl MapRequestBody {
    pub fn new() -> Self {
        Self::None // Assuming 'None' is the default variant
    }

    pub fn new_holon(holon: Holon) -> Self {
        Self::Holon(holon)
    }

    pub fn new_parameter_values(parameters: PropertyMap) -> Self {
        Self::ParameterValues(parameters)
    }

    pub fn new_target_holons(
        relationship_name: RelationshipName,
        holons_to_add: Vec<HolonReference>,
    ) -> Self {
        Self::TargetHolons(relationship_name, holons_to_add)
    }

    pub fn new_staged_reference(staged_reference: StagedReference) -> Self {
        Self::StagedRef(staged_reference)
    }

    pub fn new_query_expression(query_expression: QueryExpression) -> Self {
        Self::QueryExpression(query_expression)
    }
    pub fn new_load_holons(content_set: ContentSet) -> Self {
        Self::LoadHolons(content_set)
    }
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MapRequest {
    pub name: String, // unique key within the (single) dispatch table
    pub req_type: DanceType,
    pub body: MapRequestBody,
    pub space: HolonSpace,

}
 impl MapRequest {
    pub fn new_for_reference(reference:TransientReference) -> Self {
        let name = "get_all_holons".to_string();
        let req_type = DanceType::Standalone;
        let body = MapRequestBody::TransientReference(reference);
        let space = HolonSpace::default();
        Self {
            name,
            req_type,
            body,
            space,
        }
    }
    pub fn test_for_stage_new_holon() -> Self {
        let name = "stage_new_holon".to_string();
        let req_type = DanceType::Standalone;
        let manager = TransientHolonManager::new_empty();
        let transient_ref = manager.create_empty(MapString("my_key".to_string())).unwrap();
        let locked_holon = manager.get_holon_by_id(&transient_ref.get_temporary_id()).unwrap();
        let actualholon = locked_holon.clone().read().unwrap().clone();
        let body = MapRequestBody::new_holon(actualholon);
         //holon.with_property_value(property_name, value)?;
        let space = HolonSpace::default();
        Self {
            name,
            req_type,
            body,
            space,
        }
    }
}

/*        let holon = TransientHolon::with_fields(
            MapInteger(1),
            HolonState::Mutable,
            ValidationState::ValidationRequired,
            // None,
            property_map,
            TransientRelationshipMap::new_empty(),
            None,
        ); */