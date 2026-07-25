//! Descriptor-independent property rules for persisted HolonNode entries.

use integrity_core_types::{PropertyMap, PropertyName, PropertyValue, PvlViolation};

use crate::pvl_limits_v1::{
    raw_byte_len, saturating_u32, utf8_byte_len, MAX_BYTES_VALUE_BYTES, MAX_ENUM_VALUE_BYTES,
    MAX_PROPERTY_NAME_BYTES, MAX_STRING_VALUE_BYTES,
};

/// Validates the representation of a property name.
pub fn validate_property_name(property_name: &PropertyName) -> Result<(), PvlViolation> {
    let name = &property_name.0 .0;

    if name.is_empty() {
        return Err(PvlViolation::EmptyPropertyName);
    }

    if name.starts_with(char::is_whitespace) || name.ends_with(char::is_whitespace) {
        return Err(PvlViolation::InvalidPropertyName {
            reason: "leading or trailing whitespace".into(),
        });
    }

    if name.chars().any(char::is_control) {
        return Err(PvlViolation::InvalidPropertyName { reason: "control character".into() });
    }

    let actual_bytes = utf8_byte_len(name);
    if actual_bytes > MAX_PROPERTY_NAME_BYTES {
        return Err(PvlViolation::PropertyNameTooLong {
            actual_bytes: saturating_u32(actual_bytes),
            max_bytes: saturating_u32(MAX_PROPERTY_NAME_BYTES),
        });
    }

    Ok(())
}

/// Validates the native representation of a property value.
pub fn validate_property_value(
    property_name: &PropertyName,
    value: &PropertyValue,
) -> Result<(), PvlViolation> {
    match value {
        PropertyValue::StringValue(value) => {
            let actual_bytes = utf8_byte_len(&value.0);
            if actual_bytes > MAX_STRING_VALUE_BYTES {
                return Err(PvlViolation::StringValueTooLarge {
                    property_name: property_name.clone(),
                    actual_bytes: saturating_u32(actual_bytes),
                    max_bytes: saturating_u32(MAX_STRING_VALUE_BYTES),
                });
            }
        }
        PropertyValue::BooleanValue(_) => {}
        PropertyValue::IntegerValue(_) => {}
        PropertyValue::EnumValue(value) => {
            let token = &value.0 .0;
            if token.is_empty() {
                return Err(PvlViolation::EmptyEnumValue { property_name: property_name.clone() });
            }

            let actual_bytes = utf8_byte_len(token);
            if actual_bytes > MAX_ENUM_VALUE_BYTES {
                return Err(PvlViolation::EnumValueTooLarge {
                    property_name: property_name.clone(),
                    actual_bytes: saturating_u32(actual_bytes),
                    max_bytes: saturating_u32(MAX_ENUM_VALUE_BYTES),
                });
            }
        }
        PropertyValue::BytesValue(value) => {
            let actual_bytes = raw_byte_len(&value.0);
            if actual_bytes > MAX_BYTES_VALUE_BYTES {
                return Err(PvlViolation::BytesValueTooLarge {
                    property_name: property_name.clone(),
                    actual_bytes: saturating_u32(actual_bytes),
                    max_bytes: saturating_u32(MAX_BYTES_VALUE_BYTES),
                });
            }
        }
    }

    Ok(())
}

