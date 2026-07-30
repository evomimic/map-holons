//! Pure, descriptor-independent validation for SmartLink Tag v1 envelopes.
//!
//! The storage-owned codec remains the only parser for Tag v1. This module
//! validates substrate-free endpoint shape, projects every decode failure into
//! the normative PVL contract, and applies semantic rules to the decoded shape.

use core_types::{
    decode_smartlink_tag, DecodedSmartLinkTag, LocalId, SmartLinkDelimitedField,
    SmartLinkReadPosition, SmartLinkTagDecodeError, SmartLinkUtf8Field,
    HOLOCHAIN_ACTION_HASH_BYTES,
};
use integrity_core_types::{PvlField, PvlMalformedReason, PvlViolation};

use crate::{
    identifier_validation::INCORRECT_BYTE_LENGTH,
    pvl_limits_v1::{
        saturating_u16, utf8_byte_len, MAP_SMARTLINK_V1_MAX_BYTES, MAX_CANONICAL_KEY_BYTES,
        MAX_RELATIONSHIP_NAME_BYTES,
    },
    validate_property_map,
};

/// Stable diagnostic token for the SmartLink base endpoint.
pub const SMARTLINK_BASE_ENDPOINT: &str = "base";

/// Stable diagnostic token for the Holochain link-target endpoint.
pub const SMARTLINK_LINK_TARGET_ENDPOINT: &str = "link_target";

/// Stable diagnostic token for an external target's outbound proxy endpoint.
pub const SMARTLINK_OUTBOUND_PROXY_ENDPOINT: &str = "outbound_proxy_id";

/// Validates one raw SmartLink envelope and returns its validated decoded shape.
///
/// Rule order is consensus-visible. Raw size is checked before endpoint shape;
/// endpoint shape before decoding; and relationship properties before cached
/// target properties. Returning the decoded value lets the substrate adapter
/// inspect external routing without invoking the codec a second time.
pub fn validate_smartlink_envelope(
    base_address: &LocalId,
    link_target: &LocalId,
    tag_bytes: &[u8],
) -> Result<DecodedSmartLinkTag, PvlViolation> {
    validate_smartlink_tag_size(tag_bytes.len())?;
    validate_smartlink_endpoint_shape(SMARTLINK_BASE_ENDPOINT, base_address)?;
    validate_smartlink_endpoint_shape(SMARTLINK_LINK_TARGET_ENDPOINT, link_target)?;

    let decoded =
        decode_smartlink_tag(tag_bytes, link_target.clone()).map_err(map_smartlink_decode_error)?;

    validate_relationship_name(&decoded.relationship_name.0 .0)?;
    validate_canonical_key(decoded.canonical_key.as_str())?;
    validate_property_map(&decoded.relationship_property_values)?;
    validate_property_map(&decoded.target_property_values)?;

    Ok(decoded)
}

fn validate_smartlink_tag_size(actual_bytes: usize) -> Result<(), PvlViolation> {
    if actual_bytes > MAP_SMARTLINK_V1_MAX_BYTES {
        return Err(PvlViolation::SmartLinkTagTooLarge {
            actual_bytes: saturating_u16(actual_bytes),
            max_bytes: saturating_u16(MAP_SMARTLINK_V1_MAX_BYTES),
        });
    }

    Ok(())
}

fn validate_smartlink_endpoint_shape(
    endpoint: &str,
    identity: &LocalId,
) -> Result<(), PvlViolation> {
    if identity.as_bytes().len() != HOLOCHAIN_ACTION_HASH_BYTES {
        return Err(PvlViolation::InvalidSmartLinkEndpoint {
            endpoint: endpoint.into(),
            reason: INCORRECT_BYTE_LENGTH.into(),
        });
    }

    Ok(())
}

