use hdi::prelude::{ActionHash, Path};
use holons_integrity::LinkTypes;
use shared_types_holon::{ExternalId, HolonId, LocalId};

use crate::holon::Holon;
use crate::holon_error::HolonError;
use crate::holon_node::{
    create_path_to_holon_node, get_holon_node_by_path, get_original_holon_node, CreatePathInput,
    GetPathInput,
};

//Stateless HDI service to bridge Holon and HolonNode
//Holochain API logic and calls should all done from the HolonNode module (separation of concerns)
//Holon should be mostly self referential methods and data

///  ------ COMMANDS ------

pub fn create_local_path(
    target_holon_hash: LocalId,
    path_name: String,
    linktype: LinkTypes,
) -> Result<ActionHash, HolonError> {
    let path = Path::from(path_name);
    let link_type = linktype; //LinkTypes::LocalHolonSpace;
    let input = CreatePathInput {
        path: path,
        link_type: link_type,
        target_holon_node_hash: target_holon_hash.0,
    };
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
    match result {
        Ok(result) => {
            if let Some(record) = result {
                return Ok(Some(Holon::try_from_node(record)?));
            }
            return Ok(None);
        }
        Err(error) => return Err(error),
    }
}

pub fn get_holon_by_id(holon_id: &HolonId) -> Result<Holon, HolonError> {
    if holon_id.is_local() {
        let local_id = holon_id.local_id();
        get_holon_by_local_id(local_id)
    } else {
        let external_id = match holon_id.external_id() {
            Some(id) => id,
            None => {
                return Err(HolonError::InvalidHolonReference(
                    "HolonId is not External".to_string(),
                ))
            }
        };
        get_holon_by_external_id(external_id)
    }
}

pub fn get_holon_by_external_id(external_id: &ExternalId) -> Result<Holon, HolonError> {
    return Err(HolonError::NotImplemented(format!(
        "external_id: {} feature not built",
        external_id.space_id.0.to_string()
    )));
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
