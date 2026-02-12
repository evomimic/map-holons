use std::{collections::HashMap, sync::Arc};

use base_types::MapString;
use core_types::HolonError;
use hdk::prelude::*;
use holons_core::dances::holon_dance_adapter::*;
use holons_core::{
    core_shared_objects::transactions::TransactionContext,
    dances::{DanceRequest, DanceResponse, ResponseBody, ResponseStatusCode},
};

pub(crate) fn dispatch_dance(
    context: &Arc<TransactionContext>,
    request: DanceRequest,
) -> DanceResponse {
    let dancer = Dancer::new();
    let dispatch_result = dancer.dispatch(context, request);
    process_dispatch_result(dispatch_result)
}

// Define a type alias for functions that can be dispatched
type DanceFunction = fn(&Arc<TransactionContext>, DanceRequest) -> Result<ResponseBody, HolonError>;

/// The dispatch table offers the Dancer behaviors including the external API operations of dance and
/// (eventually) undo / redo (see [Command Pattern Wiki](https://en.wikipedia.org/wiki/Command_pattern)
/// or [Implementing Undo/Redo with the Command Pattern video](https://m.youtube.com/watch?v=FM71_a3txTo)).
/// This means that each offered agent action will be
/// implemented with its own **Command object**. These Command objects will implement the
/// `UndoableCommand` Trait. This trait defines: `execute`, `undo` and `redo` functions.
/// Additionally, an `ActionsController` is responsible for executing commands, maintaining the undo
/// and redo stacks, and orchestrating undo and redo operations.
/// * **Async vs Sync Commands** -- Commands will support either or both _Asynchronous execution_
/// (non-blocking), _Synchronous execution_ (blocking).
///
#[derive(Debug)]
struct Dancer {
    pub dispatch_table: HashMap<&'static str, DanceFunction>,
}

impl Dancer {
    fn new() -> Self {
        let mut dispatch_table = HashMap::new();
        // Register functions into the dispatch table
        dispatch_table
            .insert("abandon_staged_changes", abandon_staged_changes_dance as DanceFunction);
        dispatch_table.insert("add_related_holons", add_related_holons_dance as DanceFunction);
        dispatch_table.insert("commit", commit_dance as DanceFunction);
        dispatch_table.insert("delete_holon", delete_holon_dance as DanceFunction);
        dispatch_table.insert("get_all_holons", get_all_holons_dance as DanceFunction);
        dispatch_table.insert("get_holon_by_id", get_holon_by_id_dance as DanceFunction);
        dispatch_table.insert("load_holons", load_holons_dance as DanceFunction);
        dispatch_table.insert("new_holon", new_holon_dance as DanceFunction);
        dispatch_table.insert("query_relationships", query_relationships_dance as DanceFunction);
        dispatch_table.insert("remove_properties", remove_properties_dance as DanceFunction);
        dispatch_table
            .insert("remove_related_holons", remove_related_holons_dance as DanceFunction);
        dispatch_table.insert("stage_new_from_clone", stage_new_from_clone_dance as DanceFunction);
        dispatch_table.insert("stage_new_holon", stage_new_holon_dance as DanceFunction);
        dispatch_table.insert("stage_new_version", stage_new_version_dance as DanceFunction);
        dispatch_table.insert("with_properties", with_properties_dance as DanceFunction);

        // Add more functions (in alphabetical order) as needed

        Dancer { dispatch_table }
    }

    // Function to register a new function with the dispatch manager
    // If we want to allow dynamic registration, we will need to change the definition of the key
    // in the dispatch_table to String instead of `&static str`
    // fn register_function(&mut self, name: String, func: DanceFunction) {
    //     self.dispatch_table.insert(name.clone().as_str(), func);
    // }

    #[allow(dead_code)]
    fn dance_name_is_dispatchable(&self, request: DanceRequest) -> bool {
        info!("checking that dance_name: {:#?} is dispatchable", request.dance_name.0.as_str());
        self.dispatch_table.contains_key(request.dance_name.0.as_str())
    }

    // Function to dispatch a request based on the function name
    fn dispatch(
        &self,
        context: &Arc<TransactionContext>,
        request: DanceRequest,
    ) -> Result<ResponseBody, HolonError> {
        if let Some(func) = self.dispatch_table.get(request.dance_name.0.as_str()) {
            func(context, request)
        } else {
            Err(HolonError::NotImplemented(request.dance_name.0.clone()))
        }
    }
}

/// This function creates a DanceResponse from a `dispatch_result`.
///
/// If `dispatch_result` is `Ok`,
/// * `status_code` is set to Ok,
/// * `description` is set to "Success".
/// * `body` is set to the body returned in the dispatch_result
/// * `descriptor` is all initialized to None
/// If the `dispatch_result` is `Err`,
/// * `status_code` is set from the mapping of HolonError `ResponseStatusCode`
/// * `description` holds the error message associated with the HolonError
/// * `body` and `descriptor` are set to None
///
fn process_dispatch_result(
    dispatch_result: Result<ResponseBody, HolonError>,
) -> DanceResponse {
    match dispatch_result {
        Ok(body) => DanceResponse {
            status_code: ResponseStatusCode::OK,
            description: MapString("Success".to_string()),
            body,
            descriptor: None,
        },
        Err(error) => {
            let error_message = extract_error_message(&error);
            DanceResponse {
                status_code: ResponseStatusCode::from(error),
                description: MapString(error_message),
                body: ResponseBody::None,
                descriptor: None,
            }
        }
    }
}

/// Extracts a user-facing error message from a `HolonError` for inclusion in a `DanceResponse`.
///
/// This stays correct automatically as new `HolonError` variants are added, because
/// `HolonError` derives `thiserror::Error` and its `Display` impl is the source of truth.
fn extract_error_message(error: &HolonError) -> String {
    error.to_string()
}
