// shared_crates/holons_loader/src/errors.rs

use crate::controller::{FileProvenance, ProvenanceIndex};
use holons_prelude::prelude::CorePropertyTypeName::{ErrorMessage, ErrorType};
use holons_prelude::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};

// Global counter for generating unique error holon keys
static ERROR_SEQ: AtomicU32 = AtomicU32::new(1);

/// A load error with optional context that ties it to a specific LoaderHolon.
#[derive(Debug, Clone)]
pub struct ErrorWithContext {
    /// The underlying HolonError.
    pub error: HolonError,
    /// Optional HolonLoader key related to the error.
    pub source_loader_key: Option<MapString>,
}

impl ErrorWithContext {
    pub fn new(error: HolonError) -> Self {
        Self { error, source_loader_key: None }
    }
    pub fn with_loader_key(mut self, key: MapString) -> Self {
        self.source_loader_key = Some(key);
        self
    }
}

/// Map HolonError -> stable snake_case code for response analytics/UI.
pub fn error_type_code(err: &HolonError) -> &'static str {
    use HolonError::*;
    match err {
        // Likely along the loader path:
        DuplicateError(_, _) => "duplicate",
        EmptyField(_) => "empty_field",
        InvalidRelationship(_, _) => "invalid_relationship",
        InvalidParameter(_) => "invalid_parameter",
        ValidationError(_) => "validation_error",
        CommitFailure(_) => "commit_failure",
        UnexpectedValueType(_, _) => "unexpected_value_type",
        InvalidType(_) => "invalid_type",
        HolonNotFound(_) => "holon_not_found",
        InvalidHolonReference(_) => "invalid_holon_reference",
        NotAccessible(_, _) => "not_accessible",

        // Group the rest under a generic bucket to avoid leaking internals
        _ => "server_error",
    }
}

/// Build transient HolonErrorType holons for reporting load errors.
/// - One holon per error.
/// - If `provenance` + `source_loader_key` are available, stamp:
///     LoaderHolonKey, Filename, StartUtf8ByteOffset.
pub fn make_error_holons_best_effort(
    context: &TransactionContext,
    errors: &[ErrorWithContext],
    provenance: Option<&ProvenanceIndex>,
) -> Result<Vec<TransientReference>, HolonError> {
    if errors.is_empty() {
        return Ok(Vec::new());
    }

    let mut output = Vec::with_capacity(errors.len());
    // Try resolving the HolonErrorType descriptor once.
    let holon_error_type_descriptor = resolve_holon_error_type_descriptor(context).ok();

    for contextual_error in errors {
        let mut transient_reference = make_error_holon(
            context,
            holon_error_type_descriptor.clone(),
            &contextual_error.error,
        )?;

        if let (Some(index), Some(loader_key)) =
            (provenance, contextual_error.source_loader_key.clone())
        {
            // Add LoaderHolonKey
            transient_reference.with_property_value(
                CorePropertyTypeName::LoaderHolonKey,
                BaseValue::StringValue(loader_key.clone()),
            )?;

            // Attempt to enrich with provenance details
            if let Some(FileProvenance { filename, start_utf8_byte_offset }) =
                index.get(&loader_key)
            {
                transient_reference.with_property_value(
                    CorePropertyTypeName::Filename,
                    BaseValue::StringValue(filename.clone()),
                )?;
                if let Some(offset) = start_utf8_byte_offset {
                    transient_reference.with_property_value(
                        CorePropertyTypeName::StartUtf8ByteOffset,
                        BaseValue::IntegerValue(MapInteger(*offset)),
                    )?;
                }
            }
        }
        output.push(transient_reference);
    }

    Ok(output)
}

