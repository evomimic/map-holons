use crate::property::PropertyName;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Structural position within a PVL v1 native-entry or SmartLink grammar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PvlField {
    PropertyName,
    PropertyValueDiscriminant,
    PropertyValue,
    HolonNodeEntry,
    PropertyMap,
    TagHeader,
    RelationshipName,
    CanonicalKey,
    PayloadVersion,
    PayloadFlags,
    OutboundProxyId,
    OccurrenceId,
    PropertySectionType,
    PropertySection,
}

impl PvlField {
    /// Returns the stable grammar-position token used in deterministic messages.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PropertyName => "PropertyName",
            Self::PropertyValueDiscriminant => "PropertyValueDiscriminant",
            Self::PropertyValue => "PropertyValue",
            Self::HolonNodeEntry => "HolonNodeEntry",
            Self::PropertyMap => "PropertyMap",
            Self::TagHeader => "TagHeader",
            Self::RelationshipName => "RelationshipName",
            Self::CanonicalKey => "CanonicalKey",
            Self::PayloadVersion => "PayloadVersion",
            Self::PayloadFlags => "PayloadFlags",
            Self::OutboundProxyId => "OutboundProxyId",
            Self::OccurrenceId => "OccurrenceId",
            Self::PropertySectionType => "PropertySectionType",
            Self::PropertySection => "PropertySection",
        }
    }
}

impl fmt::Display for PvlField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Stable classification of a malformed PVL v1 representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PvlMalformedReason {
    DecodeFailed,
    MissingField(PvlField),
    InvalidDiscriminant(PvlField),
    InvalidUtf8(PvlField),
    InvalidLength(PvlField),
    InvalidFieldCombination,
    NonCanonicalEncoding,
}

impl fmt::Display for PvlMalformedReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DecodeFailed => formatter.write_str("decode failed"),
            Self::MissingField(field) => write!(formatter, "missing {field}"),
            Self::InvalidDiscriminant(field) => {
                write!(formatter, "invalid discriminant at {field}")
            }
            Self::InvalidUtf8(field) => write!(formatter, "invalid UTF-8 in {field}"),
            Self::InvalidLength(field) => write!(formatter, "invalid length for {field}"),
            Self::InvalidFieldCombination => formatter.write_str("invalid field combination"),
            Self::NonCanonicalEncoding => formatter.write_str("non-canonical encoding"),
        }
    }
}

/// Descriptor-independent PVL v1 validation violation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PvlViolation {
    MalformedHolonNode {
        reason: PvlMalformedReason,
    },
    MalformedSmartLink {
        reason: PvlMalformedReason,
    },
    UnsupportedNativeValue {
        property_name: Option<PropertyName>,
        value_kind: String,
    },
    EmptyEnumValue {
        property_name: PropertyName,
    },
    MalformedPropertyValue {
        property_name: PropertyName,
        reason: PvlMalformedReason,
    },
    HolonNodeTooLarge {
        actual_bytes: u32,
        max_bytes: u32,
    },
    TooManyProperties {
        actual_count: u16,
        max_count: u16,
    },
    PropertyNameTooLong {
        actual_bytes: u32,
        max_bytes: u32,
    },
    StringValueTooLarge {
        property_name: PropertyName,
        actual_bytes: u32,
        max_bytes: u32,
    },
    EnumValueTooLarge {
        property_name: PropertyName,
        actual_bytes: u32,
        max_bytes: u32,
    },
    BytesValueTooLarge {
        property_name: PropertyName,
        actual_bytes: u32,
        max_bytes: u32,
    },
    CanonicalKeyTooLarge {
        actual_bytes: u16,
        max_bytes: u16,
    },
    CollectionTooLarge {
        property_name: PropertyName,
        actual_items: u32,
        max_items: u32,
    },
    ValueNestingTooDeep {
        property_name: PropertyName,
        actual_depth: u8,
        max_depth: u8,
    },
    SmartLinkTagTooLarge {
        actual_bytes: u16,
        max_bytes: u16,
    },
    RelationshipNameTooLong {
        actual_bytes: u16,
        max_bytes: u16,
    },
    IdentifierTooLong {
        field_name: String,
        identifier_kind: String,
        actual_bytes: u16,
        max_bytes: u16,
    },
    ValidationDependencyLimitExceeded {
        requested: u8,
        max: u8,
    },
    EmptyPropertyName,
    InvalidPropertyName {
        reason: String,
    },
    EmptyRelationshipName,
    InvalidRelationshipName {
        reason: String,
    },
    HeterogeneousCollection {
        property_name: PropertyName,
        expected_kind: String,
        actual_kind: String,
        item_index: u32,
    },
    InvalidIdentifier {
        field_name: String,
        identifier_kind: String,
        reason: String,
    },
    EmptyIdentifier {
        field_name: String,
        identifier_kind: String,
    },
    InvalidSmartLinkEndpoint {
        endpoint: String,
        reason: String,
    },
    UnsupportedSmartLinkEndpointKind {
        endpoint: String,
        endpoint_kind: String,
    },
    InvalidUpdateTarget {
        expected_entry_kind: String,
        actual_entry_kind: String,
    },
    ImmutableNativeFieldChanged {
        field_name: String,
    },
    InvalidDeleteTarget {
        expected_target_kind: String,
        actual_target_kind: String,
    },
}

