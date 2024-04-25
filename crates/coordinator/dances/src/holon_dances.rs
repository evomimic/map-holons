/// This file defines the functions exposed via hdk_extern
///
use hdk::prelude::*;

use holons::context::HolonsContext;
use holons::holon::Holon;
use holons::holon_errors::HolonError;
use shared_types_holon::MapInteger;
use crate::dance_request::{DanceRequest};

use crate::dance_response::{ResponseBody};
// type DanceFunction = fn(context: &HolonsContext, request:DanceRequest) -> Result<ResponseBody, HolonError>;
/// Create a new holon that can be incrementally built up prior to commit.
/// As a dance adaptor, this function wraps (and insulates) the native functionality in Dance
/// and insulates the native function from any dependency on Dances. In general, this means:
/// 1.  Extracting any required input parameters from the DanceRequest's request_body
/// 2.  Invoking the native function
/// 3.  Creating a DanceResponse based on the results returned by the native function. This includes,
/// mapping any errors into an appropriate ResponseStatus and returning results in the body.
///
pub fn stage_new_holon_dance(context: &HolonsContext, _request: DanceRequest) -> Result<ResponseBody, HolonError> {
    // TODO: add support for descriptor parameter
    //
    //
    let new_holon = Holon:: new();
    let staged_reference = context.commit_manager.borrow_mut().stage_holon(new_holon);
    // This operation will have added the staged_holon to the CommitManager's vector and returned a
    // StagedReference to it.


    let index = MapInteger(staged_reference.holon_index.try_into().expect("Conversion failed"));
    Ok(ResponseBody::Index(index))

}


// // pub fn stage_new_holon(_:()) -> ExternResult<Holon> {Ok(Holon::new())}
// #[derive(Clone, Serialize, Deserialize, Debug)]
// pub struct WithPropertyInput {
//     pub holon: Holon,
//     pub property_name:PropertyName,
//     pub value: BaseValue,
// }
//
// #[hdk_extern]
// pub fn with_property_value(input: WithPropertyInput) -> ExternResult<Holon> {
//     let mut holon = input.holon.clone();
//     holon.with_property_value(
//         input.property_name.clone(),
//         input.value.clone());
//     Ok(holon)
// }
// #[hdk_extern]
// pub fn get_holon(
//     id: HolonId,
// ) -> ExternResult<Option<Holon>> {
//        match Holon::get_holon(id) {
//         Ok(result) => Ok(result),
//         Err(holon_error) => {
//             Err(holon_error.into())
//         }
//     }
// }
//
// #[hdk_extern]
// pub fn commit(input: Holon) -> ExternResult<Holon> {
//     let holon = input.clone();
//     // // quick exit to test error return
//     // return Err(HolonError::NotImplemented("load_core_schema_aoi".to_string()).into());
//     match holon.commit() {
//         Ok(result)=> Ok(result.clone()),
//         Err(holon_error) => {
//             Err(holon_error.into())
//         }
//     }
//
// }
//
// #[hdk_extern]
// pub fn get_all_holons(
//    _: (),
// ) -> ExternResult<Vec<Holon>> {
//     match Holon::get_all_holons() {
//         Ok(result) => Ok(result),
//         Err(holon_error) => {
//             Err(holon_error.into())
//         }
//     }
//
// }
// #[hdk_extern]
// pub fn delete_holon(
//     target_holon_id: ActionHash,
// ) -> ExternResult<ActionHash> {
//     match delete_holon_node(target_holon_id) {
//         Ok(result) => Ok(result),
//         Err(holon_error) => {
//             Err(holon_error.into())
//         }
//     }
// }





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
