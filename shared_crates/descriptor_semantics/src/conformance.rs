/// Requiredness derived from a descriptor property name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropertyRequirement<'a> {
    pub name: &'a str,
    pub required: bool,
}

/// Interprets the MAP optional-property `?` suffix without owning source syntax.
pub fn property_requirement(name: &str) -> PropertyRequirement<'_> {
    match name.strip_suffix('?') {
        Some(name) => PropertyRequirement { name, required: false },
        None => PropertyRequirement { name, required: true },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardinalityViolation {
    pub actual: usize,
    pub minimum: usize,
    pub maximum: usize,
}

pub fn validate_cardinality(
    actual: usize,
    minimum: usize,
    maximum: usize,
) -> Result<(), CardinalityViolation> {
    if actual >= minimum && actual <= maximum {
        Ok(())
    } else {
        Err(CardinalityViolation { actual, minimum, maximum })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_suffix_is_descriptor_data() {
        assert_eq!(
            property_requirement("description?"),
            PropertyRequirement { name: "description", required: false }
        );
        assert_eq!(
            property_requirement("type_name"),
            PropertyRequirement { name: "type_name", required: true }
        );
    }

    #[test]
    fn cardinality_is_inclusive() {
        assert!(validate_cardinality(1, 1, 2).is_ok());
        assert!(validate_cardinality(2, 1, 2).is_ok());
        assert_eq!(
            validate_cardinality(3, 1, 2),
            Err(CardinalityViolation { actual: 3, minimum: 1, maximum: 2 })
        );
    }
}
