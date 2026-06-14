use crate::core_shared_objects::Holon;
use crate::core_shared_objects::transactions::TransactionContext;
use crate::descriptors::{accessor_helpers, DanceDescriptor, DanceResponseDescriptor, HolonDescriptor};
use crate::reference_layer::{HolonReference, ReadableHolon, WritableHolon};
use base_types::{BaseValue, MapString};
use core_types::{HolonError, TypeKind};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use type_names::{CorePropertyTypeName, CoreRelationshipTypeName};
use type_names::CoreDanceImplementationName;

/// Runtime result for dance execution within a transaction.
///
/// This contract stays separate from transport-safe wire types because it may
/// contain transaction-bound references that only make sense inside the current
/// runtime session. See `dances-design-spec` for the descriptor-driven dance
/// execution model that this result supports.
pub type DanceExecutionResult = Result<DanceOutcome, HolonError>;

/// Semantic identity for the dance being invoked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DanceIdentity {
    /// The schema-level name of the dance.
    pub dance_name: MapString,
    /// Optional direct reference to the dance descriptor holon.
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

/// The holon, if any, that a dance is being performed on.
#[derive(Debug, Clone)]
pub enum DanceTarget {
    /// The invocation has no subject holon.
    None,
    /// The invocation is scoped to one subject holon.
    One(HolonReference),
}

impl DanceTarget {
    pub fn one(target: HolonReference) -> Self {
        Self::One(target)
    }
}

/// Request payload state for a dance invocation.
///
/// The request is modeled as a holon reference so descriptor-backed validation
/// can inspect it structurally. Request holons are currently expected to be
/// transient because they are invocation-scoped inputs, not persisted domain
/// state.
#[derive(Debug, Clone)]
pub enum DanceRequestState {
    /// The invocation carries no request holon.
    None,
    /// The invocation carries one request holon.
    RequestHolon(HolonReference),
}

impl DanceRequestState {
    pub fn request_holon(request_ref: HolonReference) -> Result<Self, HolonError> {
        if !request_ref.is_transient() {
            return Err(HolonError::InvalidParameter(format!(
                "DanceRequestState::RequestHolon requires a Transient reference in PR2; got {}",
                request_ref.reference_kind_string()
            )));
        }

        Ok(Self::RequestHolon(request_ref))
    }

    pub fn request_ref(&self) -> Option<&HolonReference> {
        match self {
            Self::None => None,
            Self::RequestHolon(reference) => Some(reference),
        }
    }

    pub fn parameter_holon(parameter_ref: HolonReference) -> Result<Self, HolonError> {
        Self::request_holon(parameter_ref)
    }

    pub fn parameter_ref(&self) -> Option<&HolonReference> {
        self.request_ref()
    }
}

pub type DanceParameters = DanceRequestState;

/// Execution context attached to a dance invocation.
#[derive(Debug, Clone)]
pub struct DanceContext {
    /// Records which ingress surface initiated the dance.
    pub invocation_source: InvocationSource,
    /// Optional capability reference associated with the invocation.
    pub capability_ref: Option<HolonReference>,
    /// Optional descriptor reference describing the affording holon type.
    pub affording_type_ref: Option<HolonReference>,
}

impl DanceContext {
    pub fn new(
        invocation_source: InvocationSource,
        capability_ref: Option<HolonReference>,
        affording_type_ref: Option<HolonReference>,
    ) -> Self {
        Self { invocation_source, capability_ref, affording_type_ref }
    }

    pub fn client_command() -> Self {
        Self::new(InvocationSource::ClientCommand, None, None)
    }

    pub fn trust_channel() -> Self {
        Self::new(InvocationSource::TrustChannel, None, None)
    }

    pub fn internal() -> Self {
        Self::new(InvocationSource::Internal, None, None)
    }
}

/// Identifies which runtime surface initiated the invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvocationSource {
    /// The invocation entered through the host command surface.
    ClientCommand,
    /// The invocation entered through a trust channel.
    TrustChannel,
    /// The invocation originated inside the runtime itself.
    Internal,
}

pub type DanceInvocationSource = InvocationSource;

