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
    CardinalityConstraint,
    Collection,
    CommandType,
    CommitResponseType,
    CommitValidationFinding,
    Dance,
    DanceDiagnostic,
    DanceType,
    DanceInvocation,
    DeclaredRelationshipType,
    Holon,
    HolonCollection,
    HolonError,
    HolonLoadError,
    HolonSpace,
    HolonType,
    InverseRelationshipType,
    Projection,
    Schema,
    Transaction,
    TypeDescriptor,
}

impl CoreHolonTypeName {
    pub fn as_holon_name(&self) -> MapString {
        let class_case = format!("{self:?}").to_case(Case::Pascal);
        MapString(class_case)
    }
}

/// Canonical Query Schema holon type names consumed by the Query runtime.
#[derive(Debug, Clone, PartialEq, Eq, VariantNames)]
pub enum QueryHolonTypeName {
    Distinct,
    ExecutionInstance,
    Expand,
    Limit,
    OrderBy,
    OrderBySpec,
    Query,
    QueryExpressionExecution,
    SeedHolons,
    Skip,
}

impl QueryHolonTypeName {
    /// Canonical holon type name in ClassCase (UpperCamel).
    pub fn as_holon_name(&self) -> MapString {
        MapString(format!("{self:?}").to_case(Case::Pascal))
    }
}

/// Canonical Query Dance Adapter Schema dance type names.
#[derive(Debug, Clone, PartialEq, Eq, VariantNames)]
pub enum QueryDanceTypeName {
    QueryDance,
}

impl QueryDanceTypeName {
    /// Canonical dance type name in ClassCase (UpperCamel).
    pub fn as_dance_name(&self) -> DanceName {
        DanceName(MapString(format!("{self:?}").to_case(Case::Pascal)))
    }
}

impl ToDanceName for QueryDanceTypeName {
    fn to_dance_name(self) -> DanceName {
        self.as_dance_name()
    }
}

/// Canonical key of a concrete holon type descriptor: `<TypeName>.HolonType`.
///
/// Concrete holon types are keyed by their own type name and the `HolonType`
/// descriptor family, independent of the abstract type they extend.
pub fn holon_type_descriptor_key(type_name: &MapString) -> MapString {
    MapString(format!("{type_name}.HolonType"))
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
        assert_eq!(
            MapString("HolonCollection".to_string()),
            CoreHolonTypeName::HolonCollection.as_holon_name()
        );
    }

    #[test]
    fn query_holon_type_names_match_their_variants() {
        use strum::VariantNames as _;
        let variants = [
            QueryHolonTypeName::Distinct,
            QueryHolonTypeName::ExecutionInstance,
            QueryHolonTypeName::Expand,
            QueryHolonTypeName::Limit,
            QueryHolonTypeName::OrderBy,
            QueryHolonTypeName::OrderBySpec,
            QueryHolonTypeName::Query,
            QueryHolonTypeName::QueryExpressionExecution,
            QueryHolonTypeName::SeedHolons,
            QueryHolonTypeName::Skip,
        ];
        assert_eq!(QueryHolonTypeName::VARIANTS.len(), variants.len());
        for (variant_name, variant) in QueryHolonTypeName::VARIANTS.iter().zip(variants) {
            assert_eq!(MapString(variant_name.to_string()), variant.as_holon_name());
        }
    }

    #[test]
    fn query_dance_type_name_is_a_dance_name() {
        assert_eq!(
            DanceName(MapString("QueryDance".to_string())),
            QueryDanceTypeName::QueryDance.to_dance_name()
        );
    }

    #[test]
    fn holon_type_descriptor_key_uses_the_holon_type_family() {
        assert_eq!(
            MapString("HolonCollection.HolonType".to_string()),
            holon_type_descriptor_key(&CoreHolonTypeName::HolonCollection.as_holon_name())
        );
        assert_eq!(
            MapString("OrderBySpec.HolonType".to_string()),
            holon_type_descriptor_key(&QueryHolonTypeName::OrderBySpec.as_holon_name())
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
