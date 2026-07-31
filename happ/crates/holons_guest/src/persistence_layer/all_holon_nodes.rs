use crate::persistence_layer::holon_node::get_latest_holon_node;
use hdk::prelude::*;
use holons_integrity::*;
//TODO: move this function to holon_node.rs and delete the file

/// Get all the HolonNodes from the HolonSpace, one per lineage.
///
/// The `AllHolonNodes` index holds one entry per lineage: a version-producing write is an update
/// addressed at its lineage root and deliberately adds no index entry, so a lineage never appears
/// more than once here.
///
/// The stated intent of returning the *latest* version is not met. `get_latest_holon_node` has no
/// `HolonNodeUpdates` links to follow, so each result is the lineage root — which, once a lineage
/// has versions, is no longer its current content. Head selection is later storage work; until
/// then a caller needing the current version traverses `Successor` from the root.
#[hdk_extern]
pub fn get_all_holon_nodes(_: ()) -> ExternResult<Vec<Record>> {
    let path = Path::from("all_holon_nodes");
    let base_address = path.path_entry_hash()?;
    let links_query = LinkQuery::try_new(base_address, LinkTypes::AllHolonNodes)?;
    let links = get_links(links_query, GetStrategy::default())?;
    info!("Retrieved {:?} links for 'all_holon_nodes' path", links.len());
    let get_input: Vec<GetInput> = links
        .into_iter()
        .map(|link| GetInput::new(link.target.try_into().unwrap(), GetOptions::default()))
        .collect();
    let records = HDK.with(|hdk| hdk.borrow().get(get_input))?;
    let records: Vec<Record> = records.into_iter().filter_map(|r| r).collect();
    let mut latest_records = Vec::new();
    for record in &records {
        if let Some(latest_record) = get_latest_holon_node(record.action_address().clone())? {
            latest_records.push(latest_record);
        }
    }
    Ok(latest_records)
}
