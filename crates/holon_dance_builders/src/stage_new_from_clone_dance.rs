use std::collections::BTreeMap;

use holons_core::dances::{DanceRequest, DanceType, RequestBody};
use holons_core::{core_shared_objects::HolonError, HolonReference};
use shared_types_holon::{BaseValue, MapString, PropertyName};

///
/// Builds a dance request for staging a new cloned Holon
pub fn build_stage_new_from_clone_dance_request(
    original_holon: HolonReference,
    new_key: MapString,
) -> Result<DanceRequest, HolonError> {
    let mut property_map = BTreeMap::new();
    property_map
        .insert(PropertyName(MapString("key".to_string())), Some(BaseValue::StringValue(new_key)));
    Ok(DanceRequest::new(
        MapString("stage_new_from_clone".to_string()),
        DanceType::CloneMethod(original_holon),
        RequestBody::ParameterValues(property_map),
        None,
    ))
}
