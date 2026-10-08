// shared_crates/holons_loader/src/errors.rs

use crate::controller::{FileProvenance, ProvenanceIndex};
use holons_prelude::prelude::CorePropertyTypeName::{ErrorMessage, ErrorType};
use holons_prelude::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

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

/// Build transient HolonLoadError holons for reporting load errors.
/// - One holon per error.
/// - If `provenance` + `source_loader_key` are available, stamp:
///   LoaderHolonKey, Filename, StartUtf8ByteOffset.
pub fn make_load_error_holons(
    context: &Arc<TransactionContext>,
    descriptor: Option<HolonReference>,
    errors: &[ErrorWithContext],
    provenance: Option<&ProvenanceIndex>,
) -> Result<Vec<TransientReference>, HolonError> {
    if errors.is_empty() {
        return Ok(Vec::new());
    }

    let mut output = Vec::with_capacity(errors.len());
    for contextual_error in errors {
        let mut transient_reference =
            make_error_holon(context, descriptor.clone(), &contextual_error.error)?;

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
/// If a `descriptor` is provided, it is attached by authoring `DescribedBy` directly
/// to identify the holon's type (typically `HolonErrorType`). No defaults are
/// attempted: diagnostics must remain available even when the descriptor belongs
/// to the malformed graph being reported. If `descriptor` is `None`, the holon
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
/// - Authors `DescribedBy` without default population if a descriptor is provided.
/// - Uses `populate_error_fields()` to fill in diagnostic fields.
///
/// Use this helper to create both typed and untyped error holons from a single entry point.
pub fn make_error_holon(
    context: &Arc<TransactionContext>,
    descriptor: Option<HolonReference>,
    err: &HolonError,
) -> Result<TransientReference, HolonError> {
    let mut transient_reference = create_empty_error_holon(context)?;
    if let Some(desc) = descriptor {
        transient_reference
            .add_related_holons(CoreRelationshipTypeName::DescribedBy, vec![desc])?;
    }
    populate_error_fields(&mut transient_reference, err)?;
    Ok(transient_reference)
}
// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

fn create_empty_error_holon(
    context: &Arc<TransactionContext>,
) -> Result<TransientReference, HolonError> {
    // Generate a unique, local-only key (fast and deterministic within a process).
    let id = ERROR_SEQ.fetch_add(1, Ordering::Relaxed);
    let key = MapString(format!("loader-error-{id}"));

    // Create a new, empty transient holon using the generated key.
    let transient_reference = context.mutation().new_holon(Some(key))?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_reporting_preserves_diagnostics_with_a_malformed_staged_descriptor(
    ) -> Result<(), HolonError> {
        let context = crate::controller::tests::context();
        let transient =
            context.mutation().new_holon(Some(MapString("HolonLoadError.HolonError".into())))?;
        let mut descriptor = context.mutation().stage_new_holon(transient)?;
        let malformed_property =
            context.mutation().new_holon(Some(MapString("Malformed.PropertyType".into())))?;
        let malformed_property = context.mutation().stage_new_holon(malformed_property)?;
        descriptor.add_related_holons(
            CoreRelationshipTypeName::InstanceProperties,
            vec![malformed_property.into()],
        )?;

        // The missing property TypeName makes descriptor-assisted initialization fail.
        let mut probe = context.mutation().new_holon(Some(MapString("default-probe".into())))?;
        let failure = probe.with_descriptor(descriptor.clone().into()).unwrap_err();
        assert!(!matches!(failure, HolonError::MissingDescribedBy { .. }));

        let original_error = HolonError::InvalidParameter("malformed import".into());
        let loader_key = MapString("source-loader".into());
        let mut provenance = ProvenanceIndex::new();
        provenance.insert(
            loader_key.clone(),
            FileProvenance {
                filename: MapString("broken.json".into()),
                start_utf8_byte_offset: Some(42),
            },
        );
        let errors = make_load_error_holons(
            &context,
            Some(descriptor.clone().into()),
            &[ErrorWithContext::new(original_error.clone()).with_loader_key(loader_key.clone())],
            Some(&provenance),
        )?;
        assert_eq!(errors.len(), 1);
        let error_holon = &errors[0];
        assert_eq!(
            HolonReference::from(error_holon.clone()).get_descriptor()?,
            Some(descriptor.into())
        );
        assert_eq!(
            error_holon.property_value(ErrorType)?,
            Some(BaseValue::StringValue(MapString("invalid_parameter".into())))
        );
        assert_eq!(
            error_holon.property_value(ErrorMessage)?,
            Some(BaseValue::StringValue(MapString(original_error.to_string())))
        );
        assert_eq!(
            error_holon.property_value(CorePropertyTypeName::LoaderHolonKey)?,
            Some(BaseValue::StringValue(loader_key))
        );
        assert_eq!(
            error_holon.property_value(CorePropertyTypeName::Filename)?,
            Some(BaseValue::StringValue(MapString("broken.json".into())))
        );
        assert_eq!(
            error_holon.property_value(CorePropertyTypeName::StartUtf8ByteOffset)?,
            Some(BaseValue::IntegerValue(MapInteger(42)))
        );
        Ok(())
    }
}