/// Builds a transient **HolonError** holon representing the specified `HolonError`.
///
/// This function creates a new transient holon and populates it with the
/// standard error fields (e.g., `error_type`, `error_message`).
/// If a `descriptor` is provided, it is attached via `with_descriptor()` to identify
/// the holon's type (typically `HolonErrorType`). If `descriptor` is `None`, the holon
/// is left untyped but still contains all relevant error details.
///
/// # Arguments
/// - `context`: The active holon execution context used to access transient behavior services.
/// - `descriptor`: An optional reference to the `HolonErrorType` descriptor holon.
/// - `err`: The `HolonError` instance to encode into the transient holon.
///
/// # Returns
/// - `Ok(TransientReference)` — reference to the newly created transient error holon.
/// - `Err(HolonError)` — if the transient holon could not be created or populated.
///
/// # Behavior
/// - Always calls `create_empty_error_holon()` to allocate a new transient holon.
/// - Applies `with_descriptor()` only if a descriptor is provided.
/// - Uses `populate_error_fields()` to fill in diagnostic fields.
///
/// Use this helper to create both typed and untyped error holons from a single entry point.
pub fn make_error_holon(
    context: &TransactionContext,
    descriptor: Option<HolonReference>,
    err: &HolonError,
) -> Result<TransientReference, HolonError> {
    let mut transient_reference = create_empty_error_holon(context)?;
    if let Some(desc) = descriptor {
        transient_reference.with_descriptor(desc)?;
    }
    populate_error_fields(&mut transient_reference, err)?;
    Ok(transient_reference)
}
// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

fn create_empty_error_holon(
    context: &TransactionContext,
) -> Result<TransientReference, HolonError> {
    // Generate a unique, local-only key (fast and deterministic within a process).
    let id = ERROR_SEQ.fetch_add(1, Ordering::Relaxed);
    let key = MapString(format!("loader-error-{id}"));

    // Obtain a handle to the TransientHolonBehavior service from the Space Manager.
    let transient_behavior = context.get_transient_behavior_service();

    // Create a new, empty transient holon using the generated key.
    let transient_reference = transient_behavior.create_empty(key)?;

    Ok(transient_reference)
}

fn populate_error_fields(
    error_ref: &mut TransientReference,
    err: &HolonError,
) -> Result<(), HolonError> {
    let error_type: &str = error_type_code(err);

    error_ref.with_property_value(
        ErrorType.as_property_name(),
        BaseValue::StringValue(MapString(error_type.to_string())),
    )?;

    error_ref.with_property_value(
        ErrorMessage.as_property_name(),
        BaseValue::StringValue(MapString(err.to_string())),
    )?;

    Ok(())
}

/// Descriptor resolution (best-effort):
/// 1) Staged (Nursery) lookup by key "HolonLoadError.HolonErrorType"
/// 2) Saved fallback via get_all_holons() + get_by_key()
fn resolve_holon_error_type_descriptor(
    context: &TransactionContext,
) -> Result<HolonReference, HolonError> {
    // Canonical key from the enum (=> "HolonLoadError")
    let type_name = CoreHolonTypeName::HolonLoadError.as_holon_name();
    let key = MapString(format!("{type_name}.HolonErrorType"));

    // 1) Prefer staged (Nursery) by base key
    let staged_matches = {
        let staging_behavior = context.get_staging_service();

        // Query staged holons by base key.
        staging_behavior.get_staged_holons_by_base_key(&key)?
    };

    match staged_matches.len() {
        1 => {
            let staged_ref = staged_matches.into_iter().next().unwrap();
            return Ok(HolonReference::Staged(staged_ref));
        }
        n if n > 1 => {
            return Err(HolonError::DuplicateError(
                "HolonErrorType descriptor (staged)".into(),
                n.to_string(),
            ));
        }
        _ => { /* fall through to saved fallback */ }
    }

    // 2) Saved fallback: single pass over the saved index by key
    let saved_collection = get_all_holons(context)?;
    match saved_collection.get_by_key(&key) {
        Ok(Some(reference)) => Ok(reference),
        Ok(None) => Err(HolonError::HolonNotFound(format!(
            "HolonErrorType descriptor not found by key '{}'",
            key.0
        ))),
        Err(e) => Err(e),
    }
}
