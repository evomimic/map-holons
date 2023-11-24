use hdk::prelude::*;
use holons_integrity::*;
#[hdk_extern]
pub fn get_all_holon_nodes(_: ()) -> ExternResult<Vec<Record>> {
    println!("Trace Entry: get_all_holon_nodes()");
    let path = Path::from("all_holon_nodes");
    println!("Trace calling get_links()");
    let links = get_links(path.path_entry_hash()?, LinkTypes::AllHolonNodes, None)?;
    println!("Trace returned from get_links()");
    let get_input: Vec<GetInput> = links
        .into_iter()
        .map(|link| GetInput::new(
            ActionHash::from(link.target).into(),
            GetOptions::default(),
        ))
        .collect();
    let records = HDK.with(|hdk| hdk.borrow().get(get_input))?;
    let records: Vec<Record> = records.into_iter().filter_map(|r| r).collect();
    println!("Trace about to return Ok from get_all_holon_nodes");
    Ok(records)
}
