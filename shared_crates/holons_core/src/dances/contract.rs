use crate::core_shared_objects::Holon;
use crate::reference_layer::HolonReference;
use base_types::MapString;
use core_types::HolonError;
use serde::{Deserialize, Serialize};

/// Canonical runtime result for dance execution in PRO1.
///
/// This is a tx-bound runtime contract. It is intentionally kept separate from
/// any future IPC-safe wire form because it may contain tx-bound references.
pub type DanceExecutionResult = Result<DanceOutcome, HolonError>;

/// Canonical runtime invocation envelope for dances in PRO1.
///
/// This type must not be serialized across IPC boundaries because it may
/// contain tx-bound references.
#[derive(Debug, Clone)]
pub struct DanceInvocation {
    pub identity: DanceIdentity,
    pub target: DanceTarget,
    pub parameters: DanceParameters,
    pub context: DanceContext,
}

impl DanceInvocation {
    pub fn new(
        identity: DanceIdentity,
        target: DanceTarget,
        parameters: DanceParameters,
        context: DanceContext,
    ) -> Self {
        Self { identity, target, parameters, context }
    }
}

/// Canonical semantic identity for a dance invocation in PRO1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DanceIdentity {
    pub dance_name: MapString,
    pub dance_descriptor_ref: Option<HolonReference>,
}

impl DanceIdentity {
    pub fn new(dance_name: MapString) -> Self {
        Self { dance_name, dance_descriptor_ref: None }
    }

    pub fn with_descriptor(dance_name: MapString, dance_descriptor_ref: HolonReference) -> Self {
        Self { dance_name, dance_descriptor_ref: Some(dance_descriptor_ref) }
    }
}

/// Invocation-time target selection for a dance in PRO1.
#[derive(Debug, Clone)]
pub enum DanceTarget {
    None,
    One(HolonReference),
}

impl DanceTarget {
    pub fn one(target: HolonReference) -> Self {
        Self::One(target)
    }
}

/// Structured dance parameters for PRO1.
///
/// The canonical PRO1 posture is a reference to a transient parameter holon.
#[derive(Debug, Clone)]
pub enum DanceParameters {
    None,
    ParameterHolon(HolonReference),
}

impl DanceParameters {
    pub fn parameter_holon(parameter_ref: HolonReference) -> Result<Self, HolonError> {
        if !parameter_ref.is_transient() {
            return Err(HolonError::InvalidParameter(format!(
                "DanceParameters::ParameterHolon requires a Transient reference in PRO1; got {}",
                parameter_ref.reference_kind_string()
            )));
        }

        Ok(Self::ParameterHolon(parameter_ref))
    }

    pub fn parameter_ref(&self) -> Option<&HolonReference> {
        match self {
            Self::None => None,
            Self::ParameterHolon(reference) => Some(reference),
        }
    }
}

/// Invocation-time execution metadata for a dance.
#[derive(Debug, Clone)]
pub struct DanceContext {
    pub invocation_source: DanceInvocationSource,
    pub capability_ref: Option<HolonReference>,
    pub affording_type_ref: Option<HolonReference>,
}

impl DanceContext {
    pub fn new(
        invocation_source: DanceInvocationSource,
        capability_ref: Option<HolonReference>,
        affording_type_ref: Option<HolonReference>,
    ) -> Self {
        Self { invocation_source, capability_ref, affording_type_ref }
    }

    pub fn client_command() -> Self {
        Self::new(DanceInvocationSource::ClientCommand, None, None)
    }

    pub fn trust_channel() -> Self {
        Self::new(DanceInvocationSource::TrustChannel, None, None)
    }

    pub fn internal() -> Self {
        Self::new(DanceInvocationSource::Internal, None, None)
    }
}

/// Distinguishes the ingress posture of a dance invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DanceInvocationSource {
    ClientCommand,
    TrustChannel,
    Internal,
}

/// Canonical successful outcome envelope for a dance in PRO1.
#[derive(Debug, Clone)]
pub struct DanceOutcome {
    pub result: DanceResult,
    pub diagnostics: Vec<DanceDiagnostic>,
    pub events: Vec<DanceEvent>,
}

impl DanceOutcome {
    pub fn new(
        result: DanceResult,
        diagnostics: Vec<DanceDiagnostic>,
        events: Vec<DanceEvent>,
    ) -> Self {
        Self { result, diagnostics, events }
    }

    pub fn result_only(result: DanceResult) -> Self {
        Self::new(result, vec![], vec![])
    }
}

/// Canonical structured success result family for PRO1.
#[derive(Debug, Clone)]
pub enum DanceResult {
    None,
    Holon(Holon),
    HolonReference(HolonReference),
}