fn validate_relationship_name(name: &str) -> Result<(), PvlViolation> {
    if name.is_empty() {
        return Err(PvlViolation::EmptyRelationshipName);
    }

    if name.starts_with(char::is_whitespace) || name.ends_with(char::is_whitespace) {
        return Err(PvlViolation::InvalidRelationshipName {
            reason: "leading or trailing whitespace".into(),
        });
    }

    if name.chars().any(char::is_control) {
        return Err(PvlViolation::InvalidRelationshipName { reason: "control character".into() });
    }

    let actual_bytes = utf8_byte_len(name);
    if actual_bytes > MAX_RELATIONSHIP_NAME_BYTES {
        return Err(PvlViolation::RelationshipNameTooLong {
            actual_bytes: saturating_u16(actual_bytes),
            max_bytes: saturating_u16(MAX_RELATIONSHIP_NAME_BYTES),
        });
    }

    Ok(())
}

fn validate_canonical_key(key: &str) -> Result<(), PvlViolation> {
    let actual_bytes = utf8_byte_len(key);
    if actual_bytes > MAX_CANONICAL_KEY_BYTES {
        return Err(PvlViolation::CanonicalKeyTooLarge {
            actual_bytes: saturating_u16(actual_bytes),
            max_bytes: saturating_u16(MAX_CANONICAL_KEY_BYTES),
        });
    }

    Ok(())
}

/// Exhaustively projects a storage-codec decode failure into its PVL result.
///
/// Encoder-only failures are excluded by type. Keeping this match total makes
/// future decoder changes fail compilation until consensus-visible mapping is
/// chosen explicitly.
pub fn map_smartlink_decode_error(error: SmartLinkTagDecodeError) -> PvlViolation {
    use PvlField::*;
    use PvlMalformedReason::*;

    match error {
        SmartLinkTagDecodeError::TagTooLarge { actual, maximum } => {
            PvlViolation::SmartLinkTagTooLarge {
                actual_bytes: saturating_u16(actual),
                max_bytes: saturating_u16(maximum),
            }
        }
        SmartLinkTagDecodeError::InvalidLinkTargetLength { .. } => {
            PvlViolation::InvalidSmartLinkEndpoint {
                endpoint: SMARTLINK_LINK_TARGET_ENDPOINT.into(),
                reason: INCORRECT_BYTE_LENGTH.into(),
            }
        }
        SmartLinkTagDecodeError::InvalidHeader => malformed(InvalidDiscriminant(TagHeader)),
        SmartLinkTagDecodeError::UnexpectedEnd(position) => {
            malformed(MissingField(read_position_field(position)))
        }
        SmartLinkTagDecodeError::MissingDelimiter(field) => {
            malformed(MissingField(delimited_field(field)))
        }
        SmartLinkTagDecodeError::InvalidUtf8(field) => malformed(InvalidUtf8(utf8_field(field))),
        SmartLinkTagDecodeError::UnsupportedVersion(_) => {
            malformed(InvalidDiscriminant(PayloadVersion))
        }
        SmartLinkTagDecodeError::UnknownFlags(_) => malformed(InvalidDiscriminant(PayloadFlags)),
        SmartLinkTagDecodeError::UnknownSectionType(_) => {
            malformed(InvalidDiscriminant(PropertySectionType))
        }
        SmartLinkTagDecodeError::UnknownValueType(_) => {
            malformed(InvalidDiscriminant(PropertyValueDiscriminant))
        }
        SmartLinkTagDecodeError::SectionBoundaryCrossing => {
            malformed(InvalidLength(PropertySection))
        }
        SmartLinkTagDecodeError::InvalidIntegerLength(_) => malformed(InvalidLength(PropertyValue)),
        SmartLinkTagDecodeError::DuplicateSection(_)
        | SmartLinkTagDecodeError::EmptySection(_)
        | SmartLinkTagDecodeError::NonCanonicalSectionOrder
        | SmartLinkTagDecodeError::NonCanonicalPropertyOrder
        | SmartLinkTagDecodeError::InvalidBooleanValue => malformed(NonCanonicalEncoding),
    }
}

