use hdi::prelude::*;
use hdk::prelude::*;
use holons_integrity::LinkTypes;
use holons_integrity::*;
use shared_types_holon::{
    BaseValue, ExternalId, HolonId, HolonSpaceId, LocalId, MapString, PropertyMap, PropertyName,
    PropertyValue,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LinkTagObject {
    pub relationship_name: String,
    pub holon_id: HolonId,
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
    // proxy_id: Option<HolonSpaceId>,
) -> Result<Vec<SmartLink>, HolonError> {
    debug!(
        "Entered get_relationship_links for: {:?}",
        relationship_name.0.to_string()
    );
    // Use the relationship_name reference to encode the link tag
    let link_tag_filter: LinkTag = encode_link_tag_prolog(relationship_name)?;

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

    let smartlink = SmartLink {
        from_address: LocalId(source_local_hash.clone()),
        to_address: link_tag_obj.holon_id,
        relationship_name: RelationshipName(MapString(link_tag_obj.relationship_name)),
        smart_property_values: link_tag_obj.smart_property_values,
    };

    Ok(smartlink)

}

pub fn save_smartlink(input: SmartLink) -> Result<(), HolonError> {
    // TODO: populate from property_map Null-separated property values (serialized into a String) for each of the properties listed in the access path

    let link_tag = encode_link_tag(
        &input.relationship_name,
        input.to_address.clone(),
        input.smart_property_values,
    )?;
    create_link(
        input.from_address.0.clone(),
        input.to_address.local_id().0.clone(),
        LinkTypes::SmartLink,
        link_tag,
    )?;

    Ok(())
// }
//
//     match input.to_address.clone() {
//         HolonId::External(external_id) => {
//             let link_tag = encode_link_tag(
//                 input.relationship_name.clone(),
//                 Some(external_id.space_id),
//                 input.smart_property_values,
//             )?;
//
//             create_link(
//                 input.from_address.0.clone(),
//                 input.to_address.local_id().0.clone(),
//                 LinkTypes::SmartLink,
//                 link_tag,
//             )?;
//
//             Ok(())
//         }
//         HolonId::Local(_local_id) => {
//             let link_tag = encode_link_tag(
//                 input.relationship_name.clone(),
//                 None,
//                 input.smart_property_values,
//             )?;
//
//             create_link(
//                 input.from_address.0.clone(),
//                 input.to_address.local_id().0.clone(),
//                 LinkTypes::SmartLink,
//                 link_tag,
//             )?;
//
//             Ok(())
//         }
//     }
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
        .position(|&b| b == RELATIONSHIP_NAME_SEPARATOR.as_bytes()[0]);
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
        debug!("DECODED relationship_name: {:#?}", relationship_name);

        cursor = &cursor[name_end + RELATIONSHIP_NAME_SEPARATOR.len()..];
        // Confirm PROLOG_SEPARATOR reached
        if cursor.starts_with(&PROLOG_SEPARATOR) {
            cursor = &cursor[PROLOG_SEPARATOR.len()..];
        } else {
            return Err(HolonError::InvalidParameter(
                "Invalid LinkTag: Missing PROLOG_SEPARATOR bytes".to_string(),
            ));
        }
    } else {
        return Err(HolonError::InvalidParameter(
            "Invalid LinkTag: Missing RELATIONSHIP_NAME_SEPARATOR bytes".to_string(),
        ));
    }

    let proxy_id_end_option = cursor
        .iter()
        .position(|&b| b == PROXY_ID_SEPARATOR.as_bytes()[0]);

    if let Some(proxy_id_end) = proxy_id_end_option {
        link_tag_object.proxy_id = Some(HolonSpaceId(
            ActionHash::from_raw_39(cursor[..proxy_id_end].to_vec()).map_err(|_| {
                HolonError::HashConversion(
                    "link_tag proxy_id bytes".to_string(),
                    "ActionHash".to_string(),
                )
            })?,
        ));
        debug!("DECODED proxy_id: {:#?}", link_tag_object.proxy_id.clone());
        cursor = &cursor[proxy_id_end + RELATIONSHIP_NAME_SEPARATOR.len()..];
    } else {
        return Err(HolonError::InvalidParameter(
            "Invalid LinkTag: Missing PROXY_ID_SEPARATOR bytes".to_string(),
        ));
    }

    // TODO:
    // -reference_types
    //
    // assuming the default is set to None for now

    // property values //

    // TODO: identify property_value as BaseValue type
    //
    // assuming always String for now

    let mut property_map: PropertyMap = BTreeMap::new();

    // Iterate over each PropertyName and Value pair, adding them to the property_map
    while !cursor.is_empty() {
        if cursor.starts_with(&PROPERTY_NAME_SEPARATOR) {
            cursor = &cursor[PROPERTY_NAME_SEPARATOR.len()..];
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
            debug!("property_name: {:#?}", property_name);
            cursor = &cursor[property_name_end + UNICODE_NUL_STR.len()..];

            if cursor.starts_with(&PROPERTY_VALUE_SEPARATOR) {
                cursor = &cursor[PROPERTY_VALUE_SEPARATOR.len()..];
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
                debug!("property_value: {:#?}", property_value);
                cursor = &cursor[property_value_end + UNICODE_NUL_STR.len()..];

                property_map.insert(
                    PropertyName(MapString(property_name.to_string())),
                    BaseValue::StringValue(MapString(property_value.to_string())),
                );
            }
        }
    }
    link_tag_object.smart_property_values = Some(property_map);
    debug!(
        "DECODED {:#?}",
        link_tag_object.smart_property_values.clone()
    );

    Ok(link_tag_object)
}

