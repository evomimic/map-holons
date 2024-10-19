use std::collections::HashMap;

use hdk::prelude::*;

//use hdi::map_extern::ExternResult;
use crate::dance_request::DanceRequest;
use holons::context::HolonsContext;
use holons::holon::Holon;
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::holon_space;
use holons::holon_space::HolonSpace;
use holons::holon_space_manager::HolonSpaceManager;
use shared_types_holon::MapString;

use crate::dance_response::{DanceResponse, ResponseBody, ResponseStatusCode};
use crate::descriptors_dance_adapter::*;
use crate::holon_dance_adapter::*;
use crate::session_state::SessionState;

/// The Dancer handles dance() requests on the uniform API and dispatches the Rust function
/// associated with that Dance using its dispatch_table. dance() is also responsible for
/// initializing the context from the session state and, after getting the result of the call,
/// restoring the session state from the context.
///
/// This function always returns a DanceResponse (instead of an Error) because
/// errors are encoded in the DanceResponse's status_code.
#[hdk_extern]
pub fn dance(request: DanceRequest) -> ExternResult<DanceResponse> {
    info!("Entered Dancer::dance() with {:#?}", request);

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
    let holon_space_manager = HolonSpaceManager::new(&context);

    // ------------------ ENSURE LOCAL HOLON SPACE IS IN CONTEXT ---------------------------------
    let space_reference = holon_space_manager.ensure_local_holon_space_in_context();
    if let Err(space_error) = space_reference {
        let error_message = extract_error_message(&space_error);

        // Construct DanceResponse with error details
        let response = DanceResponse {
            status_code: ResponseStatusCode::from(space_error), // Convert HolonError to ResponseStatusCode
            description: MapString(error_message),
            body: ResponseBody::None, // No body since it's an error
            descriptor: None,         // Provide appropriate value if needed
            state: SessionState::restore_session_state_from_context(&context),
        };
        return Ok(response);
    }

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
    info!("dispatching dance");
    let dispatch_result = dancer.dispatch(&context, request);

    let mut result = process_dispatch_result(&context, dispatch_result);

    // assert_eq!(result.staging_area.staged_holons.len(), context.commit_manager.borrow().staged_holons.len());

    info!(
        "======== RETURNING FROM  Dancer::dance() with {:#?}",
        result.clone()
    );

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
        dispatch_table.insert("get_all_holons", get_all_holons_dance as DanceFunction);
        dispatch_table.insert("get_holon_by_id", get_holon_by_id_dance as DanceFunction);
        dispatch_table.insert("stage_new_holon", stage_new_holon_dance as DanceFunction);
        dispatch_table.insert("commit", commit_dance as DanceFunction);
        dispatch_table.insert("delete_holon", delete_holon_dance as DanceFunction);
        dispatch_table.insert("with_properties", with_properties_dance as DanceFunction);

        dispatch_table.insert(
            "abandon_staged_changes",
            abandon_staged_changes_dance as DanceFunction,
        );

        dispatch_table.insert(
            "add_related_holons",
            add_related_holons_dance as DanceFunction,
        );
        dispatch_table.insert(
            "remove_related_holons",
            remove_related_holons_dance as DanceFunction,
        );
        dispatch_table.insert("load_core_schema", load_core_schema_dance as DanceFunction);
        dispatch_table.insert(
            "query_relationships",
            query_relationships_dance as DanceFunction,
        );

        // Add more functions as needed

        Dancer { dispatch_table }
    }
    // Function to register a new function with the dispatch manager
    // If we want to allow dynamic registration, we will need to change the definition of the key
    // in the dispatch_table to String instead of &'static str
    // fn register_function(&mut self, name: String, func: DanceFunction) {
    //     self.dispatch_table.insert(name.clone().as_str(), func);
    // }

    fn dance_name_is_dispatchable(&self, request: DanceRequest) -> bool {
        info!(
            "checking that dance_name: {:#?} is dispatchable",
            request.dance_name.0.as_str()
        );
        self.dispatch_table
            .contains_key(request.dance_name.0.as_str())
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
                state: SessionState::restore_session_state_from_context(context),
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
                state: SessionState::restore_session_state_from_context(context),
            }
        }
    }
}
/// This helper function extracts the error message from a HolonError so that the message
/// can be included in the DanceResponse
fn extract_error_message(error: &HolonError) -> String {
    match error.clone() {
        HolonError::EmptyField(_)
        | HolonError::InvalidParameter(_)
        | HolonError::HolonNotFound(_)
        | HolonError::CommitFailure(_)
        | HolonError::DeletionNotAllowed(_)
        | HolonError::WasmError(_)
        | HolonError::RecordConversion(_)
        | HolonError::InvalidHolonReference(_)
        | HolonError::InvalidType(_)
        | HolonError::IndexOutOfRange(_)
        | HolonError::NotImplemented(_)
        | HolonError::Misc(_)
        | HolonError::MissingStagedCollection(_)
        | HolonError::FailedToBorrow(_)
        | HolonError::UnableToAddHolons(_)
        | HolonError::InvalidRelationship(_, _)
        | HolonError::NotAccessible(_, _)
        | HolonError::UnexpectedValueType(_, _)
        | HolonError::Utf8Conversion(_, _)
        | HolonError::HashConversion(_, _)
        | HolonError::CacheError(_) => error.to_string(),
        HolonError::ValidationError(validation_error) => validation_error.to_string(),
    }
}