/// Resolved invocation context assembled for execution.
///
/// Binding follows the typed invocation reference, resolves the descriptor
/// relationships needed for execution, and keeps them together so the executor
/// and implementation layer do not need to rediscover invocation structure.
pub struct BoundDanceInvocation {
    invocation: DanceInvocation,
    dance_descriptor: DanceDescriptor,
    request: Option<HolonReference>,
    request_type: Option<HolonDescriptor>,
    affording_holon: Option<HolonReference>,
    affording_holon_descriptor: Option<HolonDescriptor>,
    invocation_source: Option<InvocationSource>,
}

impl BoundDanceInvocation {
    /// Returns the typed invocation reference that was bound.
    pub fn invocation(&self) -> &DanceInvocation {
        &self.invocation
    }

    /// Returns the descriptor of the dance being invoked.
    pub fn dance_descriptor(&self) -> &DanceDescriptor {
        &self.dance_descriptor
    }

    /// Returns the request holon, if one was supplied.
    pub fn request(&self) -> Option<&HolonReference> {
        self.request.as_ref()
    }

    /// Returns the request descriptor declared by the dance, if any.
    pub fn request_type(&self) -> Option<&HolonDescriptor> {
        self.request_type.as_ref()
    }

    /// Returns the response descriptor declared by the dance.
    pub fn response_type(&self) -> Result<DanceResponseDescriptor, HolonError> {
        self.dance_descriptor.response_type()
    }

    /// Returns the holon the dance is being performed on, if any.
    pub fn affording_holon(&self) -> Option<&HolonReference> {
        self.affording_holon.as_ref()
    }

    /// Returns the descriptor of the affording holon, if one is present.
    pub fn affording_holon_descriptor(&self) -> Option<&HolonDescriptor> {
        self.affording_holon_descriptor.as_ref()
    }

    /// Returns the invocation source if it was recorded on the invocation holon.
    pub fn invocation_source(&self) -> Option<InvocationSource> {
        self.invocation_source
    }
}

/// Typed reference to a `DanceInvocation` holon at the execution boundary.
///
/// This wrapper keeps dance-ingress signatures explicit while still using the
/// ordinary reference layer underneath.
#[derive(Debug, Clone, PartialEq)]
pub struct DanceInvocation {
    invocation: HolonReference,
}

impl DanceInvocation {
    /// Constructs a typed invocation reference after verifying the holon is
    /// described as `DanceInvocation`.
    pub fn new(invocation: HolonReference) -> Result<Self, HolonError> {
        let descriptor = invocation.holon_descriptor()?;
        let found = descriptor.header().type_name()?;
        let expected = MapString("DanceInvocation".to_string());

        if found != expected {
            return Err(HolonError::WrongDescriptorKind {
                expected: expected.to_string(),
                found: found.to_string(),
                descriptor: found.to_string(),
            });
        }

        Ok(Self { invocation })
    }

    /// Returns the underlying reference.
    pub fn as_holon_reference(&self) -> &HolonReference {
        &self.invocation
    }

    /// Consumes the wrapper and returns the underlying reference.
    pub fn into_inner(self) -> HolonReference {
        self.invocation
    }

    /// Follows `InvokesDance` and returns the referenced dance descriptor.
    pub fn dance_descriptor(&self) -> Result<DanceDescriptor, HolonError> {
        let descriptor = accessor_helpers::require_single_related(
            self.as_holon_reference(),
            CoreRelationshipTypeName::InvokesDance,
        )?;
        Ok(DanceDescriptor::from_holon(descriptor))
    }

    /// Follows `Request` and returns the request holon, if present.
    pub fn request(&self) -> Result<Option<HolonReference>, HolonError> {
        accessor_helpers::optional_single_related(
            self.as_holon_reference(),
            CoreRelationshipTypeName::Request,
        )
    }

    /// Returns the request holon or reports a missing required relationship.
    pub fn require_request(&self) -> Result<HolonReference, HolonError> {
        self.request()?.ok_or_else(|| HolonError::MissingRequiredRelationship {
            relationship: "Request".to_string(),
            descriptor: self
                .as_holon_reference()
                .summarize()
                .unwrap_or_else(|_| "DanceInvocation".to_string()),
        })
    }