impl PvlViolation {
    /// Returns the stable consensus-visible error code for this violation.
    ///
    /// Codes must never be reused after release. `MAP-PVL-2110` through
    /// `MAP-PVL-2119` are reserved for future forward-reference provenance.
    pub fn code(&self) -> &'static str {
        match self {
            Self::MalformedHolonNode { .. } => "MAP-PVL-1001",
            Self::UnsupportedNativeValue { .. } => "MAP-PVL-1002",
            Self::HolonNodeTooLarge { .. } => "MAP-PVL-1003",
            Self::TooManyProperties { .. } => "MAP-PVL-1101",
            Self::EmptyPropertyName => "MAP-PVL-1102",
            Self::InvalidPropertyName { .. } => "MAP-PVL-1103",
            Self::PropertyNameTooLong { .. } => "MAP-PVL-1104",
            Self::StringValueTooLarge { .. } => "MAP-PVL-1110",
            Self::EnumValueTooLarge { .. } => "MAP-PVL-1111",
            Self::BytesValueTooLarge { .. } => "MAP-PVL-1112",
            Self::CollectionTooLarge { .. } => "MAP-PVL-1113",
            Self::HeterogeneousCollection { .. } => "MAP-PVL-1114",
            Self::ValueNestingTooDeep { .. } => "MAP-PVL-1115",
            Self::EmptyEnumValue { .. } => "MAP-PVL-1116",
            Self::MalformedPropertyValue { .. } => "MAP-PVL-1117",
            Self::InvalidIdentifier { .. } => "MAP-PVL-1201",
            Self::EmptyIdentifier { .. } => "MAP-PVL-1202",
            Self::IdentifierTooLong { .. } => "MAP-PVL-1203",
            Self::InvalidUpdateTarget { .. } => "MAP-PVL-1301",
            Self::ImmutableNativeFieldChanged { .. } => "MAP-PVL-1302",
            Self::InvalidDeleteTarget { .. } => "MAP-PVL-1303",
            Self::MalformedSmartLink { .. } => "MAP-PVL-2001",
            Self::InvalidSmartLinkEndpoint { .. } => "MAP-PVL-2002",
            Self::UnsupportedSmartLinkEndpointKind { .. } => "MAP-PVL-2003",
            Self::EmptyRelationshipName => "MAP-PVL-2101",
            Self::InvalidRelationshipName { .. } => "MAP-PVL-2102",
            Self::RelationshipNameTooLong { .. } => "MAP-PVL-2103",
            Self::SmartLinkTagTooLarge { .. } => "MAP-PVL-2201",
            Self::CanonicalKeyTooLarge { .. } => "MAP-PVL-2202",
            Self::ValidationDependencyLimitExceeded { .. } => "MAP-PVL-3001",
        }
    }
}

