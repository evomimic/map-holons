use hdk::prelude::*;
use holons_integrity::LinkTypes;
use holons_integrity::*;
use shared_types_holon::{
    BaseValue, HolonId, LocalId, MapString, PropertyMap, PropertyName, PropertyValue,
};
use std::{collections::BTreeMap, str};

use crate::helpers::get_key_from_property_map;
use crate::holon_reference::HolonReference;
use crate::smart_reference::SmartReference;
use crate::{holon_error::HolonError, relationship::RelationshipName};

const fn smartlink_tag_header_length() -> usize {
    // leaving this nomenclature for now
    SMARTLINK_HEADER_BYTES.len()
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SmartLink {
    pub from_address: LocalId,
    pub to_address: HolonId, // NOTE: this could be an External HolonId
    pub relationship_name: RelationshipName, // temporarily using RelationshipName as descriptor
    pub smart_property_values: Option<PropertyMap>,
}

#[derive(Default, Serialize, Deserialize, Debug)]
pub struct LinkTagObject {
    pub relationship_name: String,
    // pub proxy_id: HolonSpaceId,
    pub smart_property_values: Option<PropertyMap>,
}

impl SmartLink {
    /// The implementation of this function currently relies on a "key" property being stored
    /// in the property_map. The intended design is to derive the key from the HolonDescriptor's
    /// KEY_PROPERTIES relationship. However, the current parameters to this function are not
    /// sufficient to do this.
    /// TODO: update this function to align with described key property list design
    pub fn get_key(&self) -> Option<MapString> {
        if let Some(ref map) = self.smart_property_values {
            get_key_from_property_map(map)
        } else {
            None
        }
    }
    pub fn to_holon_reference(&self) -> HolonReference {
        let smart_reference =
            SmartReference::new(self.to_address.clone(), self.smart_property_values.clone());
        HolonReference::Smart(smart_reference)
    }
}

// UTILITY FUNCTIONS //

pub fn get_all_relationship_links(local_source_id: LocalId) -> Result<Vec<SmartLink>, HolonError> {
    //let link_tag_filter: Option<LinkTag> = None;

    let mut smartlinks: Vec<SmartLink> = Vec::new();

    let links = get_links(
        GetLinksInputBuilder::try_new(local_source_id.0.clone(), LinkTypes::SmartLink)?.build(),
    )
    .map_err(|e| HolonError::from(e))?;

    for link in links {
        let smartlink = get_smartlink_from_link(local_source_id.0.clone(), link)?;
        smartlinks.push(smartlink);
    }

    Ok(smartlinks)
}

/// Gets links for a specific relationship from this source
pub fn get_relationship_links(
    source_action_hash: ActionHash,
    relationship_name: &RelationshipName,
) -> Result<Vec<SmartLink>, HolonError> {
    debug!(
        "Entered get_relationship_links for: {:?}",
        relationship_name.0.to_string()
    );
    // Use the relationship_name reference to encode the link tag
    let link_tag_filter: LinkTag = encode_link_tag(relationship_name.clone(), None)?;

    debug!(
        "getting links for link_tag_filter: {:?}",
        link_tag_filter.clone()
    );

    let mut smartlinks: Vec<SmartLink> = Vec::new();

    // Retrieve links using the specified link tag filter
    let links = get_links(
        GetLinksInputBuilder::try_new(source_action_hash.clone(), LinkTypes::SmartLink)?
            .tag_prefix(link_tag_filter)
            .build(),
    )
    .map_err(|e| HolonError::from(e))?;

    debug!("got {:?} links", links.len());
    // Process each link to convert it into a SmartLink
    for link in links {
        let smartlink = get_smartlink_from_link(source_action_hash.clone(), link)?;
        smartlinks.push(smartlink);
    }

    Ok(smartlinks)
}

pub fn get_smartlink_from_link(
    source_local_hash: ActionHash,
    link: Link,
) -> Result<SmartLink, HolonError> {
    let local_target = link
        .target
        .into_action_hash()
        .ok_or(wasm_error!(WasmErrorInner::Guest(String::from(
            "No action hash associated with link"
        ))))?;

    let link_tag_obj = decode_link_tag(link.tag.clone())?;

    // TODO: Enhance the following to support External HolonIds (by pulling proxy_id from LinkTagObj)
    let smartlink = SmartLink {
        from_address: LocalId(source_local_hash.clone()),
        to_address: LocalId(local_target).into(),
        relationship_name: RelationshipName(MapString(link_tag_obj.relationship_name)),
        smart_property_values: link_tag_obj.smart_property_values,
    };

    Ok(smartlink)
}

pub fn save_smartlink(input: SmartLink) -> Result<(), HolonError> {
    // TODO: convert proxy_id to string

    // TODO: populate from property_map Null-separated property values (serialized into a String) for each of the properties listed in the access path

    let link_tag = encode_link_tag(input.relationship_name.clone(), input.smart_property_values)?;

    create_link(
        input.from_address.clone().0,
        input.to_address.local_id().clone().0,
        LinkTypes::SmartLink,
        link_tag,
    )?;
    Ok(())
}

// HELPER FUNCTIONS //

pub fn decode_link_tag(link_tag: LinkTag) -> Result<LinkTagObject, HolonError> {
    let mut link_tag_object = LinkTagObject::default();
    let bytes = link_tag.into_inner();
    let mut cursor = &bytes[..];

    // Confirm link_tag represents a SmartLink & remove header bytes
    if cursor.starts_with(&SMARTLINK_HEADER_BYTES) {
        cursor = &cursor[SMARTLINK_HEADER_BYTES.len()..];
    } else {
        return Err(HolonError::InvalidParameter(
            "Invalid LinkTag: Missing header bytes".to_string(),
        ));
    }

    let name_end_option = cursor
        .iter()
        .position(|&b| b == RELATIONSHIP_NAME_SEPERATOR.as_bytes()[0]);
    // let name_end_option = cursor
    //     .windows(PROLOG_SEPARATOR.len())
    //     .position(|window| window == PROLOG_SEPARATOR);

    if let Some(name_end) = name_end_option {
        let relationship_name = str::from_utf8(&cursor[..name_end]).map_err(|_| {
            HolonError::Utf8Conversion(
                "LinkTag bytes".to_string(),
                "relationship_name str".to_string(),
            )
        })?;
        link_tag_object.relationship_name = relationship_name.to_string();
        info!("DECODED relationship_name: {:#?}", relationship_name);

        cursor = &cursor[name_end + RELATIONSHIP_NAME_SEPERATOR.len()..];
        // Confirm PROLOG_SEPARATOR reached
        if cursor.starts_with(&PROLOG_SEPERATOR) {
            cursor = &cursor[PROLOG_SEPERATOR.len()..];
        } else {
            return Err(HolonError::InvalidParameter(
                "Invalid LinkTag: Missing PROLOG_SEPARATOR bytes".to_string(),
            ));
        }
    }

    // TODO:
    // -reference_types
    // -proxy_id
    //
    // assuming both are default set to None for now

    // property values //

    // TODO: identify property_value as BaseValue type
    //
    // assuming always String for now

    let mut property_map: PropertyMap = BTreeMap::new();

    while !cursor.is_empty() {
        if cursor.starts_with(&PROPERTY_NAME_SEPERATOR) {
            cursor = &cursor[PROPERTY_NAME_SEPERATOR.len()..];
        } else {
            break;
        }

        let property_name_end_option = cursor
            .iter()
            .position(|&b| b == UNICODE_NUL_STR.as_bytes()[0]);

        if let Some(property_name_end) = property_name_end_option {
            let property_name = str::from_utf8(&cursor[..property_name_end]).map_err(|_| {
                HolonError::Utf8Conversion(
                    "LinkTag bytes".to_string(),
                    "property_name str".to_string(),
                )
            })?;
            info!("property_name: {:#?}", property_name);
            cursor = &cursor[property_name_end + UNICODE_NUL_STR.len()..];

            if cursor.starts_with(&PROPERTY_VALUE_SEPERATOR) {
                cursor = &cursor[PROPERTY_VALUE_SEPERATOR.len()..];
            } else {
                return Err(HolonError::InvalidParameter(
                    format!("Invalid LinkTag: No PROPERTY_VALUE_SEPARATOR found, missing value for property_name: {:?}", property_name)
                ));
            }

            let property_value_end_option = cursor
                .iter()
                .position(|&b| b == UNICODE_NUL_STR.as_bytes()[0]);

            if let Some(property_value_end) = property_value_end_option {
                let property_value =
                    str::from_utf8(&cursor[..property_value_end]).map_err(|_| {
                        HolonError::Utf8Conversion(
                            "LinkTag bytes".to_string(),
                            "property_value str".to_string(),
                        )
                    })?;
                info!("property_value: {:#?}", property_value);
                cursor = &cursor[property_value_end + UNICODE_NUL_STR.len()..];

                property_map.insert(
                    PropertyName(MapString(property_name.to_string())),
                    BaseValue::StringValue(MapString(property_value.to_string())),
                );
            }
        }
    }
    link_tag_object.smart_property_values = Some(property_map);

    Ok(link_tag_object)
}

pub fn encode_link_tag(
    relationship_name: RelationshipName,
    property_values: Option<PropertyMap>,
) -> Result<LinkTag, HolonError> {
    let name = relationship_name.0 .0;

    debug!("Encoding LinkTag for {:?} relationship", name);

    let mut bytes: Vec<u8> = vec![];

    bytes.extend_from_slice(&SMARTLINK_HEADER_BYTES);

    bytes.extend_from_slice(name.as_bytes());

    // TODO: optional descriptor_proxy_id which will need to have a prefix dude Hash being able to contain a Nul byte
    bytes.extend_from_slice(&UNICODE_NUL_STR.as_bytes()); // therefore remove this in the future
    bytes.extend_from_slice(&PROLOG_SEPERATOR);

    // TODO: determine reference type
    // bytes.extend_from_slice(reference_type.as_bytes());
    // bytes.extend_from_slice(UNICODE_NUL_STR.as_bytes());
    // TODO: proxy_id
    // bytes.extend_from_slice(proxy_id_string.as_bytes());
    // bytes.extend_from_slice(UNICODE_NUL_STR.as_bytes());

    if let Some(property_map) = &property_values {
        for (property, value) in property_map {
            bytes.extend_from_slice(&PROPERTY_NAME_SEPERATOR);
            bytes.extend_from_slice(property.0 .0.as_bytes());
            bytes.extend_from_slice(&UNICODE_NUL_STR.as_bytes());
            bytes.extend_from_slice(&PROPERTY_VALUE_SEPERATOR);
            bytes.extend_from_slice(&value.into_bytes().0);
            bytes.extend_from_slice(&UNICODE_NUL_STR.as_bytes());
        }
    }

    Ok(LinkTag(bytes))
}

// fn convert_link_type(link_type: LinkTypes) -> ScopedLinkType {
//     match link_type {
//         LinkTypes::SmartLink => ScopedLinkType::SmartLink,
//         // Add other mappings if needed
//         LinkTypes::HolonNodeUpdates => ScopedLinkType::HolonNodeUpdates,
//         LinkTypes::AllHolonNodes => ScopedLinkType::AllHolonNodes,
//     }
// }

// #[hdk_extern]
// pub fn add_smartlink(input: SmartLink) -> ExternResult<()> {
//     let link_tag = encode_link_tag(input.relationship_name, input.smart_property_values);
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

// UNIT TESTS //

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_encode_and_decode_link_tag() {
        let relationship_name = RelationshipName(MapString("ex_relationship_name".to_string()));
        let mut property_values: PropertyMap = BTreeMap::new();
        let name_1 = PropertyName(MapString("ex_name_1".to_string()));
        let value_1 = BaseValue::StringValue(MapString("ex_value_1".to_string()));
        property_values.insert(name_1, value_1);
        let name_2 = PropertyName(MapString("ex_name_2".to_string()));
        let value_2 = BaseValue::StringValue(MapString("ex_value_2".to_string()));
        property_values.insert(name_2, value_2);
        let name_3 = PropertyName(MapString("ex_name_3".to_string()));
        let value_3 = BaseValue::StringValue(MapString("ex_value_3".to_string()));
        property_values.insert(name_3, value_3);

        let encoded_link_tag =
            encode_link_tag(relationship_name.clone(), Some(property_values.clone())).unwrap();

        let decoded_link_tag_object = decode_link_tag(encoded_link_tag.clone()).unwrap();

        assert_eq!(
            relationship_name.0 .0,
            decoded_link_tag_object.relationship_name
        );
        assert!(decoded_link_tag_object.smart_property_values.is_some());
        assert_eq!(
            Some(property_values),
            decoded_link_tag_object.smart_property_values
        );
    }
}
