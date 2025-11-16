use base_types::MapString;
use convert_case::{Case, Casing};
use integrity_core_types::RelationshipName;
use strum_macros::VariantNames;

pub trait ToRelationshipName {
    fn to_relationship_name(self) -> RelationshipName;
}

// --- Internal single point for canonicalization (ClassCase) ---
#[inline]
fn canonical_relationship_name<S: AsRef<str>>(s: S) -> RelationshipName {
    RelationshipName(MapString(s.as_ref().to_case(Case::UpperCamel)))
}

// --- to_relationship_name impls ---

impl ToRelationshipName for &str {
    fn to_relationship_name(self) -> RelationshipName {
        canonical_relationship_name(self) // canonicalize to ClassCase
    }
}

impl ToRelationshipName for String {
    fn to_relationship_name(self) -> RelationshipName {
        // Assume already canonical; pass through unchanged
        RelationshipName(MapString(self))
    }
}

impl ToRelationshipName for MapString {
    fn to_relationship_name(self) -> RelationshipName {
        // Assume already canonical; pass through unchanged
        RelationshipName(self)
    }
}

impl ToRelationshipName for &MapString {
    fn to_relationship_name(self) -> RelationshipName {
        // Assume already canonical; pass through unchanged (clone)
        RelationshipName(self.clone())
    }
}

impl ToRelationshipName for CoreRelationshipTypeName {
    fn to_relationship_name(self) -> RelationshipName {
        self.as_relationship_name() // canonical via enum method
    }
}

impl ToRelationshipName for &CoreRelationshipTypeName {
    fn to_relationship_name(self) -> RelationshipName {
        self.clone().as_relationship_name() // canonical via enum method
    }
}

impl ToRelationshipName for RelationshipName {
    fn to_relationship_name(self) -> RelationshipName {
        self // pass-through unchanged
    }
}

impl ToRelationshipName for &RelationshipName {
    fn to_relationship_name(self) -> RelationshipName {
        self.clone() // pass-through unchanged
    }
}

#[derive(Debug, Clone, VariantNames)]
pub enum CoreRelationshipTypeName {
    BundleMembers,
    ComponentOf,
    Contains,
    Dependents,
    DependsOn,
    DescribedBy,
    ElementValueType,
    ElementValueTypeFor,
    Extends,
    HasInverse,
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
    /// Canonical relationship name in ClassCase (UpperCamel).
    pub fn as_relationship_name(&self) -> RelationshipName {
        let pascal = format!("{self:?}").to_case(Case::UpperCamel);
        RelationshipName(MapString(pascal))
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
            RelationshipName(MapString("Extends".to_string())),
            CoreRelationshipTypeName::Extends.as_relationship_name()
        );
        assert_eq!(
            RelationshipName(MapString("InstanceRelationshipFor".to_string())),
            CoreRelationshipTypeName::InstanceRelationshipFor.as_relationship_name()
        );
    }

    #[test]
    fn test_to_relationship_name_str_and_string() {
        assert_eq!(
            RelationshipName(MapString("InverseOf".to_string())),
            "INVERSE_OF".to_relationship_name() // canonicalized
        );
        assert_eq!(
            RelationshipName(MapString("AlreadyCanonical".to_string())),
            String::from("AlreadyCanonical").to_relationship_name() // pass-through
        );
    }
}
