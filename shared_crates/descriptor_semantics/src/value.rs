#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueViolation {
    EnumVariantNotDeclared,
    IntegerBelowMinimum { minimum: i64, inclusive: bool },
    IntegerAboveMaximum { maximum: i64, inclusive: bool },
    StringBelowMinimumLength { minimum: i64, actual: usize },
    StringAboveMaximumLength { maximum: i64, actual: usize },
}

pub fn validate_enum_variant<'a>(
    variant: &str,
    declared_variants: impl IntoIterator<Item = &'a str>,
) -> Result<(), ValueViolation> {
    if declared_variants.into_iter().any(|declared| declared == variant) {
        Ok(())
    } else {
        Err(ValueViolation::EnumVariantNotDeclared)
    }
}

pub fn validate_integer_minimum(
    value: i64,
    minimum: i64,
    inclusive: bool,
) -> Result<(), ValueViolation> {
    if value > minimum || (inclusive && value == minimum) {
        Ok(())
    } else {
        Err(ValueViolation::IntegerBelowMinimum { minimum, inclusive })
    }
}

pub fn validate_integer_maximum(
    value: i64,
    maximum: i64,
    inclusive: bool,
) -> Result<(), ValueViolation> {
    if value < maximum || (inclusive && value == maximum) {
        Ok(())
    } else {
        Err(ValueViolation::IntegerAboveMaximum { maximum, inclusive })
    }
}

pub fn validate_string_minimum_length(value: &str, minimum: i64) -> Result<(), ValueViolation> {
    let actual = value.chars().count();
    if (actual as i128) >= i128::from(minimum) {
        Ok(())
    } else {
        Err(ValueViolation::StringBelowMinimumLength { minimum, actual })
    }
}

pub fn validate_string_maximum_length(value: &str, maximum: i64) -> Result<(), ValueViolation> {
    let actual = value.chars().count();
    if (actual as i128) <= i128::from(maximum) {
        Ok(())
    } else {
        Err(ValueViolation::StringAboveMaximumLength { maximum, actual })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_membership_uses_exact_declared_identity() {
        assert!(validate_enum_variant("Color.Red", ["Color.Red", "Color.Blue"]).is_ok());
        assert_eq!(
            validate_enum_variant("Color.Reddish", ["Color.Red", "Color.Blue"]),
            Err(ValueViolation::EnumVariantNotDeclared)
        );
    }

    #[test]
    fn integer_bounds_honor_inclusivity() {
        assert!(validate_integer_minimum(2, 2, true).is_ok());
        assert!(validate_integer_minimum(2, 2, false).is_err());
        assert!(validate_integer_maximum(2, 2, true).is_ok());
        assert!(validate_integer_maximum(2, 2, false).is_err());
    }

    #[test]
    fn string_bounds_count_characters() {
        assert!(validate_string_minimum_length("é", 1).is_ok());
        assert!(validate_string_maximum_length("é", 1).is_ok());
        assert!(validate_string_maximum_length("éé", 1).is_err());
    }
}
