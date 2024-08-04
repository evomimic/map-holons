use hdk::prelude::*;
use holons_integrity::*;
use shared_types_holon::holon_node::HolonNode;

#[hdk_extern]
pub fn create_holon_node(holon_node: HolonNode) -> ExternResult<Record> {
    let holon_node_hash = create_entry(&EntryTypes::HolonNode(holon_node.clone()))?;
    let record = get(holon_node_hash.clone(), GetOptions::default())?
        .ok_or(
            wasm_error!(
                WasmErrorInner::Guest(String::from("Could not find the newly created HolonNode"))
            ),
        )?;
    trace!("HolonNode successfully created... adding all_holon_nodes link.");
    let path = Path::from("all_holon_nodes");
    create_link(
        path.path_entry_hash()?,
        holon_node_hash.clone(),
        LinkTypes::AllHolonNodes,
        (),
    )?;
    trace!("Returning OK from create_holon_node.");
    Ok(record)
}
#[hdk_extern]
pub fn get_holon_node(
    original_holon_node_hash: ActionHash,
) -> ExternResult<Option<Record>> {
    let links = get_links(GetLinksInputBuilder::try_new(original_holon_node_hash.clone(),LinkTypes::HolonNodeUpdates)?.build())?;
    let latest_link = links
        .into_iter()
        .max_by(|link_a, link_b| link_a.timestamp.cmp(&link_b.timestamp));
    let latest_holon_node_hash = match latest_link {
        Some(link) => link.target.clone().into_action_hash().ok_or(wasm_error!(WasmErrorInner::Guest(String::from("No action hash associated with link"))),)?,
        None => original_holon_node_hash.clone(),
    };
    get(latest_holon_node_hash, GetOptions::default())
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateHolonNodeInput {
    pub original_holon_node_hash: ActionHash,
    pub previous_holon_node_hash: ActionHash,
    pub updated_holon_node: HolonNode,
}
#[hdk_extern]
pub fn update_holon_node(input: UpdateHolonNodeInput) -> ExternResult<Record> {
    let updated_holon_node_hash = update_entry(
        input.previous_holon_node_hash.clone(),
        &input.updated_holon_node,
    )?;
    create_link(
        input.original_holon_node_hash.clone(),
        updated_holon_node_hash.clone(),
        LinkTypes::HolonNodeUpdates,
        (),
    )?;
    let record = get(updated_holon_node_hash.clone(), GetOptions::default())?
        .ok_or(
            wasm_error!(
                WasmErrorInner::Guest(String::from("Could not find the newly updated HolonNode"))
            ),
        )?;
    Ok(record)
}
#[hdk_extern]
pub fn delete_holon_node(
    original_holon_node_hash: ActionHash,
) -> ExternResult<ActionHash> {
    delete_entry(original_holon_node_hash)
}
