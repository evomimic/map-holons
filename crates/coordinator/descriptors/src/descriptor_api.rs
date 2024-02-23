/// This file defines the descriptor functions exposed via hdk_extern
///
use hdk::prelude::*;
use holons::commit_manager::CommitManager;
use holons::context::HolonsContext;
use holons::holon::Holon;
use crate::loader::*;

#[hdk_extern]
pub fn load_core_schema_api(_:()) -> ExternResult<Holon> {
    let mut context = HolonsContext {
        commit_manager: CommitManager::new().into()
    };
    match load_core_schema(&context) {
        Ok(result) => Ok(result.clone_holon()),
        Err(holon_error) => {
            Err(holon_error.into())
        }
    }
}

// match holon.commit() {
// Ok(result)=> Ok(result.clone()),
// Err(holon_error) => {
// Err(holon_error.into())
// }
// }
