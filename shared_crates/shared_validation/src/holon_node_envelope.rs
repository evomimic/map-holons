//! Descriptor-independent envelope rules for persisted HolonNode entries.

use integrity_core_types::{HolonNodeModel, PvlMalformedReason, PvlViolation};

use crate::pvl_limits_v1::{
    saturating_u16, saturating_u32, MAX_HOLON_NODE_BYTES, MAX_PROPERTY_COUNT,
};

/// Validates the serialized entry length before the entry is decoded.
pub fn validate_holon_node_size(raw_len: usize) -> Result<(), PvlViolation> {
    if raw_len > MAX_HOLON_NODE_BYTES {
        return Err(PvlViolation::HolonNodeTooLarge {
            actual_bytes: saturating_u32(raw_len),
            max_bytes: saturating_u32(MAX_HOLON_NODE_BYTES),
        });
    }

    Ok(())
}

/// Validates that the stored payload is the canonical encoding of its decoded model.
pub fn validate_holon_node_encoding(raw: &[u8], canonical: &[u8]) -> Result<(), PvlViolation> {
    if raw != canonical {
        return Err(PvlViolation::MalformedHolonNode {
            reason: PvlMalformedReason::NonCanonicalEncoding,
        });
    }

    Ok(())
}

/// Validates the number of properties in a decoded HolonNode.
pub fn validate_property_count(count: usize) -> Result<(), PvlViolation> {
    if count > MAX_PROPERTY_COUNT {
        return Err(PvlViolation::TooManyProperties {
            actual_count: saturating_u16(count),
            max_count: saturating_u16(MAX_PROPERTY_COUNT),
        });
    }

    Ok(())
}

/// Applies decoded-model rules in consensus order: encoding, property count, properties, then
/// identifier shape.
pub fn validate_holon_node_decoded(
    raw: &[u8],
    canonical: &[u8],
    model: &HolonNodeModel,
) -> Result<(), PvlViolation> {
    // Check encoding first: decoding can collapse duplicate map keys and hide the malformed input.
    validate_holon_node_encoding(raw, canonical)?;
    validate_property_count(model.property_map.len())?;
    crate::holon_node_properties::validate_holon_node_properties(&model.property_map)?;

    // Storage SL2 must revisit this rule when `original_id` leaves the persisted entry shape.
    crate::identifier_validation::validate_holon_node_identifiers(model)
}

#[cfg(test)]
mod tests {
    use base_types::{BaseValue, MapString};
    use integrity_core_types::{LocalId, PropertyMap, PropertyName};

    use super::*;

    fn model_with_property_count(count: usize) -> HolonNodeModel {
        let property_map: PropertyMap = (0..count)
            .map(|index| {
                (
                    PropertyName(MapString(format!("property-{index:03}"))),
                    BaseValue::StringValue(MapString("value".to_string())),
                )
            })
            .collect();
        HolonNodeModel::new(None, property_map)
    }

    #[test]
    fn serialized_length_boundary_is_inclusive() {
        assert_eq!(validate_holon_node_size(MAX_HOLON_NODE_BYTES), Ok(()));
        assert_eq!(
            validate_holon_node_size(MAX_HOLON_NODE_BYTES + 1),
            Err(PvlViolation::HolonNodeTooLarge { actual_bytes: 262_145, max_bytes: 262_144 })
        );
    }

    #[test]
    fn property_count_boundary_is_inclusive() {
        assert_eq!(validate_property_count(MAX_PROPERTY_COUNT), Ok(()));
        assert_eq!(
            validate_property_count(MAX_PROPERTY_COUNT + 1),
            Err(PvlViolation::TooManyProperties { actual_count: 257, max_count: 256 })
        );
    }

    #[test]
    fn violation_measurements_saturate_to_contract_widths() {
        assert_eq!(
            validate_property_count(usize::MAX),
            Err(PvlViolation::TooManyProperties { actual_count: u16::MAX, max_count: 256 })
        );

        #[cfg(target_pointer_width = "64")]
        assert_eq!(
            validate_holon_node_size(usize::MAX),
            Err(PvlViolation::HolonNodeTooLarge { actual_bytes: u32::MAX, max_bytes: 262_144 })
        );
    }

