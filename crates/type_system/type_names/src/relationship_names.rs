use base_types::MapString;
use convert_case::{Case, Casing};
use integrity_core_types::RelationshipName;
use strum_macros::VariantNames;

#[derive(Debug, Clone, VariantNames)]
pub enum CoreRelationshipTypeName {
    ComponentOf,
    Dependents,
    DependsOn,
    DescribedBy,
    ElementValueType,
    ElementValueTypeFor,
    Extends,
    InstanceRelationshipFor,
    Instances,
    InverseOf,
    OwnedBy,
    Owns,
    PropertyName,
    SourceOf,
    SourceType,
    TargetOf,
    TargetType,
    UsesKeyRule,
    ValueType,
    ValueTypeFor,
}

impl CoreRelationshipTypeName {
    pub fn as_relationship_name(&self) -> RelationshipName {
        let upper_case = format!("{self:?}").to_case(Case::ScreamingSnake);
        RelationshipName(MapString(upper_case))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variant_string_conversion() {
        assert_eq!(
            RelationshipName(MapString("COMPONENT_OF".to_string())),
            CoreRelationshipTypeName::ComponentOf.as_relationship_name()
        );

        assert_eq!(
            RelationshipName(MapString("EXTENDS".to_string())),
            CoreRelationshipTypeName::Extends.as_relationship_name()
        );
        assert_eq!(
            RelationshipName(MapString("INSTANCE_RELATIONSHIP_FOR".to_string())),
            CoreRelationshipTypeName::InstanceRelationshipFor.as_relationship_name()
        );
    }
}
