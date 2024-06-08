use hdk::prelude::{info,debug,trace,warn};
use holons::commit_manager::CommitManager;
use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use holons::holon::Holon;
use holons::holon_reference::HolonReference;

use holons::staged_reference::StagedReference;
use shared_types_holon::{MapInteger, MapString};

use crate::descriptor_types::Schema;
use crate::string_descriptor::define_string_type;

/// The load_core_value_types function creates type descriptors for each of the built-in ValueTypes
///
/// The full implementation of this function will emerge incrementally... starting with a minimal schema
///

pub fn load_core_value_types(context: &HolonsContext, schema: &HolonReference) -> Result<(), HolonError> {

    let type_name=MapString("MapString".to_string());
    let type_description = MapString("Built-in MAP String Type".to_string());
    let label = MapString("String".to_string());

    info!("Preparing to stage descriptor for {:#?}", type_name.clone());
    let descriptor = define_string_type(
        context,
        schema,
        type_name.clone(),
        type_description.clone(),
        label.clone(),
        None,
        None,
        None,
        MapInteger(0),
        MapInteger(4096)
    )?;



    info!("Staging complete... committing value type definitions.");

    let response = CommitManager::commit(context);
    info!("Commit response {:#?}", response.clone());

    // TODO: Need to retrieve the saved Schema holon by key once get_holon_by_key dance is available.

    Ok(())

}