fn malformed(reason: PvlMalformedReason) -> PvlViolation {
    PvlViolation::MalformedSmartLink { reason }
}

fn delimited_field(field: SmartLinkDelimitedField) -> PvlField {
    match field {
        SmartLinkDelimitedField::RelationshipName => PvlField::RelationshipName,
        SmartLinkDelimitedField::CanonicalKey => PvlField::CanonicalKey,
    }
}

fn utf8_field(field: SmartLinkUtf8Field) -> PvlField {
    match field {
        SmartLinkUtf8Field::RelationshipName => PvlField::RelationshipName,
        SmartLinkUtf8Field::CanonicalKey => PvlField::CanonicalKey,
        SmartLinkUtf8Field::PropertyName => PvlField::PropertyName,
        SmartLinkUtf8Field::StringPropertyValue | SmartLinkUtf8Field::EnumPropertyValue => {
            PvlField::PropertyValue
        }
    }
}

fn read_position_field(position: SmartLinkReadPosition) -> PvlField {
    match position {
        SmartLinkReadPosition::TagHeader => PvlField::TagHeader,
        SmartLinkReadPosition::PayloadVersion => PvlField::PayloadVersion,
        SmartLinkReadPosition::PayloadFlags => PvlField::PayloadFlags,
        SmartLinkReadPosition::OutboundProxyId => PvlField::OutboundProxyId,
        SmartLinkReadPosition::OccurrenceId => PvlField::OccurrenceId,
        SmartLinkReadPosition::PropertySectionType => PvlField::PropertySectionType,
        SmartLinkReadPosition::PropertySection => PvlField::PropertySection,
    }
}

#[cfg(test)]
mod tests {
    use base_types::{BaseValue, MapEnumValue, MapString};
    use core_types::{
        encode_smartlink_tag, CanonicalKey, HolonId, PropertyMap, PropertyName, RelationshipName,
        SmartLinkTagInput, TargetPropertyCacheCandidate,
    };

    use super::*;

    fn local_id(seed: u8) -> LocalId {
        LocalId(vec![seed; HOLOCHAIN_ACTION_HASH_BYTES])
    }

    fn input(
        relationship_name: impl Into<String>,
        canonical_key: impl Into<String>,
    ) -> SmartLinkTagInput {
        SmartLinkTagInput {
            target_id: HolonId::Local(local_id(2)),
            relationship_name: RelationshipName(MapString(relationship_name.into())),
            canonical_key: CanonicalKey::new(canonical_key.into()).unwrap(),
            occurrence_id: None,
            relationship_property_values: PropertyMap::new(),
            target_property_cache_candidates: Vec::new(),
        }
    }

    fn validate(input: &SmartLinkTagInput) -> Result<DecodedSmartLinkTag, PvlViolation> {
        let tag = encode_smartlink_tag(input).unwrap();
        validate_smartlink_envelope(&local_id(1), &local_id(2), &tag)
    }

    #[test]
    fn valid_envelope_returns_the_codec_decoded_shape() {
        let input = input("RelatedTo", "");
        let tag = encode_smartlink_tag(&input).unwrap();
        let expected = decode_smartlink_tag(&tag, local_id(2)).unwrap();

        assert_eq!(validate_smartlink_envelope(&local_id(1), &local_id(2), &tag), Ok(expected));
    }

    #[test]
    fn size_precedes_endpoint_shape_and_is_inclusive() {
        assert_eq!(validate_smartlink_tag_size(MAP_SMARTLINK_V1_MAX_BYTES), Ok(()));

        let oversized = vec![0; MAP_SMARTLINK_V1_MAX_BYTES + 1];
        assert_eq!(
            validate_smartlink_envelope(&LocalId(Vec::new()), &LocalId(Vec::new()), &oversized),
            Err(PvlViolation::SmartLinkTagTooLarge { actual_bytes: 513, max_bytes: 512 })
        );
    }