    /// Follows `Target` and returns the holon the dance is being performed on,
    /// if present.
    pub fn affording_holon(&self) -> Result<Option<HolonReference>, HolonError> {
        accessor_helpers::optional_single_related(
            self.as_holon_reference(),
            CoreRelationshipTypeName::Target,
        )
    }

    /// Returns the invocation source recorded on the invocation holon, if any.
    pub fn invocation_source(&self) -> Result<Option<InvocationSource>, HolonError> {
        match self.as_holon_reference().property_value("InvocationSource")? {
            Some(BaseValue::StringValue(value)) => {
                parse_invocation_source(&value).map(Some)
            }
            Some(BaseValue::EnumValue(value)) => {
                parse_invocation_source(&value.0).map(Some)
            }
            Some(other) => Err(HolonError::UnexpectedValueType(
                format!("{other:?}"),
                "Enum".to_string(),
            )),
            None => Ok(None),
        }
    }

    /// Resolves the descriptor-backed execution context needed by the executor.
    pub fn bind(self) -> Result<BoundDanceInvocation, HolonError> {
        let dance_descriptor = self.dance_descriptor()?;
        let request_type = dance_descriptor.request_type()?;
        let affording_holon = self.affording_holon()?;
        let affording_holon_descriptor = match affording_holon.as_ref() {
            Some(holon) => Some(holon.holon_descriptor()?),
            None => None,
        };

        Ok(BoundDanceInvocation {
            request: self.request()?,
            invocation_source: self.invocation_source()?,
            invocation: self,
            dance_descriptor,
            request_type,
            affording_holon,
            affording_holon_descriptor,
        })
    }

    /// Builds a canonical invocation holon for the host-side `DeleteHolon`
    /// dance surface.
    ///
    /// The invocation carries no subject holon. Instead, it carries a typed
    /// request holon that points at the holon to delete through
    /// `ReferenceTarget`, which keeps the invocation structurally valid without
    /// requiring the target's descriptor to advertise delete as an afforded
    /// dance.
    pub fn build_delete_holon(
        context: &Arc<TransactionContext>,
        target: HolonReference,
    ) -> Result<Self, HolonError> {
        let invocation_descriptor = new_runtime_descriptor_holon(
            context,
            "dance-invocation-descriptor",
            "DanceInvocation",
        )?;
        let request_type = new_runtime_descriptor_holon(
            context,
            "delete-holon-request-type",
            "DeleteHolonRequest",
        )?;
        let response_type = new_runtime_descriptor_holon(
            context,
            "delete-holon-response-type",
            "DanceResponseType",
        )?;
        let implementation = new_runtime_descriptor_holon(
            context,
            "delete-holon-implementation",
            CoreDanceImplementationName::DeleteHolon.as_command_name().0,
        )?;

        let mut dance_descriptor =
            context.mutation().new_holon(Some(MapString::from("delete-holon-dance")))?;
        initialize_runtime_descriptor_holon(&mut dance_descriptor, "DeleteHolon")?;
        dance_descriptor.add_related_holons(
            CoreRelationshipTypeName::RequestType,
            vec![request_type.clone().into()],
        )?;
        dance_descriptor.add_related_holons(
            CoreRelationshipTypeName::Response,
            vec![response_type.into()],
        )?;
        dance_descriptor.add_related_holons(
            CoreRelationshipTypeName::ForDance,
            vec![implementation.into()],
        )?;

        let mut request = context
            .mutation()
            .new_holon(Some(MapString::from("delete-holon-request")))?;
        request.with_descriptor(request_type.into())?;
        request.add_related_holons(
            CoreRelationshipTypeName::ReferenceTarget,
            vec![HolonReference::smart_with_key(
                context.context_handle(),
                target.holon_id()?,
                MapString("delete-holon-target".to_string()),
            )],
        )?;

        let mut invocation =
            context.mutation().new_holon(Some(MapString::from("delete-holon-invocation")))?;
        invocation.with_descriptor(invocation_descriptor.into())?;
        invocation.with_property_value(
            "InvocationSource",
            MapString("ClientCommand".to_string()),
        )?;
        invocation.add_related_holons(
            CoreRelationshipTypeName::InvokesDance,
            vec![dance_descriptor.into()],
        )?;
        invocation.add_related_holons(
            CoreRelationshipTypeName::Request,
            vec![request.into()],
        )?;

        Self::new(invocation.into())
    }

