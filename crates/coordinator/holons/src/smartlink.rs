use hdk::prelude::*;
//use hdi::prelude::*;
use holons_integrity::*;
use shared_types_holon::MapString;

use crate::{
    holon_error::HolonError, relationship::RelationshipName, smart_reference::SmartReference,
};

const fn smartlink_tag_header_length() -> usize {
    // leaving this nomenclature for now
    HEADER_BYTES.len()
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AddSmartLinkInput {
    pub base_holon_node_hash: ActionHash,
    pub target_holon_node_hash: ActionHash,
    pub tag: LinkTag,
}
#[hdk_extern]
pub fn add_smartlink(input: AddSmartLinkInput) -> ExternResult<()> {
    create_link(
        input.base_holon_node_hash.clone(),
        input.target_holon_node_hash.clone(),
        LinkTypes::SmartLink,
        input.tag,
    )?;
    Ok(())
}
#[hdk_extern]
pub fn get_smartlinks_for_holon_node(holon_node_hash: ActionHash) -> ExternResult<Vec<Record>> {
    let links = get_links(holon_node_hash, LinkTypes::SmartLink, None)?;
    let get_input: Vec<GetInput> = links
        .into_iter()
        .map(|link| GetInput::new(link.target.try_into().unwrap(), GetOptions::default()))
        .collect();
    let records: Vec<Record> = HDK
        .with(|hdk| hdk.borrow().get(get_input))?
        .into_iter()
        .filter_map(|r| r)
        .collect();
    Ok(records)
}
#[derive(Serialize, Deserialize, Debug)]
pub struct RemoveSmartLinkInput {
    pub base_holon_node_hash: ActionHash,
    pub target_holon_node_hash: ActionHash,
}
#[hdk_extern]
pub fn remove_smartlink(input: RemoveSmartLinkInput) -> ExternResult<()> {
    let links = get_links(
        input.base_holon_node_hash.clone(),
        LinkTypes::SmartLink,
        None,
    )?;
    for link in links {
        if link
            .target
            .into_action_hash()
            .unwrap()
            .eq(&input.target_holon_node_hash)
        {
            delete_link(link.create_link_hash)?;
        }
    }
    Ok(())
}

// TODO: expand to decode all data from smart_link
pub fn get_relationship_name_from_smartlink(link: Link) -> Result<RelationshipName, HolonError> {
    let link_tag_bytes = link.tag.clone().into_inner();
    let link_tag = String::from_utf8(link_tag_bytes).map_err(|_e| {
        HolonError::Utf8Conversion(
            "Link tag bytes".to_string(),
            "String (relationship name)".to_string(),
        )
    })?;
    debug!("got: {:?}\n link_tag from smartlink ", link_tag.clone());

    let chunks: Vec<&str> = link_tag.split(UNICODE_NUL_STR).collect();
    let name = chunks[0][smartlink_tag_header_length()..].to_string(); // drop leading header bytes
    debug!("got {:?}\n relationship_name from link_tag", name.clone());

    Ok(RelationshipName(MapString(name)))
}
