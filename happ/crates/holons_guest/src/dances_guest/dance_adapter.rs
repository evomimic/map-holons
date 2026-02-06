use std::sync::Arc;

use hdk::prelude::*;

use crate::dances_guest::dancer::dispatch_dance;
use crate::{init_guest_context, GuestHolonService};

use base_types::MapString;
use core_types::{HolonError, HolonId};
use holons_boundary::{
    DanceRequestWire, DanceResponseWire, HolonReferenceWire, ResponseBodyWire,
};
use holons_boundary::envelopes::{InternalDanceRequestEnvelope, InternalDanceResponseEnvelope};
use holons_boundary::session_state::{SerializableHolonPool, SessionStateWire};
use holons_core::{
    core_shared_objects::transactions::TransactionContext,
    dances::ResponseStatusCode,
    HolonsContextBehavior,
};

/// Adapter entrypoint for the internal dance envelope.
///
/// This function is responsible for ingress binding, runtime dispatch, and egress projection.
#[hdk_extern]
pub fn dance_adapter(
    envelope: InternalDanceRequestEnvelope,
) -> ExternResult<InternalDanceResponseEnvelope> {
    let InternalDanceRequestEnvelope { request, session } = envelope;
    let dance_name = request.dance_name.clone();

    info!(
        "\n\n\n***********************  Entered dance_adapter() with {}",
        request.summarize()
    );

    // ---- ingress validation ----
    if let Err(status_code) = validate_request(&request) {
        let response_wire = DanceResponseWire {
            status_code,
            description: MapString("Invalid Request".to_string()),
            body: ResponseBodyWire::None,
            descriptor: None,
        };

        return Ok(InternalDanceResponseEnvelope { response: response_wire, session });
    }

    // ---- context hydration ----
    let session_state = match session.as_ref() {
        Some(state) => state,
        None => {
            let error = HolonError::InvalidWireFormat {
                wire_type: "InternalDanceRequestEnvelope".to_string(),
                reason: "Missing SessionStateWire".to_string(),
            };
            return Ok(create_error_response_envelope(error, session));
        }
    };

    let context = match initialize_context_from_session_state(session_state) {
        Ok(ctx) => ctx,
        Err(error) => return Ok(create_error_response_envelope(error, session)),
    };

    // ---- bind wire -> runtime ----
    let bound_request = match request.bind(&context) {
        Ok(bound) => bound,
        Err(error) => return Ok(create_error_response_envelope(error, session)),
    };

    // ---- dispatch ----
    let response_runtime = dispatch_dance(&context, bound_request);

    // ---- project runtime -> wire ----
    let response_wire = DanceResponseWire::from(&response_runtime);

    // ---- export session state ----
    let response_session = restore_session_state_from_context(&context);

    info!(
        "\n======== RETURNING FROM {:?} Dance with {:?}",
        dance_name.0, response_wire,
    );

    Ok(InternalDanceResponseEnvelope {
        response: response_wire,
        session: response_session,
    })
}

/// Backward-compatible extern name until host config is fully switched.
#[hdk_extern]
pub fn dance(envelope: InternalDanceRequestEnvelope) -> ExternResult<InternalDanceResponseEnvelope> {
    dance_adapter(envelope)
}

fn create_error_response_envelope(
    error: HolonError,
    session: Option<SessionStateWire>,
) -> InternalDanceResponseEnvelope {
    let error_message = format!("Dance adapter failure: {}", error);

    let response_wire = DanceResponseWire {
        status_code: ResponseStatusCode::from(error),
        description: MapString(error_message),
        body: ResponseBodyWire::None,
        descriptor: None,
    };

    InternalDanceResponseEnvelope { response: response_wire, session }
}

fn initialize_context_from_session_state(
    session_state: &SessionStateWire,
) -> Result<Arc<TransactionContext>, HolonError> {
    info!("==Initializing context from session state==");
    debug!("session_state: {:#?}", session_state);

    let tx_id = session_state.get_tx_id().ok_or(HolonError::InvalidWireFormat {
        wire_type: "SessionStateWire".to_string(),
        reason: "Missing tx_id".to_string(),
    })?;

    let local_space_holon_id = match session_state.get_local_space_holon_wire() {
        Some(reference_wire) => Some(space_holon_id_from_wire_reference(&reference_wire)?),
        None => None,
    };

    let transient_holons = session_state.get_transient_holons().clone();
    let staged_holons = session_state.get_staged_holons().clone();

    let context = init_guest_context(
        transient_holons,
        staged_holons,
        local_space_holon_id.clone(),
        tx_id,
    )?;

    // Ensure the transaction context is anchored to a persisted space holon.
    let ensured_space_holon_id: HolonId = match local_space_holon_id {
        Some(id) => id,
        None => {
            let holon_service = context.get_holon_service();
            let guest_service =
                holon_service.as_any().downcast_ref::<GuestHolonService>().ok_or_else(|| {
                    HolonError::DowncastFailure("GuestHolonService".to_string())
                })?;

            let ensured_space_ref = guest_service.ensure_local_holon_space(&context)?;
            match ensured_space_ref {
                holons_core::HolonReference::Smart(smart_ref) => smart_ref.get_id()?,
                other => {
                    return Err(HolonError::InvalidHolonReference(format!(
                        "ensure_local_holon_space returned non-smart reference: {} ({})",
                        other.reference_kind_string(),
                        other.reference_id_string()
                    )))
                }
            }
        }
    };

    context.set_space_holon_id(ensured_space_holon_id)?;
    Ok(context)
}

/// Restores the session_state wire payload from context.
///
/// NOTE: State restoration is **best-effort**. If exporting staged/transient holons
/// or reading the local space holon fails (e.g., due to lock acquisition errors),
/// this function logs the error and returns `None` instead of panicking.
fn restore_session_state_from_context(
    context: &Arc<TransactionContext>,
) -> Option<SessionStateWire> {
    // ---- export staged holons ----
    let serializable_staged_pool = match context.export_staged_holons() {
        Ok(pool) => SerializableHolonPool::from(&pool),
        Err(error) => {
            warn!(
                "Failed to export staged holons while restoring session state: {:?}",
                error
            );
            return None;
        }
    };

    // ---- export transient holons ----
    let serializable_transient_pool = match context.export_transient_holons() {
        Ok(pool) => SerializableHolonPool::from(&pool),
        Err(error) => {
            warn!(
                "Failed to export transient holons while restoring session state: {:?}",
                error
            );
            return None;
        }
    };

    // ---- resolve local space holon ----
    let local_space_holon = match context.get_space_holon() {
        Ok(space_opt) => space_opt,
        Err(error) => {
            warn!(
                "Failed to read local space holon while restoring session state: {:?}",
                error
            );
            return None;
        }
    };

    Some(SessionStateWire::new(
        serializable_transient_pool,
        serializable_staged_pool,
        local_space_holon,
        Some(context.tx_id()),
    ))
}

fn validate_request(_request: &DanceRequestWire) -> Result<(), ResponseStatusCode> {
    // TODO: Add additional validation checks for dance_name, dance_type, etc.
    Ok(())
}

// TEMPORARY: remove once SessionState stores HolonId directly
/// Extracts the persisted holon id from a wire reference suitable for anchoring the space holon.
///
/// The space holon must always be persisted, so only SmartReferenceWire is accepted.
fn space_holon_id_from_wire_reference(
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