    /// Builds a canonical invocation holon for the host-side `Commit`
    /// dance surface.
    ///
    /// Commit has no request holon and no affording holon. The transaction
    /// context itself provides the state being committed.
    pub fn build_commit(
        context: &Arc<TransactionContext>,
    ) -> Result<Self, HolonError> {
        let invocation_descriptor = new_runtime_descriptor_holon(
            context,
            "dance-invocation-descriptor",
            "DanceInvocation",
        )?;
        let response_type = new_runtime_descriptor_holon(
            context,
            "commit-response-type",
            "DanceResponseType",
        )?;
        let implementation = new_runtime_descriptor_holon(
            context,
            "commit-implementation",
            CoreDanceImplementationName::Commit.as_command_name().0,
        )?;

        let mut dance_descriptor =
            context.mutation().new_holon(Some(MapString::from("commit-dance")))?;
        initialize_runtime_descriptor_holon(&mut dance_descriptor, "Commit")?;
        dance_descriptor.add_related_holons(
            CoreRelationshipTypeName::Response,
            vec![response_type.into()],
        )?;
        dance_descriptor.add_related_holons(
            CoreRelationshipTypeName::ForDance,
            vec![implementation.into()],
        )?;

        let mut invocation = context.mutation().new_holon(Some(MapString::from("commit-invocation")))?;
        invocation.with_descriptor(invocation_descriptor.into())?;
        invocation.with_property_value(
            "InvocationSource",
            MapString("ClientCommand".to_string()),
        )?;
        invocation.add_related_holons(
            CoreRelationshipTypeName::InvokesDance,
            vec![dance_descriptor.into()],
        )?;

        Self::new(invocation.into())
    }
}

fn new_runtime_descriptor_holon(
    context: &Arc<TransactionContext>,
    key: &str,
    type_name: impl Into<MapString>,
) -> Result<HolonReference, HolonError> {
    let mut descriptor = context.mutation().new_holon(Some(MapString::from(key)))?;
    initialize_runtime_descriptor_holon(&mut descriptor, type_name)?;
    Ok(descriptor.into())
}

fn initialize_runtime_descriptor_holon<T: WritableHolon>(
    descriptor: &mut T,
    type_name: impl Into<MapString>,
) -> Result<(), HolonError> {
    descriptor.with_property_value(CorePropertyTypeName::TypeName, type_name.into())?;
    descriptor.with_property_value(CorePropertyTypeName::IsAbstractType, false)?;
    descriptor.with_property_value(
        CorePropertyTypeName::InstanceTypeKind,
        MapString(TypeKind::Holon.as_schema_key()),
    )?;
    Ok(())
}

fn parse_invocation_source(value: &MapString) -> Result<InvocationSource, HolonError> {
    match value.0.as_str() {
        "ClientCommand" => Ok(InvocationSource::ClientCommand),
        "TrustChannel" => Ok(InvocationSource::TrustChannel),
        "Internal" => Ok(InvocationSource::Internal),
        other => Err(HolonError::InvalidParameter(format!(
            "Unsupported InvocationSource value: {other}"
        ))),
    }
}

impl From<DanceInvocation> for HolonReference {
    fn from(invocation: DanceInvocation) -> Self {
        invocation.into_inner()
    }
}

impl From<&DanceInvocation> for HolonReference {
    fn from(invocation: &DanceInvocation) -> Self {
        invocation.as_holon_reference().clone()
    }
}

pub fn build_dance_v2_invocation(
    invocation: HolonReference,
) -> Result<DanceInvocation, HolonError> {
    DanceInvocation::new(invocation)
}

/// Typed reference to a response holon described by `DanceResponseType`.
#[derive(Debug, Clone, PartialEq)]
pub struct DanceResponseReference {
    response: HolonReference,
}

impl DanceResponseReference {
    /// Constructs a typed response reference after verifying the response
    /// holon is described as `DanceResponseType`.
    pub fn new(response: HolonReference) -> Result<Self, HolonError> {
        let descriptor = response.holon_descriptor()?;
        let found = descriptor.header().type_name()?;
        let expected = MapString("DanceResponseType".to_string());

        if found != expected {
            return Err(HolonError::WrongDescriptorKind {
                expected: expected.to_string(),
                found: found.to_string(),
                descriptor: found.to_string(),
            });
        }

        Ok(Self { response })
    }

