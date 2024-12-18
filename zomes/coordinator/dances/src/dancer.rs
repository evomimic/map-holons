use std::collections::HashMap;

use hdk::prelude::*;
use holons::space_manager::{HolonStageQuery, HolonStagingBehavior};
//use hdi::map_extern::ExternResult;
use crate::dance_request::DanceRequest;
use crate::dance_response::{DanceResponse, ResponseBody, ResponseStatusCode};
use crate::descriptors_dance_adapter::*;
use crate::holon_dance_adapter::*;
use crate::session_state::SessionState;
use crate::staging_area::StagingArea;
use holons::context::HolonsContext;
use holons::holon_error::HolonError;
use shared_types_holon::MapString;

use crate::holon_dance_adapter::{
    abandon_staged_changes_dance, add_related_holons_dance, commit_dance, get_all_holons_dance,
    get_holon_by_id_dance, query_relationships_dance, remove_related_holons_dance,
    stage_new_from_clone_dance, stage_new_holon_dance, stage_new_version_dance,
    with_properties_dance,
};

/// The Dancer handles dance() requests on the uniform API and dispatches the Rust function
/// associated with that Dance using its dispatch_table. dance() is also responsible for
/// initializing the context from the session state and, after getting the result of the call,
/// restoring the session state from the context.
///
/// This function always returns a DanceResponse (instead of an Error) because
/// errors are encoded in the DanceResponse's status_code.
#[hdk_extern]
pub fn dance(request: DanceRequest) -> ExternResult<DanceResponse> {
    info!("\n\n\n***********************  Entered Dancer::dance() with {}", request.summarize());

    // -------------------------- ENSURE VALID REQUEST---------------------------------
    let valid = true; // TODO: Validate the dance request

    if !valid {
        let response = DanceResponse::new(
            ResponseStatusCode::BadRequest,
            MapString("Invalid Request".to_string()),
            ResponseBody::None,
            None,
            request.get_state().clone(),
        );

        return Ok(response);
    }

    let context = request.init_context_from_state();
    debug!("context initialized");
    let mut mutable_space_manager = context.space_manager.borrow_mut();

    // ------------------ ENSURE LOCAL SPACE HOLON IS COMMITTED ---------------------------------

    //note at this point the space_manager cannot be borrowed until mutable release
    let space_reference = mutable_space_manager.ensure_local_holon_space(&context);
    if let Err(space_error) = space_reference {
        let error_message = extract_error_message(&space_error);

        //release the mutable borrow of the space manager
        drop(mutable_space_manager);

        // Construct DanceResponse with error details
        let response = DanceResponse {
            status_code: ResponseStatusCode::from(space_error), // Convert HolonError to ResponseStatusCode
            description: MapString(error_message),
            body: ResponseBody::None, // No body since it's an error
            descriptor: None,         // Provide appropriate value if needed
            state: restore_session_state_from_space_manager(&context),
        };
        return Ok(response);
    }
    //release the mutable borrow of the space manager
    drop(mutable_space_manager);
    debug!("space manager ready to dance");

    // Get the Dancer
    let dancer = Dancer::new();

    // TODO: If the request is a Command, add the request to the undo_list
    // info!("confirm dance is dispatchable");
    //
    // // Dispatch the dance and map result to DanceResponse
    // if !dancer.dance_name_is_dispatchable(request.clone()) {
    //     return Err(HolonError::NotImplemented(
    //         "No function to dispatch in dispatch table".to_string(),
    //     )
    //     .into());
    // }
    debug!("dispatching dance");
    let dispatch_result = dancer.dispatch(&context, request.clone());

    let result = process_dispatch_result(&context, dispatch_result);

    // assert_eq!(result.staging_area.staged_holons.len(), context.space_manager.borrow().staged_holons.len());

    info!("\n======== RETURNING FROM {:?} Dance with {}", request.dance_name.0, result.summarize());

    Ok(result)
}

// Define a type alias for functions that can be dispatched
type DanceFunction =
    fn(context: &HolonsContext, request: DanceRequest) -> Result<ResponseBody, HolonError>;

