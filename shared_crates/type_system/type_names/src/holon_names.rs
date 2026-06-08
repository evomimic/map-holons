use base_types::MapString;
use convert_case::{Case, Casing};
use std::fmt;
use strum_macros::VariantNames;

/// A strongly-typed wrapper around the shared descriptor `type_name` for dance types.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DanceName(pub MapString);

impl fmt::Display for DanceName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// Converts common dance-name inputs into a typed `DanceName`.
pub trait ToDanceName {
    fn to_dance_name(self) -> DanceName;
}

#[inline]
fn canonical_dance_name<S: AsRef<str>>(dance_name: S) -> DanceName {
    DanceName(MapString(dance_name.as_ref().to_case(Case::UpperCamel)))
}

impl ToDanceName for &str {
    fn to_dance_name(self) -> DanceName {
        canonical_dance_name(self)
    }
}

impl ToDanceName for String {
    fn to_dance_name(self) -> DanceName {
        DanceName(MapString(self))
    }
}

impl ToDanceName for MapString {
    fn to_dance_name(self) -> DanceName {
        DanceName(self)
    }
}

impl ToDanceName for &MapString {
    fn to_dance_name(self) -> DanceName {
        DanceName(self.clone())
    }
}

impl ToDanceName for DanceName {
    fn to_dance_name(self) -> DanceName {
        self
    }
}

impl ToDanceName for &DanceName {
    fn to_dance_name(self) -> DanceName {
        self.clone()
    }
}

#[derive(Debug, Clone, VariantNames)]
pub enum CoreHolonTypeName {
    BytesValueConstraint,
    Collection,
    CommandType,
    CommitResponseType,
    Dance,
    DanceDiagnostic,
    DanceType,
    DanceInvocation,
    DeclaredRelationshipType,
    Holon,
    HolonError,
    HolonLoadError,
    HolonSpace,
    HolonType,
    IntegerValueConstraint,
    InverseRelationshipType,
    MaximumLength,
    MaximumValue,
    MinimumLength,
    MinimumValue,
    Projection,
    Schema,
    StringValueConstraint,
    Transaction,
    TypeDescriptor,
    ValueArrayConstraint,
    ValueConstraintType,
}

impl CoreHolonTypeName {
    pub fn as_holon_name(&self) -> MapString {
        let class_case = format!("{self:?}").to_case(Case::Pascal);
        MapString(class_case)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variant_string_conversion() {
        assert_eq!(
            MapString("Collection".to_string()),
            CoreHolonTypeName::Collection.as_holon_name()
        );
        assert_eq!(
            MapString("CommandType".to_string()),
            CoreHolonTypeName::CommandType.as_holon_name()
        );
        assert_eq!(
            MapString("DeclaredRelationshipType".to_string()),
            CoreHolonTypeName::DeclaredRelationshipType.as_holon_name()
        );
        assert_eq!(MapString("Dance".to_string()), CoreHolonTypeName::Dance.as_holon_name());
        assert_eq!(MapString("Holon".to_string()), CoreHolonTypeName::Holon.as_holon_name());
        assert_eq!(
            MapString("HolonSpace".to_string()),
            CoreHolonTypeName::HolonSpace.as_holon_name()
        );
        assert_eq!(
            MapString("HolonType".to_string()),
            CoreHolonTypeName::HolonType.as_holon_name()
        );
        assert_eq!(
            MapString("InverseRelationshipType".to_string()),
            CoreHolonTypeName::InverseRelationshipType.as_holon_name()
        );
        assert_eq!(MapString("Schema".to_string()), CoreHolonTypeName::Schema.as_holon_name());
        assert_eq!(
            MapString("Transaction".to_string()),
            CoreHolonTypeName::Transaction.as_holon_name()
        );
        assert_eq!(
            MapString("HolonError".to_string()),
            CoreHolonTypeName::HolonError.as_holon_name()
        );
    }

    #[test]
    fn test_dance_name_conversion() {
        assert_eq!(DanceName(MapString("DanceType".to_string())), "dance_type".to_dance_name());
        assert_eq!(
            DanceName(MapString("AlreadyCanonical".to_string())),
            String::from("AlreadyCanonical").to_dance_name()
        );
    }
}