/// Validates every property in deterministic map order.
pub fn validate_holon_node_properties(property_map: &PropertyMap) -> Result<(), PvlViolation> {
    for (property_name, value) in property_map {
        validate_property_name(property_name)?;
        validate_property_value(property_name, value)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use base_types::{BaseValue, MapBoolean, MapBytes, MapEnumValue, MapInteger, MapString};

    use super::*;

    fn property_value_from_base_value(value: BaseValue) -> PropertyValue {
        value
    }

    fn base_value_from_property_value(value: PropertyValue) -> BaseValue {
        value
    }

    const _: fn(BaseValue) -> PropertyValue = property_value_from_base_value;
    const _: fn(PropertyValue) -> BaseValue = base_value_from_property_value;

    fn property_name(value: impl Into<String>) -> PropertyName {
        PropertyName(MapString(value.into()))
    }

    #[test]
    fn property_name_boundaries_use_utf8_bytes() {
        assert_eq!(validate_property_name(&property_name("a".repeat(128))), Ok(()));
        assert_eq!(
            validate_property_name(&property_name("a".repeat(129))),
            Err(PvlViolation::PropertyNameTooLong { actual_bytes: 129, max_bytes: 128 })
        );
        assert_eq!(validate_property_name(&property_name("é".repeat(64))), Ok(()));
        assert_eq!(
            validate_property_name(&property_name(format!("{}a", "é".repeat(64)))),
            Err(PvlViolation::PropertyNameTooLong { actual_bytes: 129, max_bytes: 128 })
        );
    }

    #[test]
    fn property_name_representation_rules_are_enforced() {
        assert_eq!(
            validate_property_name(&property_name("")),
            Err(PvlViolation::EmptyPropertyName)
        );

        for name in [" leading", "trailing ", "\u{00a0}leading", "trailing\u{00a0}"] {
            assert_eq!(
                validate_property_name(&property_name(name)),
                Err(PvlViolation::InvalidPropertyName {
                    reason: "leading or trailing whitespace".into(),
                })
            );
        }

        for name in ["interior\tcontrol", "interior\0control"] {
            assert_eq!(
                validate_property_name(&property_name(name)),
                Err(PvlViolation::InvalidPropertyName { reason: "control character".into() })
            );
        }

        assert_eq!(validate_property_name(&property_name("ordinary interior space")), Ok(()));
    }

    #[test]
    fn property_name_rules_have_deterministic_precedence() {
        assert_eq!(
            validate_property_name(&property_name("\t")),
            Err(PvlViolation::InvalidPropertyName {
                reason: "leading or trailing whitespace".into(),
            })
        );

        let control_and_too_long = format!("{}\0", "a".repeat(129));
        assert_eq!(
            validate_property_name(&property_name(control_and_too_long)),
            Err(PvlViolation::InvalidPropertyName { reason: "control character".into() })
        );
    }

    #[test]
    fn string_value_boundaries_use_utf8_bytes() {
        let name = property_name("value");
        assert_eq!(
            validate_property_value(&name, &BaseValue::StringValue(MapString("a".repeat(16_384))),),
            Ok(())
        );
        assert_eq!(
            validate_property_value(&name, &BaseValue::StringValue(MapString("a".repeat(16_385))),),
            Err(PvlViolation::StringValueTooLarge {
                property_name: name.clone(),
                actual_bytes: 16_385,
                max_bytes: 16_384,
            })
        );
        assert_eq!(
            validate_property_value(&name, &BaseValue::StringValue(MapString("é".repeat(8_192))),),
            Ok(())
        );
        assert_eq!(
            validate_property_value(
                &name,
                &BaseValue::StringValue(MapString(format!("{}a", "é".repeat(8_192)))),
            ),
            Err(PvlViolation::StringValueTooLarge {
                property_name: name,
                actual_bytes: 16_385,
                max_bytes: 16_384,
            })
        );
    }

    #[test]
    fn enum_value_rules_are_enforced() {
        let name = property_name("status");
        assert_eq!(
            validate_property_value(
                &name,
                &BaseValue::EnumValue(MapEnumValue(MapString(String::new()))),
            ),
            Err(PvlViolation::EmptyEnumValue { property_name: name.clone() })
        );
        assert_eq!(
            validate_property_value(
                &name,
                &BaseValue::EnumValue(MapEnumValue(MapString("a".repeat(256)))),
            ),
            Ok(())
        );
        assert_eq!(
            validate_property_value(
                &name,
                &BaseValue::EnumValue(MapEnumValue(MapString("a".repeat(257)))),
            ),
            Err(PvlViolation::EnumValueTooLarge {
                property_name: name,
                actual_bytes: 257,
                max_bytes: 256,
            })
        );
    }

    #[test]
    fn bytes_value_boundary_is_inclusive() {
        let name = property_name("payload");
        assert_eq!(
            validate_property_value(&name, &BaseValue::BytesValue(MapBytes(vec![0; 131_072])),),
            Ok(())
        );
        assert_eq!(
            validate_property_value(&name, &BaseValue::BytesValue(MapBytes(vec![0; 131_073])),),
            Err(PvlViolation::BytesValueTooLarge {
                property_name: name,
                actual_bytes: 131_073,
                max_bytes: 131_072,
            })
        );
    }

    #[test]
    fn all_five_scalar_variants_are_supported_at_single_value_depth() {
        let name = property_name("value");
        let values: [PropertyValue; 5] = [
            BaseValue::StringValue(MapString("text".into())),
            BaseValue::BooleanValue(MapBoolean(true)),
            BaseValue::IntegerValue(MapInteger(i64::MIN)),
            BaseValue::EnumValue(MapEnumValue(MapString("member".into()))),
            BaseValue::BytesValue(MapBytes(vec![1, 2, 3])),
        ];

        for value in &values {
            assert_eq!(validate_property_value(&name, value), Ok(()));
        }
    }

    #[test]
    fn property_values_are_concrete_scalar_base_values() {
        let name = property_name("value");
        let value: PropertyValue = BaseValue::IntegerValue(MapInteger(42));
        let mut property_map = PropertyMap::new();

        assert_eq!(property_map.get(&name), None);

        property_map.insert(name.clone(), value);
        let present_value: &BaseValue =
            property_map.get(&name).expect("inserted property must be present");

        assert_eq!(present_value, &BaseValue::IntegerValue(MapInteger(42)));
    }

    #[test]
    fn property_map_checks_names_before_values() {
        let invalid_name = property_name("");
        let mut property_map = PropertyMap::new();
        property_map.insert(invalid_name, BaseValue::StringValue(MapString("a".repeat(16_385))));

        assert_eq!(
            validate_holon_node_properties(&property_map),
            Err(PvlViolation::EmptyPropertyName)
        );
    }

    #[test]
    fn property_map_reports_the_first_violation_in_btree_order() {
        let first_name = property_name("a");
        let second_name = property_name("b");
        let mut property_map = PropertyMap::new();
        property_map.insert(second_name, BaseValue::BytesValue(MapBytes(vec![0; 131_073])));
        property_map.insert(
            first_name.clone(),
            BaseValue::EnumValue(MapEnumValue(MapString(String::new()))),
        );

        assert_eq!(
            validate_holon_node_properties(&property_map),
            Err(PvlViolation::EmptyEnumValue { property_name: first_name })
        );
    }
}