    #[test]
    fn encoding_requires_an_exact_byte_match() {
        assert_eq!(validate_holon_node_encoding(&[1, 2], &[1, 2]), Ok(()));
        assert_eq!(
            validate_holon_node_encoding(&[1, 2], &[2, 1]),
            Err(PvlViolation::MalformedHolonNode {
                reason: PvlMalformedReason::NonCanonicalEncoding,
            })
        );
    }

    #[test]
    fn decoded_rules_report_encoding_before_property_count() {
        let model = model_with_property_count(MAX_PROPERTY_COUNT + 1);

        assert_eq!(
            validate_holon_node_decoded(&[1], &[2], &model),
            Err(PvlViolation::MalformedHolonNode {
                reason: PvlMalformedReason::NonCanonicalEncoding,
            })
        );

        assert_eq!(
            validate_holon_node_decoded(&[1, 2, 3], &[1, 2, 3], &model),
            Err(PvlViolation::TooManyProperties { actual_count: 257, max_count: 256 })
        );
    }

    #[test]
    fn decoded_rules_apply_property_validation_after_property_count() {
        let mut property_map = PropertyMap::new();
        property_map.insert(
            PropertyName(MapString(String::new())),
            BaseValue::StringValue(MapString("value".to_string())),
        );
        let model = HolonNodeModel::new(None, property_map);

        assert_eq!(
            validate_holon_node_decoded(&[1, 2, 3], &[1, 2, 3], &model),
            Err(PvlViolation::EmptyPropertyName)
        );
    }

    #[test]
    fn decoded_rules_report_property_count_before_property_rules() {
        let mut model = model_with_property_count(MAX_PROPERTY_COUNT + 1);
        model.property_map.insert(
            PropertyName(MapString(String::new())),
            BaseValue::StringValue(MapString("value".to_string())),
        );

        assert_eq!(
            validate_holon_node_decoded(&[1, 2, 3], &[1, 2, 3], &model),
            Err(PvlViolation::TooManyProperties { actual_count: 258, max_count: 256 })
        );
    }

    #[test]
    fn decoded_rules_accept_an_absent_original_id() {
        let model = HolonNodeModel::new(None, PropertyMap::new());

        assert_eq!(validate_holon_node_decoded(&[1], &[1], &model), Ok(()));
    }

    #[test]
    fn decoded_rules_apply_identifier_validation_after_existing_rules() {
        let invalid_identifier = Some(LocalId(Vec::new()));
        let expected_identifier_violation = PvlViolation::InvalidIdentifier {
            field_name: "original_id".into(),
            identifier_kind: "ActionHash-shaped LocalId".into(),
            reason: "incorrect byte length".into(),
        };

        let empty_properties = HolonNodeModel::new(invalid_identifier.clone(), PropertyMap::new());
        assert_eq!(
            validate_holon_node_decoded(&[1], &[2], &empty_properties),
            Err(PvlViolation::MalformedHolonNode {
                reason: PvlMalformedReason::NonCanonicalEncoding,
            })
        );
        assert_eq!(
            validate_holon_node_decoded(&[1], &[1], &empty_properties),
            Err(expected_identifier_violation.clone())
        );

        let mut too_many_properties = model_with_property_count(MAX_PROPERTY_COUNT + 1);
        too_many_properties.original_id = invalid_identifier.clone();
        assert_eq!(
            validate_holon_node_decoded(&[1], &[1], &too_many_properties),
            Err(PvlViolation::TooManyProperties { actual_count: 257, max_count: 256 })
        );

        let invalid_property = HolonNodeModel::new(
            invalid_identifier,
            PropertyMap::from([(
                PropertyName(MapString(String::new())),
                BaseValue::StringValue(MapString("value".into())),
            )]),
        );
        assert_eq!(
            validate_holon_node_decoded(&[1], &[1], &invalid_property),
            Err(PvlViolation::EmptyPropertyName)
        );
    }
}
