use std::collections::HashMap;

use hdk::prelude::*;

//use hdi::map_extern::ExternResult;
use holons::cache_manager::HolonCacheManager;
use holons::commit_manager::CommitManager;
use holons::context::HolonsContext;
use holons::holon_errors::HolonError;
use shared_types_holon::MapString;
use crate::dance_request::DanceRequest;

use crate::dance_response::{DanceResponse, ResponseBody, ResponseStatusCode};
use crate::holon_dances::stage_new_holon_dance;

/// The Dancer handles dance() requests on the uniform API and dispatches the Rust function
/// associated with that Dance using its dispatch_table. dance() is also responsible for
/// initializing the context (including converting the StagingArea into CommitManager) and,
/// after getting the result of the call, converting the CommitManager back into StagingArea
///

#[hdk_extern]
pub fn dance(request:DanceRequest)->ExternResult<DanceResponse> {

    // TODO: Validate the dance request

    let mut result = DanceResponse {
        status_code: ResponseStatusCode::NotImplemented,
        description: MapString("Invalid Request".to_string()),
        body: None,
        descriptor: None,
        staging_area: request.staging_area.clone(),
    };

    // Initialize the context, mapping the StagingArea (if there is one) into a CommitManager

    let mut commit_manager = CommitManager::new();

    if let Some(staging_area) = request.clone().staging_area {
        commit_manager = staging_area.to_commit_manager();
    }
    let context = HolonsContext::init_context(
        commit_manager,
        HolonCacheManager::new(),
    );

    // Get the Dancer
    let dancer = Dancer::new();

    // TODO: If the request is a Command, add the request to the undo_list

    // Dispatch the dance and map result to DanceResponse
    let dispatch_result = dancer.dispatch(&context, request);
    // if Ok,

    if let Ok(body) = dispatch_result {
        result.body = Some(body);
        result.status_code = ResponseStatusCode::Ok;
    } else {
        result.status_code = ResponseStatusCode::ServiceUnavailable;


    }


    // Restore the StagingArea from CommitManager




    // TODO: Restore the StagingArea from CommitManager



    Ok(result)
}

// Define a type alias for functions that can be dispatched
type DanceFunction = fn(context: &HolonsContext, request:DanceRequest) -> Result<ResponseBody, HolonError>;

// Define a struct to manage the dispatch table and offer the Dancer behaviors including the external
// API operations of dance and (eventually) undo / redo (see [Command Pattern Wiki]
// (https://en.wikipedia.org/wiki/Command_pattern) or [Implementing Undo/Redo with the Command Pattern video]
// (https://m.youtube.com/watch?v=FM71_a3txTo)). This means that each offered agent action will be implemented with its own **Command object**. These Command objects will implement the `UndoableCommand` Trait. This trait defines: `execute`, `undo` and `redo` functions. Additionally, an `ActionsController` is responsible for executing commands, maintaining the undo and redo stacks, and orchestrating undo and redo operations.
// * **Asynch vs Synch Commands** -- Commands will support either or both _Asynchronous execution_ (non-blocking), _Synchronous execution_ (blocking).
// * Command Response and Results objects --
//
// ## Commands
struct Dancer {
    pub dispatch_table: HashMap<&'static str, DanceFunction>,
}

impl Dancer {

    fn new() -> Self {
        let mut dispatch_table = HashMap::new();

        // Register functions into the dispatch table
        dispatch_table.insert("stage_new_holon", stage_new_holon_dance as DanceFunction);

        // Add more functions as needed

        Dancer { dispatch_table }
    }
    // Function to register a new function with the dispatch manager
    // If we want to allow dynamic registration, we will need to change the definition of the key
    // in the dispatch_table to String instead of &'static str
    // fn register_function(&mut self, name: String, func: DanceFunction) {
    //     self.dispatch_table.insert(name.clone().as_str(), func);
    // }

    // Function to dispatch a request based on the function name
    fn dispatch(&self, context: &HolonsContext, request:DanceRequest) -> Result<ResponseBody,HolonError> {
        if let Some(func) = self.dispatch_table.get(request.dance_name.0.as_str()) {
            func(context, request)
        } else {
            Err(HolonError::NotImplemented(request.dance_name.0.clone()))

        }
    }
}

fn process_dispatch_result(dispatch_result: Result<ResponseBody, HolonError>) -> DanceResponse {
    match dispatch_result {
        Ok(body) => {
            // If the dispatch_result is Ok, construct DanceResponse with appropriate fields
            DanceResponse {
                status_code: ResponseStatusCode::Ok,
                description: MapString("Success".to_string()),
                body: Some(body),
                descriptor: None, // Provide appropriate value if needed
                staging_area: None, // Provide appropriate value if needed
            }
        }
        Err(error) => {
            // If the dispatch_result is an error, extract the associated string value
            let error_message = match error.clone() {
                HolonError::EmptyField(msg)
                | HolonError::HolonNotFound(msg)
                | HolonError::WasmError(msg)
                | HolonError::RecordConversion(msg)
                | HolonError::InvalidHolonReference(msg)
                | HolonError::IndexOutOfRange(msg)
                | HolonError::NotImplemented(msg)
                | HolonError::MissingStagedCollection(msg)
                | HolonError::FailedToBorrow(msg)
                | HolonError::UnableToAddHolons(msg)
                | HolonError::InvalidRelationship(msg, _)
                | HolonError::CacheError(msg) => msg,
            };

            // Construct DanceResponse with error details
            DanceResponse {
                status_code: ResponseStatusCode::from(error), // Convert HolonError to ResponseStatusCode
                description: MapString("Fill in Error description here".to_string()),
                body: None, // No body since it's an error
                descriptor: None, // Provide appropriate value if needed
                staging_area: None, // Provide appropriate value if needed
                //error_message: Some(error_message), // Set the error message
            }
        }
    }
}