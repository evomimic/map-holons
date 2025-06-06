
// Auto-generated enum from enum_template.rs
use std::str::FromStr;
use std::fmt;
use strum_macros::EnumIter;

#[derive(Debug, Clone, EnumIter, Default, PartialEq, Eq)]
pub enum CoreRelationshipTypeNameName {
    #[default]
    CoreSchema,
    CoreSchemaFor,
    CollectionFor,
    Components,
    ComponentOf,
    DescribedBy,
    HasInverse,
    HasSubtype,
    Instances,
    InverseOf,
    IsA,
    OwnedBy,
    Owns,
    Predecessor,
    Properties,
    PropertyOf,
    SourceFor,
    SourceHolonType,
    Successor,
    TargetCollectionType,
    TargetHolonType,
    TargetOfCollectionType,
    ValueType,
    ValueTypeFor,
}

impl fmt::Display for CoreRelationshipTypeNameName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreRelationshipTypeNameName::CoreSchema => write!(f, "CoreSchema"),
            CoreRelationshipTypeNameName::CoreSchemaFor => write!(f, "CoreSchemaFor"),
            CoreRelationshipTypeNameName::CollectionFor => write!(f, "CollectionFor"),
            CoreRelationshipTypeNameName::Components => write!(f, "Components"),
            CoreRelationshipTypeNameName::ComponentOf => write!(f, "ComponentOf"),
            CoreRelationshipTypeNameName::DescribedBy => write!(f, "DescribedBy"),
            CoreRelationshipTypeNameName::HasInverse => write!(f, "HasInverse"),
            CoreRelationshipTypeNameName::HasSubtype => write!(f, "HasSubtype"),
            CoreRelationshipTypeNameName::Instances => write!(f, "Instances"),
            CoreRelationshipTypeNameName::InverseOf => write!(f, "InverseOf"),
            CoreRelationshipTypeNameName::IsA => write!(f, "IsA"),
            CoreRelationshipTypeNameName::OwnedBy => write!(f, "OwnedBy"),
            CoreRelationshipTypeNameName::Owns => write!(f, "Owns"),
            CoreRelationshipTypeNameName::Predecessor => write!(f, "Predecessor"),
            CoreRelationshipTypeNameName::Properties => write!(f, "Properties"),
            CoreRelationshipTypeNameName::PropertyOf => write!(f, "PropertyOf"),
            CoreRelationshipTypeNameName::SourceFor => write!(f, "SourceFor"),
            CoreRelationshipTypeNameName::SourceHolonType => write!(f, "SourceHolonType"),
            CoreRelationshipTypeNameName::Successor => write!(f, "Successor"),
            CoreRelationshipTypeNameName::TargetCollectionType => write!(f, "TargetCollectionType"),
            CoreRelationshipTypeNameName::TargetHolonType => write!(f, "TargetHolonType"),
            CoreRelationshipTypeNameName::TargetOfCollectionType => write!(f, "TargetOfCollectionType"),
            CoreRelationshipTypeNameName::ValueType => write!(f, "ValueType"),
            CoreRelationshipTypeNameName::ValueTypeFor => write!(f, "ValueTypeFor"),
        }
    }
}

impl FromStr for CoreRelationshipTypeNameName {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "CoreSchema" => Ok(CoreRelationshipTypeNameName::CoreSchema),
            "CoreSchemaFor" => Ok(CoreRelationshipTypeNameName::CoreSchemaFor),
            "CollectionFor" => Ok(CoreRelationshipTypeNameName::CollectionFor),
            "Components" => Ok(CoreRelationshipTypeNameName::Components),
            "ComponentOf" => Ok(CoreRelationshipTypeNameName::ComponentOf),
            "DescribedBy" => Ok(CoreRelationshipTypeNameName::DescribedBy),
            "HasInverse" => Ok(CoreRelationshipTypeNameName::HasInverse),
            "HasSubtype" => Ok(CoreRelationshipTypeNameName::HasSubtype),
            "Instances" => Ok(CoreRelationshipTypeNameName::Instances),
            "InverseOf" => Ok(CoreRelationshipTypeNameName::InverseOf),
            "IsA" => Ok(CoreRelationshipTypeNameName::IsA),
            "OwnedBy" => Ok(CoreRelationshipTypeNameName::OwnedBy),
            "Owns" => Ok(CoreRelationshipTypeNameName::Owns),
            "Predecessor" => Ok(CoreRelationshipTypeNameName::Predecessor),
            "Properties" => Ok(CoreRelationshipTypeNameName::Properties),
            "PropertyOf" => Ok(CoreRelationshipTypeNameName::PropertyOf),
            "SourceFor" => Ok(CoreRelationshipTypeNameName::SourceFor),
            "SourceHolonType" => Ok(CoreRelationshipTypeNameName::SourceHolonType),
            "Successor" => Ok(CoreRelationshipTypeNameName::Successor),
            "TargetCollectionType" => Ok(CoreRelationshipTypeNameName::TargetCollectionType),
            "TargetHolonType" => Ok(CoreRelationshipTypeNameName::TargetHolonType),
            "TargetOfCollectionType" => Ok(CoreRelationshipTypeNameName::TargetOfCollectionType),
            "ValueType" => Ok(CoreRelationshipTypeNameName::ValueType),
            "ValueTypeFor" => Ok(CoreRelationshipTypeNameName::ValueTypeFor),
            _ => Err(()),
        }
    }
}
