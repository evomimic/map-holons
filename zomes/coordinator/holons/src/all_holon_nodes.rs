use hdk::prelude::*;
use holons_integrity::*;

use crate::holon_node::get_latest_holon_node;

//TODO: move this function to holon_node.rs and delete the file

/// Get all the HolonNodes from the HolonSpace. In a case where a Holon has more than one version, only return the latest version.
#[hdk_extern]
pub fn get_all_holon_nodes(_: ()) -> ExternResult<Vec<Record>> {
    let path = Path::from("all_holon_nodes");
    let links = get_links(
        GetLinksInputBuilder::try_new(path.path_entry_hash()?, LinkTypes::AllHolonNodes)?.build(),
    )?;
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
