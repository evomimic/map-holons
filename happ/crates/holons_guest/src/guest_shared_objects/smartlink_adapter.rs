use base_types::MapString;
use core_types::{ExternalId, HolonError, HolonId};
use hdi::prelude::*;
use hdk::prelude::*;
use holons_core::core_shared_objects::holon::key_from_property_map;
use holons_guest_integrity::type_conversions::*;
use holons_integrity::LinkTypes;
use integrity_core_types::{LocalId, PropertyMap, RelationshipName};
use shared_validation::{decode_link_tag, encode_link_tag, encode_link_tag_prolog};

#[derive(Serialize, Deserialize, Debug)]
pub struct SmartLink {
    pub from_address: LocalId,
    pub to_address: HolonId,
    pub relationship_name: RelationshipName,
    pub forward_link_provenance: Option<LocalId>,
    pub smart_property_values: Option<PropertyMap>,
}

impl SmartLink {
    /// The implementation of this function currently relies on a "key" property being stored
    /// in the property_map. The intended design is to derive the key from the HolonDescriptor's
    /// KEY_PROPERTIES relationship. However, the current parameters to this function are not
    /// sufficient to do this.
    /// TODO: update this function to align with described key property list design
    pub fn key(&self) -> Result<Option<MapString>, HolonError> {
        if let Some(ref map) = self.smart_property_values {
            key_from_property_map(map)
        } else {
            Ok(None)
        }
    }

    /// Returns the persisted target identity and any cached smart properties.
    /// This is context-free and does not mint runtime references.
    pub fn to_pointer(&self) -> (HolonId, Option<PropertyMap>) {
        (self.to_address.clone(), self.smart_property_values.clone())
    }
}

// This link query defaults on all fields; `GetStrategy::default()` performs a network fetch.
pub fn fetch_links_to_all_holons() -> Result<Vec<HolonId>, HolonError> {
    let path = Path::from("all_holon_nodes");
    let base_address = path.path_entry_hash().map_err(holon_error_from_wasm_error)?;
    let links_query = LinkQuery::try_new(base_address, LinkTypes::AllHolonNodes)
        .map_err(holon_error_from_wasm_error)?;
    let links =
        get_links(links_query, GetStrategy::default()).map_err(holon_error_from_wasm_error)?;
    let mut holon_ids = Vec::new();
    info!(
        "Retrieved {:?} links for 'all_holon_nodes' path, converting to SmartLinks..",
        links.len()
    );

    for link in links {
        let holon_id = HolonId::Local(local_id_from_action_hash(
            link.target.clone().into_action_hash().ok_or(HolonError::HashConversion(
                "Source/Base".to_string(),
                "ActionHash".to_string(),
            ))?,
        ));
        holon_ids.push(holon_id);
    }

    Ok(holon_ids)
}

/// Gets all SmartLinks across all relationships from the given source.
pub fn get_all_relationship_links(local_source_id: &LocalId) -> Result<Vec<SmartLink>, HolonError> {
    let base_address = try_action_hash_from_local_id(local_source_id)?;
    let links_query = LinkQuery::try_new(base_address, LinkTypes::SmartLink)
        .map_err(holon_error_from_wasm_error)?;
    let links =
        get_links(links_query, GetStrategy::default()).map_err(holon_error_from_wasm_error)?;
    let mut smartlinks = Vec::with_capacity(links.len());
    debug!("Got {:?} links", links.len());

    for link in links {
        smartlinks
            .push(get_smartlink_from_link(try_action_hash_from_local_id(local_source_id)?, link)?);
    }

    Ok(smartlinks)
}

