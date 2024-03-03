/// This file defines the functions exposed via hdk_extern
///
use hdk::prelude::*;
use shared_types_holon::holon_node::{PropertyName};
use shared_types_holon::value_types::BaseValue;
use crate::holon::Holon;
use crate::holon_node::delete_holon_node;


#[hdk_extern]
pub fn new_holon(_:()) -> ExternResult<Holon> {Ok(Holon::new())}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct WithPropertyInput {
    pub holon: Holon,
    pub property_name:PropertyName,
    pub value: BaseValue,
}

#[hdk_extern]
pub fn with_property_value(input: WithPropertyInput) -> ExternResult<Holon> {
    let mut holon = input.holon.clone();
    holon.with_property_value(
        input.property_name.clone(),
        input.value.clone());
    Ok(holon)
}

#[hdk_extern]
pub fn commit(input: Holon) -> ExternResult<Holon> {
    let holon = input.clone();
    match holon.commit() {
        Ok(result)=> Ok(result.clone()),
        Err(holon_error) => {
            Err(holon_error.into())
        }
    }

}

#[hdk_extern]
pub fn get_all_holons(
   _: (),
) -> ExternResult<Vec<Holon>> {
    match Holon::get_all_holons() {
        Ok(result) => Ok(result),
        Err(holon_error) => {
            Err(holon_error.into())
        }
    }

}
#[hdk_extern]
pub fn delete_holon(
    target_holon_id: ActionHash,
) -> ExternResult<ActionHash> {
    match delete_holon_node(target_holon_id) {
        Ok(result) => Ok(result),
        Err(holon_error) => {
            Err(holon_error.into())
        }
    }
}





/*
#[derive(Serialize, Deserialize, Debug)]
pub struct UpdateHolonNodeInput {
    pub original_holon_hash: ActionHash,
    pub previous_holon_hash: ActionHash,
    pub updated_holon: HolonNode,
}
#[hdk_extern]
pub fn update_holon(input: UpdateHolonNodeInput) -> ExternResult<Record> {
    let updated_holon_hash = update_entry(
        input.previous_holon_hash.clone(),
        &input.updated_holon,
    )?;
    create_link(
        input.original_holon_hash.clone(),
        updated_holon_hash.clone(),
        LinkTypes::HolonNodeUpdates,
        (),
    )?;
    let record = get(updated_holon_hash.clone(), GetOptions::default())?
        .ok_or(
            wasm_error!(
                WasmErrorInner::Guest(String::from("Could not find the newly updated HolonNode"))
            ),
        )?;
    Ok(record)
}

 */
