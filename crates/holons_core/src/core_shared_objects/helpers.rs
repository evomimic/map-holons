use crate::Holon;
use shared_types_holon::{MapString, PropertyMap, PropertyName};
//TODO: move static/stateless HDI/HDK functions to the Holon_service

pub fn get_key_from_property_map(map: &PropertyMap) -> Option<MapString> {
    let key_option = map.get(&PropertyName(MapString("key".to_string())));
    if let Some(key) = key_option {
        Some(MapString(key.into()))
    } else {
        None
    }
}
// Standalone function to summarize a vector of Holons
pub fn summarize_holons(holons: &Vec<Holon>) -> String {
    let summaries: Vec<String> = holons.iter().map(|holon| holon.summarize()).collect();
    format!("Holons: [{}]", summaries.join(", "))
}
