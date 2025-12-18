use base_types::MapString;
use holons_core::{
    core_shared_objects::{TransientHolonManager, TransientManagerAccess},
    dances::{DanceType, RequestBody},
    TransientHolonBehavior,
};
use serde::{Deserialize, Serialize};

use crate::shared_types::holon_space::HolonSpace;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MapRequest {
    pub name: String, // unique key within the (single) dispatch table
    pub req_type: DanceType,
    pub body: RequestBody,
    pub space: HolonSpace,
}
impl MapRequest {
    pub fn test_for_stage_new_holon() -> Self {
        let name = "stage_new_holon".to_string();
        let req_type = DanceType::Standalone;
        let manager = TransientHolonManager::new_empty();
        let transient_ref = manager.create_empty(MapString("my_key".to_string())).unwrap();
        let locked_holon = manager.get_holon_by_id(&transient_ref.get_temporary_id()).unwrap();
        let actualholon = locked_holon.clone().read().unwrap().clone();
        let body = RequestBody::new_holon(actualholon);
        //holon.with_property_value(property_name, value)?;
        let space = HolonSpace::default();
        Self { name, req_type, body, space }
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
