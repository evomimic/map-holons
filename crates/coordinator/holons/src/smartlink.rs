use hdk::prelude::*;
use std::{collections::BTreeMap, str};
//use hdi::prelude::*;
use holons_integrity::*;
use shared_types_holon::{BaseValue, HolonId, MapString, PropertyMap, PropertyName, PropertyValue};

use crate::{holon_error::HolonError, relationship::RelationshipName};

const fn smartlink_tag_header_length() -> usize {
    // leaving this nomenclature for now
    HEADER_BYTES.len()
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SmartLink {
    pub from_address: HolonId,
    pub to_address: HolonId,
    pub relationship_name: RelationshipName, // temporarily using RelationshipName as descriptor
    pub smart_property_values: Option<PropertyMap>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LinkTagObject {
    pub relationship_name: String,
    // pub proxy_id: ActionHash,
    pub smart_property_values: Option<PropertyMap>,
}

// #[hdk_extern]
// pub fn add_smartlink(input: SmartLink) -> ExternResult<()> {
//     let link_tag = create_link_tag(input.relationship_name, input.smart_property_values);
//     debug!("added SmartLink with link_tag: {:?}", link_tag.clone());
//     create_link(
//         input.from_address.0.clone(),
//         input.to_address.0.clone(),
//         LinkTypes::SmartLink,
//         link_tag,
//     )?;
//     Ok(())
// }
// #[hdk_extern]
// pub fn get_smartlinks_for_holon_node(holon_node_hash: ActionHash) -> ExternResult<Vec<Record>> {
//     let links = get_links(holon_node_hash, LinkTypes::SmartLink, None)?;
//     let get_input: Vec<GetInput> = links
//         .into_iter()
//         .map(|link| GetInput::new(link.target.try_into().unwrap(), GetOptions::default()))
//         .collect();
//     let records: Vec<Record> = HDK
//         .with(|hdk| hdk.borrow().get(get_input))?
//         .into_iter()
//         .filter_map(|r| r)
//         .collect();
//     Ok(records)
// }
// #[derive(Serialize, Deserialize, Debug)]
// pub struct RemoveSmartLinkInput {
//     pub base_holon_node_hash: ActionHash,
//     pub target_holon_node_hash: ActionHash,
// }
// #[hdk_extern]
// pub fn remove_smartlink(input: RemoveSmartLinkInput) -> ExternResult<()> {
//     let links = get_links(
//         input.base_holon_node_hash.clone(),
//         LinkTypes::SmartLink,
//         None,
//     )?;
//     for link in links {
//         if link
//             .target
//             .into_action_hash()
//             .unwrap()
//             .eq(&input.target_holon_node_hash)
//         {
//             delete_link(link.create_link_hash)?;
//         }
//     }
//     Ok(())
// }

pub fn get_smartlink_from_link(
    source_holon_id: ActionHash,
    link: Link,
) -> Result<SmartLink, HolonError> {
    let target = link
        .target
        .into_action_hash()
        .ok_or(wasm_error!(WasmErrorInner::Guest(String::from(
            "No action hash associated with link"
        ))))?;
    let link_tag_bytes = link.tag.clone().into_inner();
    let link_tag = String::from_utf8(link_tag_bytes).map_err(|_e| {
        HolonError::Utf8Conversion(
            "Link tag bytes".to_string(),
            "String (relationship name)".to_string(),
        )
    })?;
    debug!("got: {:?}\n link_tag from smartlink ", link_tag.clone());

    let link_tag_obj = decode_link_tag(link_tag);

    let smartlink = SmartLink {
        from_address: HolonId(source_holon_id.clone()),
        to_address: HolonId(target),
        relationship_name: RelationshipName(MapString(link_tag_obj.relationship_name)),
        smart_property_values: link_tag_obj.smart_property_values,
    };

    Ok(smartlink)
}

pub fn decode_link_tag(link_tag: String) -> LinkTagObject {
    let mut chunks: Vec<&str> = link_tag.split(UNICODE_NUL_STR).collect();
    let relationship_name = chunks[0][smartlink_tag_header_length()..].to_string(); // drop leading header bytes
    debug!(
        "got {:?}\n relationship_name from link_tag",
        relationship_name
    );

    // TODO: this logic will change once we insert steps for reference_type and proxy_id
    // noting that the order of chunks by nul byte will be different

    let mut prop_map: BTreeMap<PropertyName, PropertyValue> = BTreeMap::new();

    for chunk in &mut chunks[1..] {
        let props: Vec<&str> = chunk.split(|c| c == 'Ⓝ' || c == 'Ⓥ').collect();

        // for now, always assuming value type is MapString
        prop_map.insert(
            PropertyName(MapString(props[1].to_string())),
            BaseValue::StringValue(MapString(props[3].to_string())),
        );
    }
    let smart_property_values: Option<PropertyMap> = if prop_map.is_empty() {
        None
    } else {
        Some(prop_map)
    };
    debug!(
        "got smart_property_values from link_tag: {:#?}",
        smart_property_values
    );

    LinkTagObject {
        relationship_name,
        smart_property_values,
    }
}

pub fn save_smartlink(input: SmartLink) -> Result<(), HolonError> {
    // TODO: convert access_path to string

    // TODO: convert proxy_id to string

    // TODO: populate from property_map Null-separated property values (serialized into a String) for each of the properties listed in the access path

    let link_tag = create_link_tag(input.relationship_name.clone(), input.smart_property_values);

    create_link(
        input.from_address.clone().0,
        input.to_address.clone().0,
        LinkTypes::SmartLink,
        link_tag,
    )?;
    Ok(())
}

pub fn create_link_tag(
    relationship_name: RelationshipName,
    property_values: Option<PropertyMap>,
) -> LinkTag {
    let name = relationship_name.0 .0;
    let mut bytes: Vec<u8> = vec![];

    bytes.extend_from_slice(&HEADER_BYTES);

    bytes.extend_from_slice(name.as_bytes());

    bytes.extend_from_slice(&PROLOG_SEPERATOR);
    bytes.extend_from_slice(&UNICODE_NUL_STR.as_bytes());

    // TODO: determine reference type
    // bytes.extend_from_slice(reference_type.as_bytes());
    // bytes.extend_from_slice(UNICODE_NUL_STR.as_bytes());
    // TODO: proxy_id
    // bytes.extend_from_slice(proxy_id_string.as_bytes());
    // bytes.extend_from_slice(UNICODE_NUL_STR.as_bytes());

    if let Some(property_map) = &property_values {
        for (prop, val) in property_map {
            bytes.extend_from_slice(&PROP_NAME_SEPERATOR);
            bytes.extend_from_slice(prop.0 .0.as_bytes());
            bytes.extend_from_slice(&UNICODE_NUL_STR.as_bytes());
            bytes.extend_from_slice(&PROP_VAL_SEPERATOR);
            bytes.extend_from_slice(&val.into_bytes().0);
            bytes.extend_from_slice(&UNICODE_NUL_STR.as_bytes());
        }
    }
    debug!("created link_tag: {:?}", str::from_utf8(&bytes));
    LinkTag(bytes)
}
