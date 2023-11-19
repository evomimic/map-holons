use hdk::prelude::*;
//use hdi::prelude::*;
use holons_integrity::*;
#[derive(Serialize, Deserialize, Debug)]
pub struct AddSmartLinkInput {
    pub base_holon_node_hash: ActionHash,
    pub target_holon_node_hash: ActionHash,
    pub tag: LinkTag,
}
#[hdk_extern]
pub fn add_smartlink(
    input: AddSmartLinkInput,
) -> ExternResult<()> {
    create_link(
        input.base_holon_node_hash.clone(),
        input.target_holon_node_hash.clone(),
        LinkTypes::SmartLink,
        input.tag,
    )?;
    Ok(())
}
#[hdk_extern]
pub fn get_smartlinks_for_holon_node(
    holon_node_hash: ActionHash,
) -> ExternResult<Vec<Record>> {
    let links = get_links(holon_node_hash, LinkTypes::SmartLink, None)?;
    let get_input: Vec<GetInput> = links
        .into_iter()
        .map(|link| GetInput::new(
            ActionHash::from(link.target).into(),
            GetOptions::default(),
        ))
        .collect();
    let records: Vec<Record> = HDK
        .with(|hdk| hdk.borrow().get(get_input))?
        .into_iter()
        .filter_map(|r| r)
        .collect();
    Ok(records)
}
#[derive(Serialize, Deserialize, Debug)]
pub struct RemoveSmartLinkInput {
    pub base_holon_node_hash: ActionHash,
    pub target_holon_node_hash: ActionHash,
}
#[hdk_extern]
pub fn remove_smartlink(
    input: RemoveSmartLinkInput,
) -> ExternResult<()> {
    let links = get_links(
        input.base_holon_node_hash.clone(),
        LinkTypes::SmartLink,
        None,
    )?;
    for link in links {
        if ActionHash::from(link.target.clone()).eq(&input.target_holon_node_hash) {
            delete_link(link.create_link_hash)?;
        }
    }
    Ok(())
}
