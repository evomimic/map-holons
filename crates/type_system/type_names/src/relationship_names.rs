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
            RelationshipName(MapString("ComponentOf".to_string())),
            CoreRelationshipTypeName::ComponentOf.as_relationship_name()
        );
        assert_eq!(
            RelationshipName(MapString("Dependents".to_string())),
            CoreRelationshipTypeName::Dependents.as_relationship_name()
        );
        assert_eq!(
            RelationshipName(MapString("DependsOn".to_string())),
            CoreRelationshipTypeName::DependsOn.as_relationship_name()
        );
        assert_eq!(
            RelationshipName(MapString("DescribedBy".to_string())),
            CoreRelationshipTypeName::DescribedBy.as_relationship_name()
        );
        assert_eq!(
            RelationshipName(MapString("ElementValueType".to_string())),
            CoreRelationshipTypeName::ElementValueType.as_relationship_name()
        );
        assert_eq!(
            RelationshipName(MapString("ElementValueTypeFor".to_string())),
            CoreRelationshipTypeName::ElementValueTypeFor.as_relationship_name()
        );
        assert_eq!(
            RelationshipName(MapString("Extends".to_string())),
            CoreRelationshipTypeName::Extends.as_relationship_name()
        );
        assert_eq!(
            RelationshipName(MapString("InstanceRelationshipFor".to_string())),
            CoreRelationshipTypeName::InstanceRelationshipFor.as_relationship_name()
        );
        assert_eq!(
            RelationshipName(MapString("Instances".to_string())),
            CoreRelationshipTypeName::Instances.as_relationship_name()
        );
        assert_eq!(
            RelationshipName(MapString("InverseOf".to_string())),
            CoreRelationshipTypeName::InverseOf.as_relationship_name()
        );
        assert_eq!(
            RelationshipName(MapString("OwnedBy".to_string())),
            CoreRelationshipTypeName::OwnedBy.as_relationship_name()
        );
        assert_eq!(
            RelationshipName(MapString("Owns".to_string())),
            CoreRelationshipTypeName::Owns.as_relationship_name()
        );
        assert_eq!(
            RelationshipName(MapString("PropertyName".to_string())),
            CoreRelationshipTypeName::PropertyName.as_relationship_name()
        );
        assert_eq!(
            RelationshipName(MapString("SourceOf".to_string())),
            CoreRelationshipTypeName::SourceOf.as_relationship_name()
        );
        assert_eq!(
            RelationshipName(MapString("SourceType".to_string())),
            CoreRelationshipTypeName::SourceType.as_relationship_name()
        );
        assert_eq!(
            RelationshipName(MapString("TargetOf".to_string())),
            CoreRelationshipTypeName::TargetOf.as_relationship_name()
        );
        assert_eq!(
            RelationshipName(MapString("TargetType".to_string())),
            CoreRelationshipTypeName::TargetType.as_relationship_name()
        );
        assert_eq!(
            RelationshipName(MapString("UsesKeyRule".to_string())),
            CoreRelationshipTypeName::UsesKeyRule.as_relationship_name()
        );
        assert_eq!(
            RelationshipName(MapString("ValueType".to_string())),
            CoreRelationshipTypeName::ValueType.as_relationship_name()
        );
        assert_eq!(
            RelationshipName(MapString("ValueTypeFor".to_string())),
            CoreRelationshipTypeName::ValueTypeFor.as_relationship_name()
        );
    }
}
