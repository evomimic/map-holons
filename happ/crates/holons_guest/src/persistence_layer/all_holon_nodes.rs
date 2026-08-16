use core_types::{HolonError, HolonId};
use hdk::prelude::*;
use holons_guest_integrity::{type_conversions::*, ALL_HOLON_NODES_PATH};
use holons_integrity::*;
//TODO: move this function to holon_node.rs and delete the file

/// Get all the HolonNodes from the HolonSpace, one per lineage.
///
/// The `AllHolonNodes` index holds one entry per lineage: a version-producing write is an update
/// addressed at its lineage root and deliberately adds no index entry, so a lineage never appears
/// more than once here.
///
/// Each result is therefore a lineage *root*, which once a lineage has versions is no longer its
/// current content. Head selection is later storage work; until then a caller needing the current
/// version traverses `Successor` from the root.
#[hdk_extern]
pub fn get_all_holon_nodes(_: ()) -> ExternResult<Vec<Record>> {
    let path = Path::from(ALL_HOLON_NODES_PATH);
    let base_address = path.path_entry_hash()?;
    let links_query = LinkQuery::try_new(base_address, LinkTypes::AllHolonNodes)?;
    let links = get_links(links_query, GetStrategy::default())?;
    info!("Retrieved {:?} links for 'all_holon_nodes' path", links.len());
    let get_input: Vec<GetInput> = links
        .into_iter()
        .map(|link| GetInput::new(link.target.try_into().unwrap(), GetOptions::default()))
        .collect();
    let records = HDK.with(|hdk| hdk.borrow().get(get_input))?;
    Ok(records.into_iter().flatten().collect())
}

/// Get the `HolonId` of every HolonNode in the HolonSpace, one per lineage.
///
/// Reads the same `AllHolonNodes` global index as `get_all_holon_nodes`, but returns
/// identities rather than records, so callers that only need ids skip the record fetch.
/// The one-entry-per-lineage and lineage-root caveats documented above apply here too.
///
/// This global-index reader is preserved until the `AllHolonNodes` index itself is retired, which
/// waits on a coordinator-owned replacement for whole-space discovery (Storage SL5b, #631).
/// It moved here from the retired guest SmartLink facade (Storage SL4, #630): despite
/// the old `fetch_links_to_all_holons` name it never returned links, and `AllHolonNodes`
/// is not a SmartLink relationship.
// This link query defaults on all fields; `GetStrategy::default()` performs a network fetch.
pub fn get_all_holon_ids() -> Result<Vec<HolonId>, HolonError> {
    let path = Path::from(ALL_HOLON_NODES_PATH);
    let base_address = path.path_entry_hash().map_err(holon_error_from_wasm_error)?;
    let links_query = LinkQuery::try_new(base_address, LinkTypes::AllHolonNodes)
        .map_err(holon_error_from_wasm_error)?;
    let links =
        get_links(links_query, GetStrategy::default()).map_err(holon_error_from_wasm_error)?;
    info!("Retrieved {:?} links for 'all_holon_nodes' path, converting to HolonIds..", links.len());

    let mut holon_ids = Vec::with_capacity(links.len());
    for link in links {
        let holon_id = HolonId::Local(local_id_from_action_hash(
            link.target.clone().into_action_hash().ok_or(HolonError::HashConversion(
                "Target".to_string(),
                "ActionHash".to_string(),
            ))?,
        ));
        holon_ids.push(holon_id);
    }

    Ok(holon_ids)
}
