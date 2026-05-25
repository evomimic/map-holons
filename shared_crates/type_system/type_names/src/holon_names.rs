use base_types::MapString;
use convert_case::{Case, Casing};
use strum_macros::VariantNames;

#[derive(Debug, Clone, VariantNames)]
pub enum CoreHolonTypeName {
    BytesValueConstraint,
    Collection,
    CommandType,
    CommitResponseType,
    Dance,
    DanceType,
    DeclaredRelationshipType,
    Holon,
    HolonErrorType,
    HolonLoadError,
    HolonSpaceType,
    HolonType,
    IntegerValueConstraint,
    InverseRelationshipType,
    MaximumLength,
    MaximumValue,
    MinimumLength,
    MinimumValue,
    SchemaHolonType,
    SchemaType,
    StringValueConstraint,
    TransactionType,
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
            MapString("HolonSpaceType".to_string()),
            CoreHolonTypeName::HolonSpaceType.as_holon_name()
        );
        assert_eq!(
            MapString("HolonType".to_string()),
            CoreHolonTypeName::HolonType.as_holon_name()
        );
        assert_eq!(
            MapString("InverseRelationshipType".to_string()),
            CoreHolonTypeName::InverseRelationshipType.as_holon_name()
        );
        assert_eq!(
            MapString("SchemaHolonType".to_string()),
            CoreHolonTypeName::SchemaHolonType.as_holon_name()
        );
        assert_eq!(
            MapString("TransactionType".to_string()),
            CoreHolonTypeName::TransactionType.as_holon_name()
        );
    }
}
