// shared_crates/holon_dance_builders/src/new_holon_dance.rs
use base_types::{BaseValue, MapString};
use holons_core::dances::{DanceRequest, DanceType, RequestBody};
use integrity_core_types::{PropertyMap, PropertyName};

/// Build a Standalone `"new_holon"` dance request with an optional key.
///
/// - If `key` is `None`, the request uses `RequestBody::None` (creates a keyless transient holon).
/// - If `key` is `Some(MapString)`, the request uses `RequestBody::ParameterValues`
///   containing a single `"key"` property.
pub fn build_new_holon_dance_request(key: Option<MapString>) -> DanceRequest {
    match key {
        Some(k) => {
            let mut params = PropertyMap::new();
            params.insert(PropertyName(MapString("key".into())), BaseValue::StringValue(k));

            DanceRequest::new(
                MapString("new_holon".into()),
                DanceType::Standalone,
                RequestBody::ParameterValues(params),
                None,
            )
        }
        None => DanceRequest::new(
            MapString("new_holon".into()),
            DanceType::Standalone,
            RequestBody::None,
            None,
        ),
    }
}
