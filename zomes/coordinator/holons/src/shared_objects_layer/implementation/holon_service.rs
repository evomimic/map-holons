use crate::persistence_layer::{
    create_path_to_holon_node, get_holon_node_by_path, get_original_holon_node, CreatePathInput,
    GetPathInput,
};
use crate::shared_objects_layer::{Holon, HolonError};
use hdi::prelude::{ActionHash, Path};
use holons_integrity::LinkTypes;
use shared_types_holon::LocalId;
//Stateless HDI service to bridge Holon and HolonNode
//Holochain API logic and calls should all done from the HolonNode module (separation of concerns)
//Holon should be mostly self-referential methods and data

///  ------ COMMANDS ------

pub fn create_local_path(
    target_holon_hash: LocalId,
    path_name: String,
    linktype: LinkTypes,
) -> Result<ActionHash, HolonError> {
    let path = Path::from(path_name);
    let link_type = linktype; //LinkTypes::LocalHolonSpace;
    let input = CreatePathInput { path, link_type, target_holon_node_hash: target_holon_hash.0 };
    create_path_to_holon_node(input).map_err(|e| HolonError::from(e))
}

///  ------ QUERIES ------

pub fn get_holon_by_path(
    path_name: String,
    linktype: LinkTypes,
) -> Result<Option<Holon>, HolonError> {
    let path = Path::from(path_name);
    let link_type = linktype;
    let input = GetPathInput { path: path.clone(), link_type };
    let result = get_holon_node_by_path(input).map_err(|e| HolonError::from(e));
    return match result {
        Ok(result) => {
            if let Some(record) = result {
                return Ok(Some(Holon::try_from_node(record)?));
            }
            Ok(None)
        }
        Err(error) => Err(error),
    };
}

/// gets a specific HolonNode from the local persistent store based on the original ActionHash, it then
/// "inflates" the HolonNode into a Holon and returns it
pub fn get_holon_by_local_id(local_id: &LocalId) -> Result<Holon, HolonError> {
    let holon_node_record = get_original_holon_node(local_id.0.clone())?; //, GetOptions::default())?;
    if let Some(node) = holon_node_record {
        let holon = Holon::try_from_node(node)?;
        return Ok(holon);
    } else {
        // no holon_node fetched for specified holon_id
        Err(HolonError::HolonNotFound(local_id.0.to_string()))
    }
}