/// Non-fatal execution diagnostics returned with a successful outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DanceDiagnostic {
    pub severity: DanceDiagnosticSeverity,
    pub code: String,
    pub message: String,
}

impl DanceDiagnostic {
    pub fn new(
        severity: DanceDiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self { severity, code: code.into(), message: message.into() }
    }

    pub fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(DanceDiagnosticSeverity::Info, code, message)
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(DanceDiagnosticSeverity::Warning, code, message)
    }
}

/// Severity of a non-fatal dance diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DanceDiagnosticSeverity {
    Info,
    Warning,
}

/// Execution-side event returned with a successful outcome.
#[derive(Debug, Clone)]
pub struct DanceEvent {
    pub event_name: String,
    pub payload: Option<HolonReference>,
}

impl DanceEvent {
    pub fn new(event_name: impl Into<String>, payload: Option<HolonReference>) -> Self {
        Self { event_name: event_name.into(), payload }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DanceContext, DanceDiagnostic, DanceDiagnosticSeverity, DanceEvent, DanceIdentity,
        DanceInvocationSource, DanceOutcome, DanceParameters, DanceResult,
    };
    use crate::descriptors::test_support::{build_context, new_test_holon};
    use crate::HolonReference;
    use base_types::MapString;
    use core_types::HolonError;
    use serde_json::{json, to_value};

    #[test]
    fn parameter_holon_accepts_transient_reference() {
        let context = build_context();
        let transient = new_test_holon(&context, "params").expect("create transient test holon");

        let parameters = DanceParameters::parameter_holon(HolonReference::from(transient))
            .expect("valid params");

        assert!(matches!(
            parameters,
            DanceParameters::ParameterHolon(reference) if reference.is_transient()
        ));
    }

    #[test]
    fn parameter_holon_rejects_non_transient_reference() {
        let context = build_context();
        let transient = new_test_holon(&context, "params").expect("create transient test holon");
        let staged =
            context.mutation().stage_new_holon(transient).expect("stage transient test holon");

        let error = DanceParameters::parameter_holon(HolonReference::from(staged))
            .expect_err("staged ref should be rejected");

        assert!(
            matches!(error, HolonError::InvalidParameter(message) if message.contains("Transient reference"))
        );
    }

    #[test]
    fn context_helpers_select_expected_invocation_source() {
        assert_eq!(
            DanceContext::client_command().invocation_source,
            DanceInvocationSource::ClientCommand
        );
        assert_eq!(
            DanceContext::trust_channel().invocation_source,
            DanceInvocationSource::TrustChannel
        );
        assert_eq!(DanceContext::internal().invocation_source, DanceInvocationSource::Internal);
    }

    #[test]
    fn outcome_result_only_starts_without_metadata() {
        let outcome = DanceOutcome::result_only(DanceResult::None);

        assert!(matches!(outcome.result, DanceResult::None));
        assert!(outcome.diagnostics.is_empty());
        assert!(outcome.events.is_empty());
    }

    #[test]
    fn diagnostic_helpers_capture_severity_and_text() {
        let info = DanceDiagnostic::info("legacy_bridge", "shape stabilized");
        let warning = DanceDiagnostic::warning("partial", "follow-up needed");

        assert_eq!(info.severity, DanceDiagnosticSeverity::Info);
        assert_eq!(info.code, "legacy_bridge");
        assert_eq!(warning.severity, DanceDiagnosticSeverity::Warning);
        assert_eq!(warning.message, "follow-up needed");
    }

    #[test]
    fn serializable_contract_subtypes_round_trip_to_json() {
        let diagnostic = DanceDiagnostic::warning("partial", "follow-up needed");

        assert_eq!(
            to_value(&diagnostic).expect("serialize diagnostic"),
            json!({
                "severity": "Warning",
                "code": "partial",
                "message": "follow-up needed"
            })
        );

        assert_eq!(
            to_value(DanceInvocationSource::TrustChannel).expect("serialize source"),
            json!("TrustChannel")
        );
    }

    #[test]
    fn identity_can_carry_optional_descriptor_metadata() {
        let context = build_context();
        let descriptor =
            new_test_holon(&context, "dance_descriptor").expect("create descriptor metadata holon");
        let identity = DanceIdentity::with_descriptor(
            MapString("query_relationships".to_string()),
            descriptor.into(),
        );

        assert_eq!(identity.dance_name.0, "query_relationships");
        assert!(identity.dance_descriptor_ref.is_some());
    }

    #[test]
    fn event_payload_can_point_at_holon_reference() {
        let context = build_context();
        let event_payload =
            new_test_holon(&context, "event_payload").expect("create event payload holon");
        let event = DanceEvent::new("validated", Some(event_payload.into()));

        assert_eq!(event.event_name, "validated");
        assert!(matches!(event.payload, Some(reference) if reference.is_transient()));
    }
}