impl fmt::Display for PvlViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: ", self.code())?;
        match self {
            Self::MalformedHolonNode { reason } => {
                write!(formatter, "malformed HolonNode ({reason})")
            }
            Self::MalformedSmartLink { reason } => {
                write!(formatter, "malformed SmartLink ({reason})")
            }
            Self::UnsupportedNativeValue { .. } => {
                formatter.write_str("unsupported native value representation")
            }
            Self::EmptyEnumValue { .. } => formatter.write_str("enum value is empty"),
            Self::MalformedPropertyValue { reason, .. } => {
                write!(formatter, "malformed property value ({reason})")
            }
            Self::HolonNodeTooLarge { max_bytes, .. } => {
                write!(formatter, "HolonNode exceeds {max_bytes}-byte limit")
            }
            Self::TooManyProperties { max_count, .. } => {
                write!(formatter, "property count exceeds {max_count}")
            }
            Self::PropertyNameTooLong { max_bytes, .. } => {
                write!(formatter, "property name exceeds {max_bytes}-byte limit")
            }
            Self::StringValueTooLarge { max_bytes, .. } => {
                write!(formatter, "string value exceeds {max_bytes}-byte limit")
            }
            Self::EnumValueTooLarge { max_bytes, .. } => {
                write!(formatter, "enum value exceeds {max_bytes}-byte limit")
            }
            Self::BytesValueTooLarge { max_bytes, .. } => {
                write!(formatter, "bytes value exceeds {max_bytes}-byte limit")
            }
            Self::CanonicalKeyTooLarge { max_bytes, .. } => {
                write!(formatter, "canonical key exceeds {max_bytes}-byte limit")
            }
            Self::CollectionTooLarge { max_items, .. } => {
                write!(formatter, "collection exceeds {max_items}-item limit")
            }
            Self::ValueNestingTooDeep { max_depth, .. } => {
                write!(formatter, "value nesting exceeds depth {max_depth}")
            }
            Self::SmartLinkTagTooLarge { max_bytes, .. } => {
                write!(formatter, "SmartLink tag exceeds {max_bytes}-byte limit")
            }
            Self::RelationshipNameTooLong { max_bytes, .. } => {
                write!(formatter, "relationship name exceeds {max_bytes}-byte limit")
            }
            Self::IdentifierTooLong { max_bytes, .. } => {
                write!(formatter, "identifier exceeds {max_bytes}-byte limit")
            }
            Self::ValidationDependencyLimitExceeded { max, .. } => {
                write!(formatter, "validation dependency count exceeds {max}")
            }
            Self::EmptyPropertyName => formatter.write_str("property name is empty"),
            Self::InvalidPropertyName { .. } => formatter.write_str("property name is invalid"),
            Self::EmptyRelationshipName => formatter.write_str("relationship name is empty"),
            Self::InvalidRelationshipName { .. } => {
                formatter.write_str("relationship name is invalid")
            }
            Self::HeterogeneousCollection { item_index, .. } => {
                write!(formatter, "collection item {item_index} has a different value kind")
            }
            Self::InvalidIdentifier { .. } => formatter.write_str("identifier is invalid"),
            Self::EmptyIdentifier { .. } => formatter.write_str("identifier is empty"),
            Self::InvalidSmartLinkEndpoint { .. } => {
                formatter.write_str("SmartLink endpoint is invalid")
            }
            Self::UnsupportedSmartLinkEndpointKind { .. } => {
                formatter.write_str("SmartLink endpoint kind is unsupported")
            }
            Self::InvalidUpdateTarget { .. } => formatter.write_str("update target is invalid"),
            Self::ImmutableNativeFieldChanged { .. } => {
                formatter.write_str("immutable native field changed")
            }
            Self::InvalidDeleteTarget { .. } => formatter.write_str("delete target is invalid"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base_types::MapString;
    use serde_json::{json, Value};
    use std::collections::HashSet;

    fn property_name(value: &str) -> PropertyName {
        PropertyName(MapString::from(value))
    }

    fn all_variants() -> Vec<PvlViolation> {
        let name = || property_name("status");
        vec![
            PvlViolation::MalformedHolonNode { reason: PvlMalformedReason::DecodeFailed },
            PvlViolation::MalformedSmartLink {
                reason: PvlMalformedReason::MissingField(PvlField::RelationshipName),
            },
            PvlViolation::UnsupportedNativeValue { property_name: None, value_kind: "Map".into() },
            PvlViolation::EmptyEnumValue { property_name: name() },
            PvlViolation::MalformedPropertyValue {
                property_name: name(),
                reason: PvlMalformedReason::NonCanonicalEncoding,
            },
            PvlViolation::HolonNodeTooLarge { actual_bytes: 1, max_bytes: 1 },
            PvlViolation::TooManyProperties { actual_count: 1, max_count: 1 },
            PvlViolation::PropertyNameTooLong { actual_bytes: 1, max_bytes: 1 },
            PvlViolation::StringValueTooLarge {
                property_name: name(),
                actual_bytes: 1,
                max_bytes: 1,
            },
            PvlViolation::EnumValueTooLarge {
                property_name: name(),
                actual_bytes: 1,
                max_bytes: 1,
            },
            PvlViolation::BytesValueTooLarge {
                property_name: name(),
                actual_bytes: 1,
                max_bytes: 1,
            },
            PvlViolation::CanonicalKeyTooLarge { actual_bytes: 1, max_bytes: 1 },
            PvlViolation::CollectionTooLarge {
                property_name: name(),
                actual_items: 1,
                max_items: 1,
            },
            PvlViolation::ValueNestingTooDeep {
                property_name: name(),
                actual_depth: 1,
                max_depth: 1,
            },
            PvlViolation::SmartLinkTagTooLarge { actual_bytes: 1, max_bytes: 1 },
            PvlViolation::RelationshipNameTooLong { actual_bytes: 1, max_bytes: 1 },
            PvlViolation::IdentifierTooLong {
                field_name: "id".into(),
                identifier_kind: "LocalId".into(),
                actual_bytes: 1,
                max_bytes: 1,
            },
            PvlViolation::ValidationDependencyLimitExceeded { requested: 1, max: 1 },
            PvlViolation::EmptyPropertyName,
            PvlViolation::InvalidPropertyName { reason: "control character".into() },
            PvlViolation::EmptyRelationshipName,
            PvlViolation::InvalidRelationshipName { reason: "whitespace".into() },
            PvlViolation::HeterogeneousCollection {
                property_name: name(),
                expected_kind: "String".into(),
                actual_kind: "Integer".into(),
                item_index: 1,
            },
            PvlViolation::InvalidIdentifier {
                field_name: "id".into(),
                identifier_kind: "LocalId".into(),
                reason: "bad hash".into(),
            },
            PvlViolation::EmptyIdentifier {
                field_name: "id".into(),
                identifier_kind: "LocalId".into(),
            },
            PvlViolation::InvalidSmartLinkEndpoint {
                endpoint: "source".into(),
                reason: "bad hash".into(),
            },
            PvlViolation::UnsupportedSmartLinkEndpointKind {
                endpoint: "target".into(),
                endpoint_kind: "AgentPubKey".into(),
            },
            PvlViolation::InvalidUpdateTarget {
                expected_entry_kind: "HolonNode".into(),
                actual_entry_kind: "Other".into(),
            },
            PvlViolation::ImmutableNativeFieldChanged { field_name: "id".into() },
            PvlViolation::InvalidDeleteTarget {
                expected_target_kind: "Create".into(),
                actual_target_kind: "Update".into(),
            },
        ]
    }

    #[test]
    fn registry_assigns_one_unique_well_formed_code_to_every_variant() {
        let variants = all_variants();
        let codes: Vec<_> = variants.iter().map(exhaustive_code).collect();
        let unique: HashSet<_> = codes.iter().copied().collect();

        assert_eq!(codes.len(), 30);
        assert_eq!(unique.len(), codes.len());
        assert!(codes.iter().all(|code| {
            code.strip_prefix("MAP-PVL-").is_some_and(|digits| {
                digits.len() == 4 && digits.bytes().all(|byte| byte.is_ascii_digit())
            })
        }));
        assert!(!unique.contains("MAP-PVL-1118"));
        assert!(
            (2110..=2119).all(|reserved| !unique.contains(format!("MAP-PVL-{reserved}").as_str()))
        );
    }

    fn exhaustive_code(violation: &PvlViolation) -> &'static str {
        match violation {
            PvlViolation::MalformedHolonNode { .. }
            | PvlViolation::MalformedSmartLink { .. }
            | PvlViolation::UnsupportedNativeValue { .. }
            | PvlViolation::EmptyEnumValue { .. }
            | PvlViolation::MalformedPropertyValue { .. }
            | PvlViolation::HolonNodeTooLarge { .. }
            | PvlViolation::TooManyProperties { .. }
            | PvlViolation::PropertyNameTooLong { .. }
            | PvlViolation::StringValueTooLarge { .. }
            | PvlViolation::EnumValueTooLarge { .. }
            | PvlViolation::BytesValueTooLarge { .. }
            | PvlViolation::CanonicalKeyTooLarge { .. }
            | PvlViolation::CollectionTooLarge { .. }
            | PvlViolation::ValueNestingTooDeep { .. }
            | PvlViolation::SmartLinkTagTooLarge { .. }
            | PvlViolation::RelationshipNameTooLong { .. }
            | PvlViolation::IdentifierTooLong { .. }
            | PvlViolation::ValidationDependencyLimitExceeded { .. }
            | PvlViolation::EmptyPropertyName
            | PvlViolation::InvalidPropertyName { .. }
            | PvlViolation::EmptyRelationshipName
            | PvlViolation::InvalidRelationshipName { .. }
            | PvlViolation::HeterogeneousCollection { .. }
            | PvlViolation::InvalidIdentifier { .. }
            | PvlViolation::EmptyIdentifier { .. }
            | PvlViolation::InvalidSmartLinkEndpoint { .. }
            | PvlViolation::UnsupportedSmartLinkEndpointKind { .. }
            | PvlViolation::InvalidUpdateTarget { .. }
            | PvlViolation::ImmutableNativeFieldChanged { .. }
            | PvlViolation::InvalidDeleteTarget { .. } => violation.code(),
        }
    }

    #[test]
    fn messages_use_stable_codes_and_numeric_limits() {
        assert_eq!(
            PvlViolation::HolonNodeTooLarge { actual_bytes: 300_000, max_bytes: 262_144 }
                .to_string(),
            "MAP-PVL-1003: HolonNode exceeds 262144-byte limit"
        );
        assert_eq!(
            PvlViolation::TooManyProperties { actual_count: 300, max_count: 256 }.to_string(),
            "MAP-PVL-1101: property count exceeds 256"
        );
        assert_eq!(
            PvlViolation::SmartLinkTagTooLarge { actual_bytes: 600, max_bytes: 512 }.to_string(),
            "MAP-PVL-2201: SmartLink tag exceeds 512-byte limit"
        );
        assert_eq!(
            PvlViolation::CanonicalKeyTooLarge { actual_bytes: 300, max_bytes: 256 }.to_string(),
            "MAP-PVL-2202: canonical key exceeds 256-byte limit"
        );
    }

    #[test]
    fn malformed_messages_use_fixed_reason_and_field_tokens() {
        assert_eq!(
            PvlViolation::MalformedSmartLink {
                reason: PvlMalformedReason::MissingField(PvlField::RelationshipName),
            }
            .to_string(),
            "MAP-PVL-2001: malformed SmartLink (missing RelationshipName)"
        );
        assert_eq!(
            PvlViolation::MalformedHolonNode { reason: PvlMalformedReason::DecodeFailed }
                .to_string(),
            "MAP-PVL-1001: malformed HolonNode (decode failed)"
        );
        assert_eq!(
            PvlViolation::MalformedPropertyValue {
                property_name: property_name("secret-property"),
                reason: PvlMalformedReason::NonCanonicalEncoding,
            }
            .to_string(),
            "MAP-PVL-1117: malformed property value (non-canonical encoding)"
        );
    }

    #[test]
    fn messages_do_not_leak_diagnostic_strings_or_property_names() {
        let violation = PvlViolation::HeterogeneousCollection {
            property_name: property_name("secret-property"),
            expected_kind: "secret-expected".into(),
            actual_kind: "secret-actual".into(),
            item_index: 7,
        };
        let message = violation.to_string();
        assert_eq!(message, "MAP-PVL-1114: collection item 7 has a different value kind");
        assert!(!message.contains("secret"));
    }

    #[test]
    fn serde_round_trips_materially_distinct_payload_shapes() {
        let cases: Vec<(PvlViolation, Value)> = vec![
            (PvlViolation::EmptyPropertyName, json!("EmptyPropertyName")),
            (
                PvlViolation::MalformedSmartLink {
                    reason: PvlMalformedReason::MissingField(PvlField::RelationshipName),
                },
                json!({"MalformedSmartLink":{"reason":{"MissingField":"RelationshipName"}}}),
            ),
            (
                PvlViolation::UnsupportedNativeValue {
                    property_name: Some(property_name("status")),
                    value_kind: "Map".into(),
                },
                json!({"UnsupportedNativeValue":{"property_name":"status","value_kind":"Map"}}),
            ),
            (
                PvlViolation::UnsupportedNativeValue {
                    property_name: None,
                    value_kind: "Map".into(),
                },
                json!({"UnsupportedNativeValue":{"property_name":null,"value_kind":"Map"}}),
            ),
            (
                PvlViolation::StringValueTooLarge {
                    property_name: property_name("title"),
                    actual_bytes: 20_000,
                    max_bytes: 16_384,
                },
                json!({"StringValueTooLarge":{"property_name":"title","actual_bytes":20000,"max_bytes":16384}}),
            ),
            (
                PvlViolation::CanonicalKeyTooLarge { actual_bytes: 300, max_bytes: 256 },
                json!({"CanonicalKeyTooLarge":{"actual_bytes":300,"max_bytes":256}}),
            ),
            (
                PvlViolation::ValueNestingTooDeep {
                    property_name: property_name("items"),
                    actual_depth: 3,
                    max_depth: 2,
                },
                json!({"ValueNestingTooDeep":{"property_name":"items","actual_depth":3,"max_depth":2}}),
            ),
            (
                PvlViolation::IdentifierTooLong {
                    field_name: "local_id".into(),
                    identifier_kind: "LocalId".into(),
                    actual_bytes: 300,
                    max_bytes: 256,
                },
                json!({"IdentifierTooLong":{"field_name":"local_id","identifier_kind":"LocalId","actual_bytes":300,"max_bytes":256}}),
            ),
        ];

        for (violation, expected_json) in cases {
            let encoded = serde_json::to_value(&violation).unwrap();
            assert_eq!(encoded, expected_json);
            assert_eq!(serde_json::from_value::<PvlViolation>(encoded).unwrap(), violation);
        }
    }
}