    #[test]
    fn endpoint_shape_precedes_decode() {
        let malformed_tag = [];
        assert_eq!(
            validate_smartlink_envelope(&LocalId(vec![0; 38]), &local_id(2), &malformed_tag),
            Err(PvlViolation::InvalidSmartLinkEndpoint {
                endpoint: SMARTLINK_BASE_ENDPOINT.into(),
                reason: INCORRECT_BYTE_LENGTH.into(),
            })
        );
        assert_eq!(
            validate_smartlink_envelope(&local_id(1), &LocalId(vec![0; 40]), &malformed_tag),
            Err(PvlViolation::InvalidSmartLinkEndpoint {
                endpoint: SMARTLINK_LINK_TARGET_ENDPOINT.into(),
                reason: INCORRECT_BYTE_LENGTH.into(),
            })
        );
    }

    #[test]
    fn relationship_name_rules_have_deterministic_precedence_and_byte_boundaries() {
        assert_eq!(validate(&input("", "")), Err(PvlViolation::EmptyRelationshipName));
        assert_eq!(
            validate(&input(" leading", "")),
            Err(PvlViolation::InvalidRelationshipName {
                reason: "leading or trailing whitespace".into(),
            })
        );
        assert_eq!(
            validate(&input("interior\tcontrol", "")),
            Err(PvlViolation::InvalidRelationshipName { reason: "control character".into() })
        );
        assert!(validate(&input("é".repeat(64), "")).is_ok());
        assert_eq!(
            validate(&input(format!("{}a", "é".repeat(64)), "")),
            Err(PvlViolation::RelationshipNameTooLong { actual_bytes: 129, max_bytes: 128 })
        );
    }

    #[test]
    fn canonical_key_is_optional_and_has_an_inclusive_byte_limit() {
        assert!(validate(&input("RelatedTo", "")).is_ok());
        assert!(validate(&input("RelatedTo", "é".repeat(128))).is_ok());
        assert_eq!(
            validate(&input("RelatedTo", format!("{}a", "é".repeat(128)))),
            Err(PvlViolation::CanonicalKeyTooLarge { actual_bytes: 257, max_bytes: 256 })
        );
    }

    #[test]
    fn relationship_properties_are_validated_before_target_cache_properties() {
        let mut input = input("RelatedTo", "");
        let relationship_property_name = PropertyName(MapString("relationship".into()));
        input.relationship_property_values.insert(
            relationship_property_name.clone(),
            BaseValue::EnumValue(MapEnumValue(MapString(String::new()))),
        );
        input.target_property_cache_candidates.push(TargetPropertyCacheCandidate {
            property_name: PropertyName(MapString(String::new())),
            value: BaseValue::StringValue(MapString("encodable".into())),
        });

        assert_eq!(
            validate(&input),
            Err(PvlViolation::EmptyEnumValue { property_name: relationship_property_name })
        );
    }

    #[test]
    fn field_bearing_decoder_failures_map_to_normative_fields() {
        for (position, field) in [
            (SmartLinkReadPosition::TagHeader, PvlField::TagHeader),
            (SmartLinkReadPosition::PayloadVersion, PvlField::PayloadVersion),
            (SmartLinkReadPosition::PayloadFlags, PvlField::PayloadFlags),
            (SmartLinkReadPosition::OutboundProxyId, PvlField::OutboundProxyId),
            (SmartLinkReadPosition::OccurrenceId, PvlField::OccurrenceId),
            (SmartLinkReadPosition::PropertySectionType, PvlField::PropertySectionType),
            (SmartLinkReadPosition::PropertySection, PvlField::PropertySection),
        ] {
            assert_eq!(
                map_smartlink_decode_error(SmartLinkTagDecodeError::UnexpectedEnd(position)),
                malformed(PvlMalformedReason::MissingField(field))
            );
        }

        for (field, expected) in [
            (SmartLinkUtf8Field::RelationshipName, PvlField::RelationshipName),
            (SmartLinkUtf8Field::CanonicalKey, PvlField::CanonicalKey),
            (SmartLinkUtf8Field::PropertyName, PvlField::PropertyName),
            (SmartLinkUtf8Field::StringPropertyValue, PvlField::PropertyValue),
            (SmartLinkUtf8Field::EnumPropertyValue, PvlField::PropertyValue),
        ] {
            assert_eq!(
                map_smartlink_decode_error(SmartLinkTagDecodeError::InvalidUtf8(field)),
                malformed(PvlMalformedReason::InvalidUtf8(expected))
            );
        }
    }

