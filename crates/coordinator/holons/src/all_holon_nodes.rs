use hdk::prelude::*;
use holons_integrity::*;
#[hdk_extern]
pub fn get_all_holon_nodes(_: ()) -> ExternResult<Vec<Record>> {
    let path = Path::from("all_holon_nodes");
     let links = get_links(GetLinksInputBuilder::try_new(path.path_entry_hash()?,LinkTypes::AllHolonNodes)?.build())?;
    let get_input: Vec<GetInput> = links
        .into_iter()
        .map(|link| GetInput::new(
            link.target.try_into().unwrap(),
            GetOptions::default(),
        ))
        .collect();
    let records = HDK.with(|hdk| hdk.borrow().get(get_input))?;
    let records: Vec<Record> = records.into_iter().filter_map(|r| r).collect();
    Ok(records)
}
