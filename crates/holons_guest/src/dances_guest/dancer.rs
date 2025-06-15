use hdk::prelude::*;
use holons_core::HolonsContextBehavior;

use holons_core::core_shared_objects::HolonError;

use crate::init_guest_context;

use holons_core::dances::descriptors_dance_adapter::load_core_schema_dance;
use holons_core::dances::holon_dance_adapter::*;
use holons_core::dances::{
    DanceRequest, DanceResponse, ResponseBody, ResponseStatusCode, SessionState,
};
use base_types::MapString;
use std::collections::HashMap;
use std::sync::Arc;

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

    // -------------------------- ENSURE VALID REQUEST ---------------------------------
    if let Err(status_code) = validate_request(&request) {
        let response = DanceResponse::new(
            status_code,
            MapString("Invalid Request".to_string()),
            ResponseBody::None,
            None,
            request.state.clone(),
        );

        return Ok(response);
    }

    // Initialize the context for this request
    //
    let context = match initialize_context_from_request(&request) {
        Ok(ctx) => ctx,
        Err(error_response) => return Ok(error_response),
    };

    debug!("context and space manager ready to dance");

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

    let dispatch_result = dancer.dispatch(&*context, request.clone());
    let result = process_dispatch_result(&*context, dispatch_result);

    // assert_eq!(result.staging_area.staged_holons.len(), context.get_space_manager().staged_holons.len());

    info!("\n======== RETURNING FROM {:?} Dance with {}", request.dance_name.0, result.summarize());

    Ok(result)
}

// Define a type alias for functions that can be dispatched
type DanceFunction = fn(
    context: &dyn HolonsContextBehavior,
    request: DanceRequest,
) -> Result<ResponseBody, HolonError>;

/// The dispatch table offers the Dancer behaviors including the external API operations of dance and
/// (eventually) undo / redo (see [Command Pattern Wiki](https://en.wikipedia.org/wiki/Command_pattern)
/// or [Implementing Undo/Redo with the Command Pattern video](https://m.youtube.com/watch?v=FM71_a3txTo)).
/// This means that each offered agent action will be
/// implemented with its own **Command object**. These Command objects will implement the
/// `UndoableCommand` Trait. This trait defines: `execute`, `undo` and `redo` functions.
/// Additionally, an `ActionsController` is responsible for executing commands, maintaining the undo
/// and redo stacks, and orchestrating undo and redo operations.
/// * **Asynch vs Synch Commands** -- Commands will support either or both _Asynchronous execution_
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
        context: &dyn HolonsContextBehavior,
        request: DanceRequest,
    ) -> Result<ResponseBody, HolonError> {
        if let Some(func) = self.dispatch_table.get(request.dance_name.0.as_str()) {
            func(context, request)
        } else {
            Err(HolonError::NotImplemented(request.dance_name.0.clone()))
        }
    }
}

/// Creates a `DanceResponse` for cases where `init_context_from_session` fails.
/// Uses the session state from the `DanceRequest` to preserve state integrity.
///
/// # Arguments
/// * `error` - The error that occurred during initialization.
/// * `request` - The original `DanceRequest` containing the session state.
///
/// # Returns
/// A `DanceResponse` with the error details and the original session state.
fn create_error_response(error: HolonError, request: &DanceRequest) -> DanceResponse {
    let error_message = format!("Failed to initialize context: {}", error);
    DanceResponse {
        status_code: ResponseStatusCode::from(error),
        description: MapString(error_message),
        body: ResponseBody::None,
        descriptor: None,
        state: request.get_state().cloned(), // Use the session state from the request
    }
}
fn initialize_context_from_request(
    request: &DanceRequest,
) -> Result<Arc<dyn HolonsContextBehavior>, DanceResponse> {
    info!("Initializing context from request: {:#?}", request);

    // Since `dance()` validates the request, we can safely unwrap the state.
    let session_state = request.state.as_ref().expect("Valid request should have a state");

    let staged_holons = session_state.get_staged_holons().clone();
    let local_space_holon = session_state.get_local_holon_space();

    // Initialize context from session state
    init_guest_context(staged_holons, local_space_holon)
        .map_err(|error| create_error_response(error, request))
}

// fn initialize_context_from_request(
//     request: &DanceRequest,
// ) -> Result<Arc<dyn HolonsContextBehavior>, DanceResponse> {
//     // Extract session state from request
//     let session_state = request.get_state();
//     let staged_holons = session_state.get_staged_holons().clone();
//     let local_space_holon = session_state.get_local_holon_space();
//
//     // Initialize context from session state
//     init_guest_context(staged_holons, local_space_holon)
//         .map_err(|error| create_error_response(error, request))
// }
/// Restores the session state for the DanceResponse from context. This should always
/// be called before returning DanceResponse since the state is intended to be "ping-ponged"
/// between client and guest.
/// NOTE: Errors in restoring the state are not handled (i.e., will cause panic)
fn restore_session_state_from_context(context: &dyn HolonsContextBehavior) -> Option<SessionState> {
    let space_manager = context.get_space_manager();

    // Export staged holons as a single SerializableHolonPool
    let serializable_pool = space_manager.export_staged_holons();

    // Get the local space holon
    let local_space_holon = space_manager.get_space_holon();

    // Construct SessionState with SerializableHolonPool replacing StagingArea
    Some(SessionState::new(serializable_pool, local_space_holon))
}
// fn restore_session_state_from_space_manager(context: &dyn HolonsContextBehavior) -> SessionState {
//     let space_manager = &context.get_space_manager();
//     let staging_area = StagingArea::empty();
//     let staged_holons = space_manager.export_staged_holons();
//     let staged_index = space_manager.export_keyed_index();
//     let staging_area = StagingArea::new_from_references(staged_holons, staged_index);
//     let local_space_holon = space_manager.get_space_holon();
//     SessionState::new(staging_area, local_space_holon)
// }

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
    context: &dyn HolonsContextBehavior, // 🔄 Changed back to `&dyn`
    dispatch_result: Result<ResponseBody, HolonError>,
) -> DanceResponse {
    match dispatch_result {
        Ok(body) => DanceResponse {
            status_code: ResponseStatusCode::OK,
            description: MapString("Success".to_string()),
            body,
            descriptor: None,
            state: restore_session_state_from_context(context),
        },
        Err(error) => {
            let error_message = extract_error_message(&error);
            DanceResponse {
                status_code: ResponseStatusCode::from(error),
                description: MapString(error_message),
                body: ResponseBody::None,
                descriptor: None,
                state: restore_session_state_from_context(context),
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
        | HolonError::DowncastFailure(_)
        | HolonError::DuplicateError(_, _)
        | HolonError::EmptyField(_)
        | HolonError::FailedToBorrow(_)
        | HolonError::HashConversion(_, _)
        | HolonError::HolonNotFound(_)
        | HolonError::IndexOutOfRange(_)
        | HolonError::InvalidHolonReference(_)
        | HolonError::InvalidParameter(_)
        | HolonError::InvalidRelationship(_, _)
        | HolonError::InvalidTransition(_)
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
fn validate_request(request: &DanceRequest) -> Result<(), ResponseStatusCode> {
    // Check if session_state is present
    if request.state.is_none() {
        warn!("Validation failed: Missing session state");
        return Err(ResponseStatusCode::BadRequest);
    }

    // TODO: Add additional validation checks for dance_name, dance_type, etc.

    Ok(())
}