pub fn encode_link_tag(
    relationship_name: &RelationshipName,
    to_address: HolonId,
    property_values: Option<PropertyMap>,
) -> Result<LinkTag, HolonError> {

    debug!("ENCODING LinkTag for {:?} relationship", relationship_name);
    let mut bytes = encode_link_tag_prolog(relationship_name)?.into_inner();

    if let HolonId::External(external_id) = to_address {
        bytes.extend_from_slice(EXTERNAL_REFERENCE_TYPE.as_bytes());
        bytes.extend_from_slice(UNICODE_NUL_STR.as_bytes());
        bytes.extend_from_slice(&external_id.space_id.0.into_inner());
        bytes.extend_from_slice(UNICODE_NUL_STR.as_bytes());
    }
    else {
        bytes.extend_from_slice(LOCAL_REFERENCE_TYPE.as_bytes());
        bytes.extend_from_slice(UNICODE_NUL_STR.as_bytes());
    }

    if let Some(property_map) = &property_values {
        for (property, value) in property_map {
            bytes.extend_from_slice(&PROPERTY_NAME_SEPARATOR);
            bytes.extend_from_slice(property.0 .0.as_bytes());
            bytes.extend_from_slice(&UNICODE_NUL_STR.as_bytes());
            bytes.extend_from_slice(&PROPERTY_VALUE_SEPARATOR);
            bytes.extend_from_slice(&value.into_bytes().0);
            bytes.extend_from_slice(&UNICODE_NUL_STR.as_bytes());
        }
    }

    Ok(LinkTag(bytes))
}
pub fn encode_link_tag_prolog(
    relationship_name: &RelationshipName,
) -> Result<LinkTag, HolonError> {
    let name = relationship_name.clone().0;

    debug!("ENCODING LinkTag Filter for {:?} relationship", name);

    let mut bytes: Vec<u8> = vec![];

    bytes.extend_from_slice(&SMARTLINK_HEADER_BYTES);

    bytes.extend_from_slice(name.as_bytes());

    bytes.extend_from_slice(&UNICODE_NUL_STR.as_bytes()); // therefore remove this in the future
    bytes.extend_from_slice(&PROLOG_SEPARATOR);

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

        let space_id = HolonSpaceId(ActionHash::try_from("uhCkkRCrWQQJ95dvwNDgGeRHwJQVjcrvKrmuDf6T0iylizE2gWyHC").unwrap());
        let local_id = LocalId(ActionHash::try_from("uhCkkLQ8hxxrt27W8TtkpcX1XAqbUyfD5_Rv5Us0X").unwrap());

        let holon_id = HolonId::External(ExternalId {
            space_id, local_id });

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

        let encoded_link_tag = encode_link_tag(
            &relationship_name,
            holon_id,
            Some(property_values.clone()),
        )
        .unwrap();

        let decoded_link_tag_object = decode_link_tag(encoded_link_tag.clone()).unwrap();

        assert_eq!(
            relationship_name.0 .0,
            decoded_link_tag_object.relationship_name
        );
        assert_eq!(space_id, decoded_link_tag_object.proxy_id);
        assert!(decoded_link_tag_object.smart_property_values.is_some());
        assert_eq!(
            Some(property_values),
            decoded_link_tag_object.smart_property_values
        );
    }
}
