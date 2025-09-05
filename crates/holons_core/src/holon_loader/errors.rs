use base_types::{BaseValue, MapInteger, MapString};
use core_types::HolonError;

use crate::core_shared_objects::holon::TransientHolon;

use super::names as N;

/// Public, stable error codes that appear in a HolonLoadError holon’s `error_code`.
/// Keep codes short, precise, and machine-friendly. Human text goes in `error_message`.
#[derive(Debug, Clone)]
pub enum LoaderErrorCode {
    MissingRequiredProperty,
    UnresolvedReference,
    InvalidValueType,
    InvalidRelationship,
    CommitFailure,
    NotImplemented,
    Misc,
}

impl From<LoaderErrorCode> for MapString {
    fn from(code: LoaderErrorCode) -> Self {
        use LoaderErrorCode::*;
        MapString(match code {
            MissingRequiredProperty => "MissingRequiredProperty",
            UnresolvedReference     => "UnresolvedReference",
            InvalidValueType        => "InvalidValueType",
            InvalidRelationship     => "InvalidRelationship",
            CommitFailure           => "CommitFailure",
            NotImplemented          => "NotImplemented",
            Misc                    => "Misc",
        }.to_string())
    }
}

/// Data needed to construct a single HolonLoadError holon.
#[derive(Debug, Clone)]
pub struct LoadErrorFields {
    /// If the client tracks a JSON line number; otherwise None.
    pub line: Option<i64>,
    /// Machine-readable category exposed in `error_code`.
    pub code: LoaderErrorCode,
    /// Human-readable details exposed in `error_message`.
    pub message: String,
}

/// Build a HolonLoadError holon as a `TransientHolon`.
/// We set only properties here. The caller should attach it to the response
/// via the declared relationship `HAS_LOAD_ERROR` (after staging the response),
/// and (optionally) set `DescribedBy` to #HolonLoadError.Type in Pass 2.
pub fn build_load_error_holon(fields: LoadErrorFields) -> TransientHolon {
    let mut err = TransientHolon::new();

    // error_code (required)
    let _ = err.with_property_value(
        N::prop(N::PROP_ERROR_CODE),
        Some(BaseValue::StringValue(MapString::from(fields.code))),
    );

    // error_message (required)
    let _ = err.with_property_value(
        N::prop(N::PROP_ERROR_MESSAGE),
        Some(BaseValue::StringValue(MapString(fields.message))),
    );

    // error_line (optional)
    if let Some(line) = fields.line {
        let _ = err.with_property_value(
            N::prop(N::PROP_ERROR_LINE),
            Some(BaseValue::IntegerValue(MapInteger(line))),
        );
    }

    err
}

/// Best-effort mapping from internal `HolonError` → public `LoadErrorFields`.
/// You can refine this (and add line numbers) as the validator semantics evolve.
pub fn map_holon_error(e: HolonError) -> LoadErrorFields {
    use LoaderErrorCode::*;

    let (code, message) = match &e {
        HolonError::InvalidParameter(m)      => (MissingRequiredProperty, m.clone()),
        HolonError::InvalidType(m)           => (InvalidValueType, m.clone()),
        HolonError::InvalidRelationship(_,_) => (InvalidRelationship, e.to_string()),
        HolonError::ValidationError(v)       => (InvalidValueType, v.to_string()),
        HolonError::CommitFailure(m)         => (CommitFailure, m.clone()),
        HolonError::NotImplemented(m)        => (NotImplemented, m.clone()),
        _                                    => (Misc, e.to_string()),
    };

    LoadErrorFields { line: None, code, message }
}
