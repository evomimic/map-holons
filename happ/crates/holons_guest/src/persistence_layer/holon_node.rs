use hdk::prelude::*;
use holons_guest_integrity::{type_conversions::try_action_hash_from_local_id, HolonNode};
use holons_integrity::*;
use integrity_core_types::HolonNodeModel;

use crate::persistence_layer::holon_storage::persist_holon;
use core_types::HolonWriteRequest;

#[derive(Serialize, Deserialize, Debug)]
pub struct CreatePathInput {
    pub path: Path,
    pub link_type: LinkTypes,
    pub target_holon_node_hash: ActionHash,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetPathInput {
    pub path: Path,
    pub link_type: LinkTypes,
}

/// Publishes a HolonNode as the root of a new lineage and returns its record.
///
/// Delegates to `persistence_layer::holon_storage::persist_holon` so there is exactly one write
/// path for holon nodes, and one place that decides which substrate action a write becomes. This
/// extern remains as a raw authoring probe for tests that need a record back; production code
/// calls `persist_holon` directly and never handles a `Record`.
#[hdk_extern]
pub fn create_holon_node(holon_node: HolonNode) -> ExternResult<Record> {
    let stored = persist_holon(HolonWriteRequest::PublishRoot {
        holon_node: HolonNodeModel::from(holon_node),
    })
    .map_err(|e| wasm_error!(WasmErrorInner::Guest(e.to_string())))?;

    let holon_node_hash = try_action_hash_from_local_id(&stored.version_metadata.version_id)
        .map_err(|e| wasm_error!(WasmErrorInner::Guest(e.to_string())))?;

    trace!("Returning OK from create_holon_node.");
    get(holon_node_hash, GetOptions::default())?.ok_or(wasm_error!(WasmErrorInner::Guest(
        String::from("Could not find the newly created HolonNode")
    )))
}
#[hdk_extern]
pub fn create_path_to_holon_node(input: CreatePathInput) -> ExternResult<ActionHash> {
    let result = create_link(
        input.path.path_entry_hash()?,
        input.target_holon_node_hash.clone(),
        input.link_type,
        (),
    )?;
    Ok(result)
}

#[hdk_extern]
pub fn delete_holon_node(original_holon_node_hash: ActionHash) -> ExternResult<ActionHash> {
    // delete links to Local Holon Space
    let local_space_path = Path::from("local_holon_space");
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

/// Enumerates revisions reachable through `HolonNodeUpdates` links.
///
/// Scaffolded, and currently reports only the record it was given: MAP authors versions as native
/// root-addressed updates and deliberately does not maintain a parallel `HolonNodeUpdates` link
/// index, so there are no links for this to follow. Revision-history traversal is later storage
/// work, which will decide whether to build on Holochain's own update graph (`get_details`) or on
/// the link index — it should not harden both.
#[hdk_extern]
pub fn get_all_revisions_for_holon_node(
    original_holon_node_hash: ActionHash,
) -> ExternResult<Vec<Record>> {
    let Some(original_record) =
        get_original_holon_node_with_details(original_holon_node_hash.clone())?
    else {
        return Ok(vec![]);
    };
    let links_query =
        LinkQuery::try_new(original_holon_node_hash.clone(), LinkTypes::HolonNodeUpdates)?;
    let links = get_links(links_query, GetStrategy::default())?;
    let get_input: Vec<GetInput> = links
        .into_iter()
        .map(|link| {
            Ok(GetInput::new(
                link.target
                    .into_action_hash()
                    .ok_or(wasm_error!(WasmErrorInner::Guest(
                        "No action hash associated with link".to_string()
                    )))?
                    .into(),
                GetOptions::default(),
            ))
        })
        .collect::<ExternResult<Vec<GetInput>>>()?;
    let records = HDK.with(|hdk| hdk.borrow().get(get_input))?;
    let mut records: Vec<Record> = records.into_iter().flatten().collect();
    records.insert(0, original_record);
    Ok(records)
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

/// Selects the newest revision reachable through `HolonNodeUpdates` links.
///
/// Scaffolded, and currently equivalent to `get_original_holon_node`: MAP authors no
/// `HolonNodeUpdates` links (see `get_all_revisions_for_holon_node`), so this always falls through
/// to the hash it was given. Note that hash is now a lineage *root* rather than the only version
/// of a holon, so this returns the root's content, not the lineage head. Head selection is later
/// storage work.
#[hdk_extern]
pub fn get_latest_holon_node(original_holon_node_hash: ActionHash) -> ExternResult<Option<Record>> {
    let links_query =
        LinkQuery::try_new(original_holon_node_hash.clone(), LinkTypes::HolonNodeUpdates)?;
    let links = get_links(links_query, GetStrategy::default())?;
    let latest_link =
        links.into_iter().max_by(|link_a, link_b| link_a.timestamp.cmp(&link_b.timestamp));
    let latest_holon_node_hash = match latest_link {
        Some(link) => link.target.clone().into_action_hash().ok_or(wasm_error!(
            WasmErrorInner::Guest("No action hash associated with link".to_string())
        ))?,
        None => original_holon_node_hash.clone(),
    };
    get(latest_holon_node_hash, GetOptions::default())
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