    /// Returns the underlying reference.
    pub fn as_holon_reference(&self) -> &HolonReference {
        &self.response
    }

    /// Consumes the wrapper and returns the underlying reference.
    pub fn into_inner(self) -> HolonReference {
        self.response
    }

    /// Returns the related response-body holon, if one is present.
    pub fn response_body(&self) -> Result<Option<HolonReference>, HolonError> {
        accessor_helpers::optional_single_related(
            self.as_holon_reference(),
            CoreRelationshipTypeName::ResponseBody,
        )
    }

    /// Returns the related response-body holon or reports a missing body.
    pub fn require_response_body(&self) -> Result<HolonReference, HolonError> {
        self.response_body()?.ok_or_else(|| HolonError::MissingRequiredRelationship {
            relationship: "ResponseBody".to_string(),
            descriptor: self
                .as_holon_reference()
                .summarize()
                .unwrap_or_else(|_| "DanceResponseType".to_string()),
        })
    }
}

impl From<DanceResponseReference> for HolonReference {
    fn from(response: DanceResponseReference) -> Self {
        response.into_inner()
    }
}

impl From<&DanceResponseReference> for HolonReference {
    fn from(response: &DanceResponseReference) -> Self {
        response.as_holon_reference().clone()
    }
}

pub fn build_dance_v2_response(
    response: HolonReference,
) -> Result<DanceResponseReference, HolonError> {
    DanceResponseReference::new(response)
}

/// Successful dance outcome with optional diagnostics and events.
#[derive(Debug, Clone)]
pub struct DanceOutcome {
    /// The primary result of execution.
    pub result: DanceResult,
    /// Non-fatal diagnostics emitted during execution.
    pub diagnostics: Vec<DanceDiagnostic>,
    /// Execution-side events associated with the outcome.
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

/// Primary result payload returned by a successful dance.
#[derive(Debug, Clone)]
pub enum DanceResult {
    /// The dance completed without a result payload.
    None,
    /// The dance returned a fully materialized holon.
    Holon(Holon),
    /// The dance returned a holon by reference.
    HolonReference(HolonReference),
}

/// Non-fatal diagnostic emitted during dance execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DanceDiagnostic {
    /// The severity of the diagnostic.
    pub severity: DanceDiagnosticSeverity,
    /// A stable diagnostic code.
    pub code: String,
    /// Human-readable diagnostic text.
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

/// Severity level for a non-fatal diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DanceDiagnosticSeverity {
    /// Informational note.
    Info,
    /// Warning that execution succeeded but surfaced a concern.
    Warning,
}

/// Event emitted alongside a successful dance outcome.
#[derive(Debug, Clone)]
pub struct DanceEvent {
    /// Event name for downstream consumers.
    pub event_name: String,
    /// Optional holon payload associated with the event.
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
        DanceOutcome, DanceRequestState, DanceResult, InvocationSource,
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

        let parameters = DanceRequestState::request_holon(HolonReference::from(transient))
            .expect("valid params");

        assert!(matches!(
            parameters,
            DanceRequestState::RequestHolon(reference) if reference.is_transient()
        ));
    }

    #[test]
    fn parameter_holon_rejects_non_transient_reference() {
        let context = build_context();
        let transient = new_test_holon(&context, "params").expect("create transient test holon");
        let staged =
            context.mutation().stage_new_holon(transient).expect("stage transient test holon");

        let error = DanceRequestState::request_holon(HolonReference::from(staged))
            .expect_err("staged ref should be rejected");

        assert!(
            matches!(error, HolonError::InvalidParameter(message) if message.contains("Transient reference"))
        );
    }

    #[test]
    fn context_helpers_select_expected_invocation_source() {
        assert_eq!(
            DanceContext::client_command().invocation_source,
            InvocationSource::ClientCommand
        );
        assert_eq!(DanceContext::trust_channel().invocation_source, InvocationSource::TrustChannel);
        assert_eq!(DanceContext::internal().invocation_source, InvocationSource::Internal);
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
            to_value(InvocationSource::TrustChannel).expect("serialize source"),
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
