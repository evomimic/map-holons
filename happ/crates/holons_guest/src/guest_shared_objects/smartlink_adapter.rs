use base_types::MapString;
use core_types::{
    decode_smartlink_tag, encode_smartlink_tag, smartlink_relationship_prefix, CanonicalKey,
    HolonError, HolonId, SmartLinkId, SmartLinkTagInput, TargetPropertyCacheCandidate,
};
use hdi::prelude::*;
use hdk::prelude::*;
use holons_guest_integrity::type_conversions::*;
use holons_integrity::LinkTypes;
use integrity_core_types::{LocalId, PropertyMap, RelationshipName};

#[derive(Serialize, Deserialize, Debug)]
pub struct SmartLink {
    pub smartlink_id: Option<SmartLinkId>,
    pub from_address: LocalId,
    pub to_address: HolonId,
    pub relationship_name: RelationshipName,
    pub canonical_key: CanonicalKey,
    pub relationship_property_values: PropertyMap,
    pub target_property_values: PropertyMap,
}

impl SmartLink {
    pub fn key(&self) -> Result<Option<MapString>, HolonError> {
        Ok((!self.canonical_key.as_str().is_empty())
            .then(|| MapString(self.canonical_key.as_str().to_string())))
    }

    /// Returns the persisted target identity and any cached smart properties.
    /// This is context-free and does not mint runtime references.
    pub fn to_pointer(&self) -> (HolonId, Option<PropertyMap>) {
        (
            self.to_address.clone(),
            (!self.target_property_values.is_empty()).then(|| self.target_property_values.clone()),
        )
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
        smartlink_relationship_prefix(relationship_name)
            .map_err(smartlink_tag_error_to_holon_error)?,
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
        decode_smartlink_tag(&link.tag.into_inner(), local_id_from_action_hash(local_target))
            .map_err(smartlink_tag_error_to_holon_error)?;

    Ok(SmartLink {
        smartlink_id: Some(SmartLinkId(local_id_from_action_hash(link.create_link_hash))),
        from_address: local_id_from_action_hash(source_local_hash),
        to_address: link_tag_object.target_id,
        relationship_name: link_tag_object.relationship_name,
        canonical_key: link_tag_object.canonical_key,
        relationship_property_values: link_tag_object.relationship_property_values,
        target_property_values: link_tag_object.target_property_values,
    })
}

/// Persists a SmartLink and returns its create action hash.
///
/// An equivalent persisted link returns its canonical existing action hash instead of writing a
/// duplicate. Deterministic selection keeps replay behavior independent of DHT query ordering.
pub fn save_smartlink(input: SmartLink) -> Result<ActionHash, HolonError> {
    let target_property_cache_candidates = input
        .target_property_values
        .iter()
        .map(|(property_name, value)| TargetPropertyCacheCandidate {
            property_name: property_name.clone(),
            value: value.clone(),
        })
        .collect();
    let link_tag = LinkTag(
        encode_smartlink_tag(&SmartLinkTagInput {
            target_id: input.to_address.clone(),
            relationship_name: input.relationship_name.clone(),
            canonical_key: input.canonical_key,
            occurrence_id: None,
            relationship_property_values: input.relationship_property_values,
            target_property_cache_candidates,
        })
        .map_err(smartlink_tag_error_to_holon_error)?,
    );
    let source_hash = try_action_hash_from_local_id(&input.from_address)?;
    let target_hash = try_action_hash_from_local_id(input.to_address.local_id())?;

    if let Some(existing_action_hash) =
        equivalent_smartlink_action_hash(&source_hash, &target_hash, &link_tag)?
    {
        debug!(
            "SmartLink already persisted for relationship {:?} from {:?}; skipping duplicate write",
            input.relationship_name.0 .0, input.from_address
        );
        return Ok(existing_action_hash);
    }

    create_link(source_hash, target_hash, LinkTypes::SmartLink, link_tag)
        .map_err(holon_error_from_wasm_error)
}

/// Returns the canonical existing action hash for an equivalent live SmartLink.
///
/// The full encoded tag serves as the narrowest possible `tag_prefix` filter. Exact bytes are
/// compared after querying because prefix filtering alone does not establish equivalence. If old
/// data contains multiple exact matches, select the lowest `ActionHash` so replay behavior is
/// stable regardless of DHT query ordering.
fn equivalent_smartlink_action_hash(
    source_hash: &ActionHash,
    target_hash: &ActionHash,
    link_tag: &LinkTag,
) -> Result<Option<ActionHash>, HolonError> {
    let links_query = LinkQuery::try_new(source_hash.clone(), LinkTypes::SmartLink)
        .map_err(holon_error_from_wasm_error)?
        .tag_prefix(link_tag.clone());
    let links =
        get_links(links_query, GetStrategy::default()).map_err(holon_error_from_wasm_error)?;

    Ok(links
        .into_iter()
        .filter(|link| {
            link.tag == *link_tag
                && link.target.clone().into_action_hash().as_ref() == Some(target_hash)
        })
        .map(|link| link.create_link_hash)
        .min())
}

fn smartlink_tag_error_to_holon_error(error: impl std::fmt::Display) -> HolonError {
    HolonError::InvalidWireFormat {
        wire_type: "SmartLinkTagV1".to_string(),
        reason: error.to_string(),
    }
}
