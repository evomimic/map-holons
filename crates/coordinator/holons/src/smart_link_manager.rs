use crate::holon_error::HolonError;
use crate::relationship::RelationshipName;
use hdk::prelude::*;
use holons_integrity::LinkTypes;
use shared_types_holon::HolonId;

use holons_integrity::smartlink::{HEADER_BYTES, PROLOG_SEPERATOR, UNICODE_NUL_STR};
// use holons_integrity::smart_link::{UNICODE_NUL_STR, HEADER_BYTES, PROLOG_SEPERATOR, LOCAL_REFERENCE_TYPE, EXTERNAL_REFERENCE_TYPE};

#[derive(Serialize, Deserialize, Debug)]
pub struct SmartLinkInput {
    pub from_address: HolonId,
    pub to_address: HolonId,
    // temporarily using RelationshipName as descriptor
    pub relationship_name: RelationshipName,
    // temporarily set as options - defaulting to None for now
    // pub access_path: Option<AccessPath>,
    // pub proxy_id: Option<OutboundProxyId>,
    // pub property_map: Option<PropertyMap>,
}

pub fn create_smart_link(input: SmartLinkInput) -> Result<(), HolonError> {
    // TODO: convert access_path to string

    // TODO: convert proxy_id to string

    // TODO: populate from property_map Null-separated property values (serialized into a String) for each of the properties listed in the access path

    let link_tag = create_link_tag(input.relationship_name.0 .0.clone());

    create_link(
        input.from_address.clone().0,
        input.to_address.clone().0,
        LinkTypes::SmartLink,
        link_tag,
    )?;
    Ok(())
}
// fn create_link_tag(relationship_descriptor: String, access_path_string: String, proxy_id_string: String, property_values: String) -> LinkTag {
fn create_link_tag(relationship_descriptor: String) -> LinkTag {
    let mut bytes: Vec<u8> = vec![];

    bytes.extend_from_slice(&HEADER_BYTES);

    bytes.extend_from_slice(relationship_descriptor.as_bytes());
    bytes.extend_from_slice(UNICODE_NUL_STR.as_bytes());
    // bytes.extend_from_slice(access_path_string.as_bytes());
    // bytes.extend_from_slice(UNICODE_NUL_STR.as_bytes());
    bytes.extend_from_slice(&PROLOG_SEPERATOR);

    // TODO: determine reference type
    // bytes.extend_from_slice(reference_type.as_bytes());

    // bytes.extend_from_slice(proxy_id_string.as_bytes());

    // bytes.extend_from_slice(property_values.as_bytes());

    LinkTag(bytes)
}