/// Gets links for a specific relationship from this source.
pub fn get_relationship_links(
    source_action_hash: ActionHash,
    relationship_name: &RelationshipName,
) -> Result<Vec<SmartLink>, HolonError> {
    debug!("Entered get_relationship_links for: {:?}", relationship_name.0.to_string());
    let link_tag_filter = LinkTag(
        encode_link_tag_prolog(relationship_name).map_err(smartlink_tag_error_to_holon_error)?,
    );

    let links_query = LinkQuery::try_new(source_action_hash.clone(), LinkTypes::SmartLink)
        .map_err(holon_error_from_wasm_error)?
        .tag_prefix(link_tag_filter);
    let links =
        get_links(links_query, GetStrategy::default()).map_err(holon_error_from_wasm_error)?;
    let mut smartlinks = Vec::with_capacity(links.len());
    debug!("Got {:?} links", links.len());

    for link in links {
        let smartlink = get_smartlink_from_link(source_action_hash.clone(), link)?;
        debug!("Got SmartLink {:?}", smartlink);
        smartlinks.push(smartlink);
    }

    Ok(smartlinks)
}

/// Converts a Holochain Link into a SmartLink.
fn get_smartlink_from_link(
    source_local_hash: ActionHash,
    link: Link,
) -> Result<SmartLink, HolonError> {
    let local_target = link
        .target
        .into_action_hash()
        .ok_or(wasm_error!(WasmErrorInner::Guest(String::from(
            "No action hash associated with link"
        ))))
        .map_err(holon_error_from_wasm_error)?;
    let link_tag_object =
        decode_link_tag(&link.tag.into_inner()).map_err(smartlink_tag_error_to_holon_error)?;

    let to_address = match link_tag_object.proxy_id {
        Some(proxy_id) => HolonId::External(ExternalId {
            space_id: proxy_id,
            local_id: local_id_from_action_hash(local_target),
        }),
        None => HolonId::Local(local_id_from_action_hash(local_target)),
    };

    Ok(SmartLink {
        from_address: local_id_from_action_hash(source_local_hash),
        to_address,
        relationship_name: RelationshipName(MapString(link_tag_object.relationship_name)),
        forward_link_provenance: link_tag_object.forward_link_provenance,
        smart_property_values: link_tag_object.smart_property_values,
    })
}

/// Persists a SmartLink, suppressing the write when an equivalent link already exists.
///
/// Equivalence is source `LocalId` + target action hash + byte-identical encoded tag. The
/// canonical shared codec encodes `PropertyMap` in `BTreeMap` order, making repeated commit
/// passes idempotent at this choke point.
pub fn save_smartlink(input: SmartLink) -> Result<(), HolonError> {
    let link_tag = LinkTag(
        encode_link_tag(
            &input.relationship_name,
            &input.to_address,
            input.smart_property_values.as_ref(),
            input.forward_link_provenance.as_ref(),
        )
        .map_err(smartlink_tag_error_to_holon_error)?,
    );
    let source_hash = try_action_hash_from_local_id(&input.from_address)?;
    let target_hash = try_action_hash_from_local_id(input.to_address.local_id())?;

    if equivalent_smartlink_exists(&source_hash, &target_hash, &link_tag)? {
        debug!(
            "SmartLink already persisted for relationship {:?} from {:?}; skipping duplicate write",
            input.relationship_name.0 .0, input.from_address
        );
        return Ok(());
    }

    create_link(source_hash, target_hash, LinkTypes::SmartLink, link_tag)
        .map_err(holon_error_from_wasm_error)?;

    Ok(())
}

/// Returns true when a live SmartLink from `source_hash` to `target_hash` carrying exactly
/// `link_tag` already exists.
///
/// The full encoded tag serves as the narrowest possible `tag_prefix` filter. Exact bytes are
/// compared after querying because prefix filtering alone does not establish equivalence.
fn equivalent_smartlink_exists(
    source_hash: &ActionHash,
    target_hash: &ActionHash,
    link_tag: &LinkTag,
) -> Result<bool, HolonError> {
    let links_query = LinkQuery::try_new(source_hash.clone(), LinkTypes::SmartLink)
        .map_err(holon_error_from_wasm_error)?
        .tag_prefix(link_tag.clone());
    let links =
        get_links(links_query, GetStrategy::default()).map_err(holon_error_from_wasm_error)?;

    Ok(links.into_iter().any(|link| {
        link.tag == *link_tag && link.target.into_action_hash().as_ref() == Some(target_hash)
    }))
}

fn smartlink_tag_error_to_holon_error(error: impl std::fmt::Display) -> HolonError {
    HolonError::InvalidParameter(format!("Invalid SmartLink tag: {error}"))
}
