use base_types::MapString;
use convert_case::{Case, Casing};
use integrity_core_types::RelationshipName;
use strum_macros::VariantNames;

pub trait ToRelationshipName {
    fn to_relationship_name(self) -> RelationshipName;
}

impl ToRelationshipName for &str {
    fn to_relationship_name(self) -> RelationshipName {
        let upper = self.to_case(Case::ScreamingSnake);
        RelationshipName(MapString(upper))
    }
}

impl ToRelationshipName for String {
    fn to_relationship_name(self) -> RelationshipName {
        let upper = self.to_case(Case::ScreamingSnake);
        RelationshipName(MapString(upper))
    }
}

impl ToRelationshipName for MapString {
    fn to_relationship_name(self) -> RelationshipName {
        let upper = self.0.to_case(Case::ScreamingSnake);
        RelationshipName(MapString(upper))
    }
}

impl ToRelationshipName for &MapString {
    fn to_relationship_name(self) -> RelationshipName {
        let upper = self.0.to_case(Case::ScreamingSnake);
        RelationshipName(MapString(upper))
    }
}

impl ToRelationshipName for CoreRelationshipTypeName {
    fn to_relationship_name(self) -> RelationshipName {
        // Assuming as_relationship_name() already gives a canonical MapString,
        self.as_relationship_name()
    }
}

impl ToRelationshipName for &CoreRelationshipTypeName {
    fn to_relationship_name(self) -> RelationshipName {
        // Assuming as_relationship_name() already gives a canonical MapString,
        self.clone().as_relationship_name()
    }
}

impl ToRelationshipName for RelationshipName {
    fn to_relationship_name(self) -> RelationshipName {
        // Normalize in case a RelationshipName was constructed ad hoc
        let upper = self.0 .0.to_case(Case::ScreamingSnake);
        RelationshipName(MapString(upper))
    }
}

impl ToRelationshipName for &RelationshipName {
    fn to_relationship_name(self) -> RelationshipName {
        let upper = self.0 .0.to_case(Case::ScreamingSnake);
        RelationshipName(MapString(upper))
    }
}

#[derive(Debug, Clone, VariantNames)]
pub enum CoreRelationshipTypeName {
    BundleMembers,
    ComponentOf,
    Dependents,
    DependsOn,
    DescribedBy,
    ElementValueType,
    ElementValueTypeFor,
    Extends,
    HasLoadError,
    HasRelationshipReference,
    InstanceProperties,
    InstanceRelationshipFor,
    InstanceRelationships,
    Instances,
    InverseOf,
    OwnedBy,
    Owns,
    Predecessor,
    PropertyName,
    ReferenceSource,
    ReferenceTarget,
    SourceOf,
    SourceType,
    Successor,
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