    #[test]
    fn structural_decoder_failures_map_to_normative_reasons() {
        assert_eq!(
            map_smartlink_decode_error(SmartLinkTagDecodeError::TagTooLarge {
                actual: 513,
                maximum: 512,
            }),
            PvlViolation::SmartLinkTagTooLarge { actual_bytes: 513, max_bytes: 512 }
        );
        assert_eq!(
            map_smartlink_decode_error(SmartLinkTagDecodeError::InvalidLinkTargetLength {
                actual: 38,
            }),
            PvlViolation::InvalidSmartLinkEndpoint {
                endpoint: SMARTLINK_LINK_TARGET_ENDPOINT.into(),
                reason: INCORRECT_BYTE_LENGTH.into(),
            }
        );
        assert_eq!(
            map_smartlink_decode_error(SmartLinkTagDecodeError::MissingDelimiter(
                SmartLinkDelimitedField::RelationshipName,
            )),
            malformed(PvlMalformedReason::MissingField(PvlField::RelationshipName))
        );
        assert_eq!(
            map_smartlink_decode_error(SmartLinkTagDecodeError::MissingDelimiter(
                SmartLinkDelimitedField::CanonicalKey,
            )),
            malformed(PvlMalformedReason::MissingField(PvlField::CanonicalKey))
        );

        let cases = [
            (
                SmartLinkTagDecodeError::InvalidHeader,
                PvlMalformedReason::InvalidDiscriminant(PvlField::TagHeader),
            ),
            (
                SmartLinkTagDecodeError::UnsupportedVersion(2),
                PvlMalformedReason::InvalidDiscriminant(PvlField::PayloadVersion),
            ),
            (
                SmartLinkTagDecodeError::UnknownFlags(0x80),
                PvlMalformedReason::InvalidDiscriminant(PvlField::PayloadFlags),
            ),
            (
                SmartLinkTagDecodeError::UnknownSectionType(3),
                PvlMalformedReason::InvalidDiscriminant(PvlField::PropertySectionType),
            ),
            (
                SmartLinkTagDecodeError::UnknownValueType(99),
                PvlMalformedReason::InvalidDiscriminant(PvlField::PropertyValueDiscriminant),
            ),
            (
                SmartLinkTagDecodeError::SectionBoundaryCrossing,
                PvlMalformedReason::InvalidLength(PvlField::PropertySection),
            ),
            (
                SmartLinkTagDecodeError::InvalidIntegerLength(1),
                PvlMalformedReason::InvalidLength(PvlField::PropertyValue),
            ),
            (
                SmartLinkTagDecodeError::InvalidBooleanValue,
                PvlMalformedReason::NonCanonicalEncoding,
            ),
            (SmartLinkTagDecodeError::EmptySection(1), PvlMalformedReason::NonCanonicalEncoding),
            (
                SmartLinkTagDecodeError::DuplicateSection(1),
                PvlMalformedReason::NonCanonicalEncoding,
            ),
            (
                SmartLinkTagDecodeError::NonCanonicalSectionOrder,
                PvlMalformedReason::NonCanonicalEncoding,
            ),
            (
                SmartLinkTagDecodeError::NonCanonicalPropertyOrder,
                PvlMalformedReason::NonCanonicalEncoding,
            ),
        ];

        for (error, reason) in cases {
            assert_eq!(map_smartlink_decode_error(error), malformed(reason));
        }
    }
}
