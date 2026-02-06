use std::{collections::HashMap, sync::Arc};

use hdk::prelude::*;

use crate::{init_guest_context, GuestHolonService};

use base_types::MapString;
use core_types::{HolonError, HolonId};
use holons_core::dances::dance_request::DanceRequestWire;
use holons_core::dances::dance_response::{DanceResponseWire, ResponseBodyWire};
use holons_core::dances::holon_dance_adapter::*;
use holons_core::{
    core_shared_objects::transactions::TransactionContext,
    dances::{DanceRequest, DanceResponse, ResponseBody, ResponseStatusCode, SessionState},
    HolonReferenceWire, HolonsContextBehavior,
};

/// The Dancer handles dance() requests on the uniform API and dispatches the Rust function
/// associated with that Dance using its dispatch_table. dance() is also responsible for
/// initializing the context from the session_state state and, after getting the result of the call,
/// restoring the session_state state from the context.
///
/// This function always returns a DanceResponse (instead of an Error) because
/// errors are encoded in the DanceResponse's status_code.
#[hdk_extern]
pub fn dance(request: DanceRequestWire) -> ExternResult<DanceResponseWire> {
    info!("\n\n\n***********************  Entered Dancer::dance() with {}", request.summarize());

    // -------------------------- ENSURE VALID REQUEST ---------------------------------
    if let Err(status_code) = validate_request(&request) {
        // Build a *wire* response directly (we do not have a TransactionContext yet).
        let response_wire = DanceResponseWire {
            status_code,
            description: MapString("Invalid Request".to_string()),
            body: ResponseBodyWire::None,
            descriptor: None,
            state: request.state.clone(),
        };

        return Ok(response_wire);
    }

    // -------------------------- INIT CONTEXT FROM SESSION STATE ---------------------------------
    //
    // This is the only place we can create a TransactionContext on the guest side.
    // We intentionally do this *before* context_binding the wire request, because context_binding
    // requires a live TransactionContext for tx_id validation + handle attachment.
    let context = match initialize_context_from_request(&request) {
        Ok(ctx) => ctx,
        Err(error_response_wire) => return Ok(error_response_wire),
    };

    // -------------------------- BIND WIRE → RUNTIME ---------------------------------
    //
    // Validate tx provenance and attach TransactionContextHandle to all embedded references.
    let bound_request = match request.clone().bind(&context) {
        Ok(bound) => bound,
        Err(error) => {
            // Binding failed locally (e.g., cross-transaction reference).
            // Return a wire error response preserving the original session_state state.
            let response_wire = DanceResponseWire {
                status_code: ResponseStatusCode::from(error.clone()),
                description: MapString(format!("Failed to bind DanceRequestWire: {}", error)),
                body: ResponseBodyWire::None,
                descriptor: None,
                state: request.state.clone(),
            };
            return Ok(response_wire);
        }
    };

    debug!("context and space manager ready to dance");

    // -------------------------- DISPATCH ---------------------------------
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

    let dispatch_result = dancer.dispatch(&context, bound_request);
    let result_runtime = process_dispatch_result(&context, dispatch_result);

    // -------------------------- PROJECT RUNTIME → WIRE ---------------------------------
    //
    // IPC boundary must return wire form only.
    let result_wire = DanceResponseWire::from(&result_runtime);

    info!(
        "\n======== RETURNING FROM {:?} Dance with {:?}",
        request.dance_name.0,
        result_wire, //.summarize(),
    );

    Ok(result_wire)
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

/// Creates a `DanceResponseWire` for cases where `init_context_from_session` fails.
/// Uses the session_state state from the `DanceRequest` to preserve state integrity.
///
/// # Arguments
/// * `error` - The error that occurred during initialization.
/// * `request` - The original `DanceRequest` containing the session_state state.
///
/// # Returns
/// A `DanceResponseWire` with the error details and the original session_state state.
fn create_error_response_wire(error: HolonError, request: &DanceRequestWire) -> DanceResponseWire {
    let error_message = format!("Failed to initialize context: {}", error);

    DanceResponseWire {
        status_code: ResponseStatusCode::from(error),
        description: MapString(error_message),
        body: ResponseBodyWire::None,
        descriptor: None,
        state: request.get_state().cloned(), // Use the session_state state from the request
    }
}

fn initialize_context_from_request(
    request: &DanceRequestWire,
) -> Result<Arc<TransactionContext>, DanceResponseWire> {
    info!("==Initializing context from request==");
    debug!("request: {:#?}", request);

    // Since `dance()` validates the request, we can safely unwrap the state.
    let session_state = request.state.as_ref().expect("Valid request should have a state");

    let staged_holons = session_state.get_staged_holons().clone();
    let transient_holons = session_state.get_transient_holons().clone();
    let local_space_holon_wire = session_state.get_local_space_holon_wire();
    let tx_id = session_state.get_tx_id().ok_or_else(|| {
        create_error_response_wire(
            HolonError::InvalidParameter("SessionState missing tx_id".into()),
            request,
        )
    })?;

    // TEMPORARY: extract `Option<HolonId>` from the wire reference.
    // If SessionState stores `Option<HolonId>`, this block goes away.
    let local_space_holon_id: Option<HolonId> = match &local_space_holon_wire {
        Some(wire_ref) => Some(space_holon_id_from_wire_reference(wire_ref).map_err(|e| {
            // If the session_state state carried an invalid space holon reference,
            // treat this as a request/format error (still return a response).
            create_error_response_wire(e, request)
        })?),
        None => None,
    };

    // Initialize context from session_state state pools.
    let context =
        init_guest_context(transient_holons, staged_holons, local_space_holon_id.clone(), tx_id)
            .map_err(|error| create_error_response_wire(error, request))?;

    // Ensure the TransactionContext has a space holon id set.
    //
    // If the session_state state provided one, use it.
    // Otherwise, ensure/create it via guest persistence and then store the id.
    let ensured_space_holon_id: HolonId = match local_space_holon_id {
        Some(id) => id,
        None => {
            // Guest-specific "ensure" requires the concrete GuestHolonService.
            let holon_service = context.get_holon_service();
            let guest_service =
                holon_service.as_any().downcast_ref::<GuestHolonService>().ok_or_else(|| {
                    create_error_response_wire(
                        HolonError::DowncastFailure("GuestHolonService".to_string()),
                        request,
                    )
                })?;

            // Ensure the persisted HolonSpace holon exists and extract its id.
            let ensured_space_ref = guest_service
                .ensure_local_holon_space(&context)
                .map_err(|error| create_error_response_wire(error, request))?;

            match ensured_space_ref {
                holons_core::HolonReference::Smart(smart_ref) => smart_ref
                    .get_id()
                    .map_err(|error| create_error_response_wire(error, request))?,
                other => {
                    return Err(create_error_response_wire(
                        HolonError::InvalidHolonReference(format!(
                            "ensure_local_holon_space returned non-smart reference: {} ({})",
                            other.reference_kind_string(),
                            other.reference_id_string()
                        )),
                        request,
                    ));
                }
            }
        }
    };

    // Store the id on the context / space manager.
    context
        .set_space_holon_id(ensured_space_holon_id)
        .map_err(|error| create_error_response_wire(error, request))?;

    Ok(context)
}

/// Restores the session_state state for the DanceResponse from context. This should always
/// be called before returning DanceResponse since the state is intended to be "ping-ponged"
/// between client and guest.
///
/// NOTE: State restoration is **best-effort**. If exporting staged/transient holons
/// or reading the local space holon fails (e.g., due to lock acquisition errors),
/// this function logs the error and returns `None` instead of panicking.
fn restore_session_state_from_context(context: &Arc<TransactionContext>) -> Option<SessionState> {
    // Export staged holons as a single SerializableHolonPool
    let serializable_staged_pool = match context.export_staged_holons() {
        Ok(pool) => pool,
        Err(error) => {
            warn!(
                "Failed to export staged holons while restoring session_state state: {:?}",
                error
            );
            return None;
        }
    };

    // Export transient holons as a single SerializableHolonPool
    let serializable_transient_pool = match context.export_transient_holons() {
        Ok(pool) => pool,
        Err(error) => {
            warn!(
                "Failed to export transient holons while restoring session_state state: {:?}",
                error
            );
            return None;
        }
    };

    // Get the local space holon (now returns Result<Option<HolonReference>, HolonError>)
    let local_space_holon = match context.get_space_holon() {
        Ok(space_opt) => space_opt,
        Err(error) => {
            warn!(
                "Failed to read local_holon_space while restoring session_state state: {:?}",
                error
            );
            return None;
        }
    };

    // Construct SessionState with SerializableHolonPool replacing StagingArea
    Some(SessionState::new(
        serializable_transient_pool,
        serializable_staged_pool,
        local_space_holon,
        Some(context.tx_id()),
    ))
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
    context: &Arc<TransactionContext>,
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

/// Extracts a user-facing error message from a `HolonError` for inclusion in a `DanceResponse`.
///
/// This stays correct automatically as new `HolonError` variants are added, because
/// `HolonError` derives `thiserror::Error` and its `Display` impl is the source of truth.
fn extract_error_message(error: &HolonError) -> String {
    error.to_string()
}

fn validate_request(request: &DanceRequestWire) -> Result<(), ResponseStatusCode> {
    if request.state.is_none() {
        warn!("Validation failed: Missing session_state state");
        return Err(ResponseStatusCode::BadRequest);
    }

    // TODO: Add additional validation checks for dance_name, dance_type, etc.

    Ok(())
}

// TEMPORARY: remove once SessionState stores HolonId directly
/// Extracts the persisted holon id from a wire reference suitable for anchoring the space holon.
///
/// The space holon must always be persisted, so only SmartReferenceWire is accepted.
pub fn space_holon_id_from_wire_reference(
    reference_wire: &HolonReferenceWire,
) -> Result<HolonId, HolonError> {
    match reference_wire {
        HolonReferenceWire::Smart(smart_wire) => Ok(smart_wire.holon_id().clone()),
        other => Err(HolonError::InvalidHolonReference(format!(
            "Space holon must be a SmartReferenceWire; got {:?}",
            other
        ))),
    }
}
