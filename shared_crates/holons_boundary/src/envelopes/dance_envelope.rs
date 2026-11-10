use crate::{DanceRequestWire, DanceResponseWire, HolonReferenceWire, SessionStateWire};
use core_types::HolonError;
use serde::{Deserialize, Serialize};

/// Reference-oriented invocation or the retained legacy request shape.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum DanceEnvelopeRequest {
    Invocation { invocation: HolonReferenceWire },
    Legacy(DanceRequestWire),
}

impl DanceEnvelopeRequest {
    /// Stable transport label; canonical semantics are resolved from the invocation holon.
    pub fn transport_label(&self) -> &str {
        match self {
            Self::Invocation { .. } => "canonical",
            Self::Legacy(request) => &request.dance_name.0,
        }
    }
}

/// Canonical execution returns its actual response holon, or a structured failure.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum DanceEnvelopeResponse {
    Reference { result: Result<HolonReferenceWire, HolonError> },
    Legacy(DanceResponseWire),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DanceRequestEnvelope {
    pub request: DanceEnvelopeRequest,
    /// Commit-only registry-owner state. It is never echoed in response pools.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptor_registry: Option<crate::DescriptorRegistryWire>,
    pub session: Option<SessionStateWire>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DanceResponseEnvelope {
    /// Optional cache data must never invalidate the authoritative persistence response.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_registry_return"
    )]
    pub descriptor_registry: Option<crate::DescriptorRegistryWire>,
    pub response: DanceEnvelopeResponse,
    pub session: Option<SessionStateWire>,
}

fn deserialize_registry_return<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<crate::DescriptorRegistryWire>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum RegistryReturn {
        Valid(crate::DescriptorRegistryWire),
        Malformed(serde::de::IgnoredAny),
    }
    Ok(match Option::<RegistryReturn>::deserialize(deserializer)? {
        Some(RegistryReturn::Valid(registry)) => Some(registry),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn legacy_request_keeps_its_existing_transport_shape() {
        let wire = json!({"request":{"dance_name":"load_holons","dance_type":"Standalone","body":"None"},"session":null});
        let envelope: DanceRequestEnvelope = serde_json::from_value(wire.clone()).unwrap();
        assert!(matches!(envelope.request, DanceEnvelopeRequest::Legacy(_)));
        assert_eq!(serde_json::to_value(envelope).unwrap(), wire);
    }

    #[test]
    fn canonical_transport_carries_only_reference_identity_and_structured_errors() {
        let wire = json!({"request":{"invocation":{"Transient":{"tx_id":41,"id":"2f9dcd83-47ee-482e-8059-28dca43d8a64"}}},"session":null});
        let envelope: DanceRequestEnvelope = serde_json::from_value(wire.clone()).unwrap();
        assert!(matches!(envelope.request, DanceEnvelopeRequest::Invocation { .. }));
        assert_eq!(serde_json::to_value(envelope).unwrap(), wire);
        let failure = DanceResponseEnvelope {
            descriptor_registry: None,
            response: DanceEnvelopeResponse::Reference {
                result: Err(HolonError::InvalidParameter("invalid affording subject".into())),
            },
            session: None,
        };
        let serialized = serde_json::to_value(&failure).unwrap();
        assert_eq!(serde_json::from_value::<DanceResponseEnvelope>(serialized).unwrap(), failure);
    }
    #[test]
    fn malformed_optional_registry_does_not_destroy_commit_response() {
        let wire = json!({"response":{"result":{"Err":{"InvalidParameter":"authoritative outcome"}}},"session":null,"descriptor_registry":{"owner":"malformed"}});
        let envelope: DanceResponseEnvelope = serde_json::from_value(wire).unwrap();
        assert!(envelope.descriptor_registry.is_none());
        assert!(matches!(envelope.response, DanceEnvelopeResponse::Reference { result: Err(_) }));
    }
}