// Define a struct to manage the dispatch table and offer the Dancer behaviors including the external
// API operations of dance and (eventually) undo / redo (see [Command Pattern Wiki]
// (https://en.wikipedia.org/wiki/Command_pattern) or [Implementing Undo/Redo with the Command Pattern video]
// (https://m.youtube.com/watch?v=FM71_a3txTo)). This means that each offered agent action will be implemented with its own **Command object**. These Command objects will implement the `UndoableCommand` Trait. This trait defines: `execute`, `undo` and `redo` functions. Additionally, an `ActionsController` is responsible for executing commands, maintaining the undo and redo stacks, and orchestrating undo and redo operations.
// * **Asynch vs Synch Commands** -- Commands will support either or both _Asynchronous execution_ (non-blocking), _Synchronous execution_ (blocking).
// * Command Response and Results objects --
//
// ## Commands
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
        dispatch_table.insert("load_core_schema", load_core_schema_dance as DanceFunction);
        dispatch_table.insert("query_relationships", query_relationships_dance as DanceFunction);
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
    // in the dispatch_table to String instead of &'static str
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
        context: &HolonsContext,
        request: DanceRequest,
    ) -> Result<ResponseBody, HolonError> {
        if let Some(func) = self.dispatch_table.get(request.dance_name.0.as_str()) {
            func(context, request)
        } else {
            Err(HolonError::NotImplemented(request.dance_name.0.clone()))
            // Err(HolonError::InvalidParameter("couldn't find some dance in the dispatch table".to_string()))
        }
    }
}

/// Restores the session state for the DanceResponse from context. This should always
/// be called before returning DanceResponse since the state is intended to be "ping-ponged"
/// between client and guest.
/// NOTE: Errors in restoring the state are not handled (i.e., will cause panic)
pub fn restore_session_state_from_space_manager(context: &HolonsContext) -> SessionState {
    let space_manager = &context.space_manager.borrow();
    let staged_holons = space_manager.get_staged_holons();
    let staged_index = space_manager.get_stage_key_index();
    let staging_area = StagingArea::new_from_references(staged_holons, staged_index);
    let local_space_holon = space_manager.get_space_holon();
    SessionState::new(staging_area, local_space_holon)
}

/// This function creates a DanceResponse from a `dispatch_result`.
///
/// If `dispatch_result` is `Ok`,
/// * `status_code` is set to Ok,
/// * `description` is set to "Success".
/// * `body` is set to the body returned in the dispatch_result
/// * `descriptor` is all initialized to None
/// * `state` is restored from context
///
/// If the `dispatch_result` is `Err`,
/// * `status_code` is set from the mapping of HolonError `ResponseStatusCode`
/// * `description` holds the error message associated with the HolonError
/// * `body` and `descriptor` are set to None
/// * `state` is restored from context
///

fn process_dispatch_result(
    context: &HolonsContext,
    dispatch_result: Result<ResponseBody, HolonError>,
) -> DanceResponse {
    match dispatch_result {
        Ok(body) => {
            // If the dispatch_result is Ok, construct DanceResponse with appropriate fields
            DanceResponse {
                status_code: ResponseStatusCode::OK,
                description: MapString("Success".to_string()),
                body,
                descriptor: None,
                state: restore_session_state_from_space_manager(context),
            }
        }
        Err(error) => {
            let error_message = extract_error_message(&error);
            // Construct DanceResponse with error details
            DanceResponse {
                status_code: ResponseStatusCode::from(error), // Convert HolonError to ResponseStatusCode
                description: MapString(error_message),
                body: ResponseBody::None, // No body since it's an error
                descriptor: None,         // Provide appropriate value if needed
                state: restore_session_state_from_space_manager(context),
            }
        }
    }
}
/// This helper function extracts the error message from a HolonError so that the message
/// can be included in the DanceResponse
fn extract_error_message(error: &HolonError) -> String {
    match error.clone() {
        HolonError::CacheError(_)
        | HolonError::CommitFailure(_)
        | HolonError::DeletionNotAllowed(_)
        | HolonError::EmptyField(_)
        | HolonError::FailedToBorrow(_)
        | HolonError::HashConversion(_, _)
        | HolonError::HolonNotFound(_)
        | HolonError::IndexOutOfRange(_)
        | HolonError::InvalidHolonReference(_)
        | HolonError::InvalidParameter(_)
        | HolonError::InvalidRelationship(_, _)
        | HolonError::InvalidType(_)
        | HolonError::InvalidUpdate(_)
        | HolonError::Misc(_)
        | HolonError::MissingStagedCollection(_)
        | HolonError::NotAccessible(_, _)
        | HolonError::NotImplemented(_)
        | HolonError::RecordConversion(_)
        | HolonError::UnableToAddHolons(_)
        | HolonError::UnexpectedValueType(_, _)
        | HolonError::Utf8Conversion(_, _)
        | HolonError::WasmError(_) => error.to_string(),
        HolonError::ValidationError(validation_error) => validation_error.to_string(),
    }
}
