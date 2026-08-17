//
use crate::{
    persistence_layer::{get_holon_node_by_path, GetPathInput},
    try_from_record,
};
use core_types::HolonError;
use hdi::prelude::Path;
use holons_core::core_shared_objects::SavedHolon;
use holons_guest_integrity::type_conversions::*;
use holons_integrity::LinkTypes;
//Stateless HDI service to bridge Holon and HolonNode
//Holochain API logic and calls should all done from the HolonNode module (separation of concerns)
//Holon should be mostly self-referential methods and data
//
// ///  ------ COMMANDS ------

// /// Marks the holon_node identified by the specified LocalId as deleted in the persistent store.
// pub fn delete_holon_internal(id: LocalId) -> Result<ActionHash, HolonError> {
//     let record = get(try_action_hash_from_local_id(&id)?, GetOptions::default())
//         .map_err(|e| holon_error_from_wasm_error(e))?
//         .ok_or_else(|| HolonError::HolonNotFound(format!("at id: {:?}", id.0)))?;
//     let _holon = try_from_record(record)?;
//     // holon.is_deletable()?;
//     delete_holon_node(try_action_hash_from_local_id(&id)?)
//         .map_err(|e| holon_error_from_wasm_error(e))
// }
// ///  ------ QUERIES ------
//
// //TODO move this associated (non-self /instance) function to the Holon_service
// pub fn get_all_holons() -> Result<Vec<Holon>, HolonError> {
//     let records = get_all_holon_nodes(());
//     match records {
//         Ok(records) => {
//             let mut holons = Vec::<Holon>::new();
//             for holon_node_record in records.clone() {
//                 let holon = try_from_record(holon_node_record.clone())?;
//                 holons.push(holon);
//             }
//             Ok(holons)
//         }
//         Err(error) => Err(HolonError::WasmError(error.to_string())),
//     }
// }
pub fn get_holon_by_path(
    path_name: String,
    linktype: LinkTypes,
) -> Result<Option<SavedHolon>, HolonError> {
    let path = Path::from(path_name);
    let link_type = linktype;
    let input = GetPathInput { path: path.clone(), link_type };
    let result = get_holon_node_by_path(input).map_err(|e| holon_error_from_wasm_error(e));
    match result {
        Ok(result) => {
            if let Some(record) = result {
                return Ok(Some(try_from_record(record)?));
            }
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

// /// gets a specific HolonNode from the local persistent store based on the original ActionHash, it then
// /// "inflates" the HolonNode into a Holon and returns it
// pub fn fetch_holon(holon_id: &HolonId) -> Result<Holon, HolonError> {
//     let local_id = match holon_id {
//         HolonId::Local(local_id) => local_id,
//         HolonId::External(_) => {
//             // Return InvalidParameter error for ExternalId
//             return Err(HolonError::InvalidParameter(
//                 "Expected LocalId, found ExternalId.".to_string(),
//             ));
//         }
//     };
//
//     let holon_node_record = get_original_holon_node(local_id.0.clone())?; // Retrieve the holon node
//     if let Some(node) = holon_node_record {
//         let holon = Holon::try_from_node(node)?;
//         Ok(holon)
//     } else {
//         // No holon_node fetched for the specified holon_id
//         Err(HolonError::HolonNotFound(local_id.0.to_string()))
//     }
// }
