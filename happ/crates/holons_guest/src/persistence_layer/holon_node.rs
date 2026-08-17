//! Scaffolded HolonNode externs, retained from the Holochain scaffolding tool.
//!
//! Version-aware persistence lives in `holon_storage`; this module holds the remaining scaffolded
//! externs that predate it and are kept for API stability and for later storage work.
//!
//! # Naming
//!
//! The `original_holon_node_hash` parameters here predate version-aware storage and are not
//! renamed, because they are part of an existing extern surface. Read them as **lineage root**:
//! a holon is now addressed by the `Create` that began its lineage, and subsequent versions are
//! updates rooted at it. See `core_types::holon_storage` for the intended vocabulary.

use hdk::prelude::*;
use holons_guest_integrity::LOCAL_HOLON_SPACE_PATH;
use holons_integrity::*;

#[derive(Serialize, Deserialize, Debug)]
pub struct GetPathInput {
    pub path: Path,
    pub link_type: LinkTypes,
}

#[hdk_extern]
pub fn delete_holon_node(original_holon_node_hash: ActionHash) -> ExternResult<ActionHash> {
    // delete links to Local Holon Space
    let local_space_path = Path::from(LOCAL_HOLON_SPACE_PATH);
    let base_address = local_space_path.path_entry_hash()?;
    let links_query = LinkQuery::try_new(base_address, LinkTypes::LocalHolonSpace)?;
    let links = get_links(links_query, GetStrategy::default())?;

    for link in links {
        if let Some(hash) = link.target.into_action_hash() {
            if hash == original_holon_node_hash {
                delete_link(link.create_link_hash, GetOptions::default())?;
            }
        }
    }

    delete_entry(original_holon_node_hash)
}

#[hdk_extern]
pub fn get_all_deletes_for_holon_node(
    original_holon_node_hash: ActionHash,
) -> ExternResult<Option<Vec<SignedActionHashed>>> {
    let Some(details) = get_details(original_holon_node_hash, GetOptions::default())? else {
        return Ok(None);
    };
    match details {
        Details::Entry(_) => Err(wasm_error!(WasmErrorInner::Guest("Malformed details".into()))),
        Details::Record(record_details) => Ok(Some(record_details.deletes)),
    }
}

#[hdk_extern]
pub fn get_holon_node_by_path(input: GetPathInput) -> ExternResult<Option<Record>> {
    let links_query = LinkQuery::try_new(input.path.path_entry_hash()?, input.link_type)?;
    let links = get_links(links_query, GetStrategy::default())?;
    let latest_link =
        links.into_iter().max_by(|link_a, link_b| link_a.timestamp.cmp(&link_b.timestamp));
    let latest_holon_node_hash = match latest_link {
        Some(link) => link.target.clone().into_action_hash().ok_or(wasm_error!(
            WasmErrorInner::Guest(String::from("No action hash associated with link"))
        ))?,
        None => return Ok(None),
    };
    get(latest_holon_node_hash, GetOptions::default())
}

/// Raw scaffolded read of one exact action, returning its `Record`.
///
/// The name is misleading and retained only for the scaffolded surface: this does not walk a
/// lineage to find an original. New code uses `holon_storage::get_holon`, which is
/// version-addressed by name and returns record-derived `VersionMetadata` instead of a `Record`.
#[hdk_extern]
pub fn get_original_holon_node(
    original_holon_node_hash: ActionHash,
) -> ExternResult<Option<Record>> {
    get(original_holon_node_hash, GetOptions::default())
}

#[hdk_extern]
pub fn get_oldest_delete_for_holon_node(
    original_holon_node_hash: ActionHash,
) -> ExternResult<Option<SignedActionHashed>> {
    let Some(mut deletes) = get_all_deletes_for_holon_node(original_holon_node_hash)? else {
        return Ok(None);
    };
    deletes.sort_by(|delete_a, delete_b| {
        delete_a.action().timestamp().cmp(&delete_b.action().timestamp())
    });
    Ok(deletes.first().cloned())
}

#[hdk_extern]
pub fn get_original_holon_node_with_details(
    original_holon_node_hash: ActionHash,
) -> ExternResult<Option<Record>> {
    let Some(details) = get_details(original_holon_node_hash, GetOptions::default())? else {
        return Ok(None);
    };
    match details {
        Details::Record(details) => Ok(Some(details.record)),
        _ => Err(wasm_error!(WasmErrorInner::Guest("Malformed get details response".to_string()))),
    }
}
