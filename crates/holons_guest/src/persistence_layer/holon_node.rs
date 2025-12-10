use hdk::prelude::*;
use holons_guest_integrity::HolonNode;
use holons_integrity::*;

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

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateHolonNodeInput {
    pub original_holon_node_hash: ActionHash,
    pub previous_holon_node_hash: ActionHash,
    pub updated_holon_node: HolonNode,
}

#[hdk_extern]
pub fn create_holon_node(holon_node: HolonNode) -> ExternResult<Record> {
    let holon_node_hash = create_entry(&EntryTypes::HolonNode(holon_node.clone()))?;
    let record = get(holon_node_hash.clone(), GetOptions::default())?.ok_or(wasm_error!(
        WasmErrorInner::Guest(String::from("Could not find the newly created HolonNode"))
    ))?;
    debug!("HolonNode successfully created... adding all_holon_nodes link.");
    let path = Path::from("all_holon_nodes");
    // path.ensure()?;
    create_link(path.path_entry_hash()?, holon_node_hash.clone(), LinkTypes::AllHolonNodes, ())?;
    trace!("Returning OK from create_holon_node.");
    Ok(record)
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
    // delete link to all_holon_nodes anchor
    let all_nodes_path = Path::from("all_holon_nodes");
    let links = get_links(
        GetLinksInputBuilder::try_new(all_nodes_path.path_entry_hash()?, LinkTypes::AllHolonNodes)?
            .build(),
    )?;
    for link in links {
        if let Some(hash) = link.target.into_action_hash() {
            if hash == original_holon_node_hash {
                delete_link(link.create_link_hash)?;
            }
        }
    }
    // delete links to Local Holon Space
    let local_space_path = Path::from("local_holon_space");
    let links = get_links(
        GetLinksInputBuilder::try_new(
            local_space_path.path_entry_hash()?,
            LinkTypes::LocalHolonSpace,
        )?
        .build(),
    )?;
    for link in links {
        if let Some(hash) = link.target.into_action_hash() {
            if hash == original_holon_node_hash {
                delete_link(link.create_link_hash)?;
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
pub fn get_all_revisions_for_holon_node(
    original_holon_node_hash: ActionHash,
) -> ExternResult<Vec<Record>> {
    let Some(original_record) =
        get_original_holon_node_with_details(original_holon_node_hash.clone())?
    else {
        return Ok(vec![]);
    };
    let links = get_links(
        GetLinksInputBuilder::try_new(
            original_holon_node_hash.clone(),
            LinkTypes::HolonNodeUpdates,
        )?
        .build(),
    )?;
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
    let links = get_links(
        GetLinksInputBuilder::try_new(input.path.path_entry_hash()?, input.link_type)?.build(),
    )?;
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

#[hdk_extern]
pub fn get_original_holon_node(
    original_holon_node_hash: ActionHash,
) -> ExternResult<Option<Record>> {
    get(original_holon_node_hash, GetOptions::default())
}

#[hdk_extern]
pub fn get_latest_holon_node(original_holon_node_hash: ActionHash) -> ExternResult<Option<Record>> {
    let links = get_links(
        GetLinksInputBuilder::try_new(
            original_holon_node_hash.clone(),
            LinkTypes::HolonNodeUpdates,
        )?
        .build(),
    )?;
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

#[hdk_extern]
pub fn update_holon_node(input: UpdateHolonNodeInput) -> ExternResult<Record> {
    let updated_holon_node_hash =
        update_entry(input.previous_holon_node_hash.clone(), &input.updated_holon_node)?;
    create_link(
        input.original_holon_node_hash.clone(),
        updated_holon_node_hash.clone(),
        LinkTypes::HolonNodeUpdates,
        (),
    )?;
    let record =
        get(updated_holon_node_hash.clone(), GetOptions::default())?.ok_or(wasm_error!(
            WasmErrorInner::Guest(String::from("Could not find the newly updated HolonNode"))
        ))?;
    Ok(record)
}
