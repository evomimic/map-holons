use std::sync::Arc;

use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::DanceInvocation;
use holons_core::{resolve_core_descriptor, HolonReference, ReadableHolon, WritableHolon};
use serde_json::Value;

/// Completes transient submission evidence inside the admitted LoadHolons operation.
/// Client mutation remains closed after submission, including rejected load outcomes.
pub(crate) struct LoadRequestCompletion {
    request: HolonReference,
    diagnostic_type: HolonReference,
    evidence_type: HolonReference,
}

impl LoadRequestCompletion {
    pub(crate) fn from_invocation(
        context: &Arc<TransactionContext>,
        invocation: &DanceInvocation,
    ) -> Result<Option<Self>, HolonError> {
        let collection = invocation.as_holon_reference().related_holons("LoadRequest")?;
        let members = collection
            .read()
            .map_err(|_| HolonError::InvalidState("LoadRequest collection lock poisoned".into()))?
            .get_members()
            .clone();
        let request = match members.as_slice() {
            [] => return Ok(None),
            [HolonReference::Transient(reference)] if reference.tx_id() == context.tx_id() => {
                members[0].clone()
            }
            _ => {
                return Err(HolonError::InvalidParameter(
                    "LoadRequest must be a single transient in the loader transaction".into(),
                ))
            }
        };
        if request.holon_descriptor()?.header().type_name()?.to_string() != "LoadRequest" {
            return Err(HolonError::InvalidParameter(
                "Retained submission must be described as LoadRequest".into(),
            ));
        }
        // Resolve saved descriptors before execution can close the transaction's saved namespace.
        Ok(Some(Self {
            request,
            diagnostic_type: resolve_core_descriptor(context, "LoadDiagnostic.Projection")?,
            evidence_type: resolve_core_descriptor(context, "DiagnosticEvidenceValue.Projection")?,
        }))
    }

    pub(crate) fn responded(&mut self, mut response: HolonReference) -> Result<(), HolonError> {
        if !matches!((&response, &self.request), (HolonReference::Transient(response), HolonReference::Transient(request)) if response.tx_id() == request.tx_id())
        {
            return Err(HolonError::InvalidParameter(
                "LoadHolons response must be transient".into(),
            ));
        }
        self.request.add_related_holons("LoadResponse", vec![response.clone()])?;
        response.add_related_holons("LoadRequest", vec![self.request.clone()])?;
        self.request.with_property_value("LoadRequestStatus", "Responded")?;
        Ok(())
    }

    pub(crate) fn failed(
        &mut self,
        context: &Arc<TransactionContext>,
        error: &HolonError,
    ) -> Result<(), HolonError> {
        self.request.with_property_value("LoadRequestStatus", "InvocationFailed")?;
        self.request.with_property_value("Message", error.to_string())?;
        let mut diagnostic: HolonReference = context
            .mutation()
            .new_holon(Some(
                format!("load-request-evidence-{}", context.lookup().transient_count()?).into(),
            ))?
            .into();
        diagnostic.with_descriptor(self.diagnostic_type.clone())?;
        diagnostic.with_property_value("DiagnosticCategory", "Invocation failure")?;
        diagnostic.with_property_value("Message", error.to_string())?;
        let value = serde_json::to_value(error)
            .map_err(|error| HolonError::InvalidState(error.to_string()))?;
        let evidence = evidence_value(context, &self.evidence_type, &value, None)?;
        diagnostic.add_related_holons("SourceError", vec![evidence])?;
        self.request.add_related_holons("Diagnostics", vec![diagnostic])?;
        Ok(())
    }
}

/// Preserve structured runtime errors as typed, navigable transient evidence.
fn evidence_value(
    context: &Arc<TransactionContext>,
    descriptor: &HolonReference,
    value: &Value,
    name: Option<&str>,
) -> Result<HolonReference, HolonError> {
    let mut node: HolonReference = context
        .mutation()
        .new_holon(Some(
            format!("load-request-evidence-{}", context.lookup().transient_count()?).into(),
        ))?
        .into();
    node.with_descriptor(descriptor.clone())?;
    let kind = match value {
        Value::Null => "null",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Bool(_) => "boolean",
    };
    node.with_property_value("EvidenceValueKind", kind)?;
    if let Some(name) = name {
        node.with_property_value("MemberName", name)?;
    }
    let mut members = Vec::new();
    match value {
        Value::Object(values) => {
            for (name, value) in values {
                members.push(evidence_value(context, descriptor, value, Some(name))?);
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                members.push(evidence_value(context, descriptor, value, Some(&index.to_string()))?);
            }
        }
        Value::Null => {}
        Value::String(value) => {
            node.with_property_value("EvidenceScalarValue", value.as_str())?;
        }
        _ => {
            node.with_property_value("EvidenceScalarValue", value.to_string())?;
        }
    }
    if !members.is_empty() {
        node.add_related_holons("EvidenceMembers", members)?;
    }
    Ok(node)
}
