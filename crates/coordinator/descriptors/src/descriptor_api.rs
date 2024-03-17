use crate::loader::*;
/// This file defines the descriptor functions exposed via hdk_extern
///
use hdk::prelude::*;

use holons::context::HolonsContext;
use holons::holon::Holon;

#[hdk_extern]
pub fn load_core_schema_api(_: ()) -> ExternResult<Holon> {
    let context = HolonsContext::new();
    match load_core_schema(&context) {
        Ok(result) => Ok(result.clone_holon(&context)),
        Err(holon_error) => Err(holon_error.into()),
    }
}

// match holon.commit() {
// Ok(result)=> Ok(result.clone()),
// Err(holon_error) => {
// Err(holon_error.into())
// }
// }
