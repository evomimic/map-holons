
// Auto-generated enum from enum_template.rs
use std::str::FromStr;
use std::fmt;
use strum_macros::EnumIter;

#[derive(Debug, Clone, EnumIter, Default, PartialEq, Eq)]
pub enum CoreHolonTypeNameName {
    #[default]
    DanceRequestType,
    DanceResponseType,
    HolonCollectionType,
    HolonSpaceType,
    HolonType,
    PropertyType,
    RelationshipType,
    SchemaType,
    TypeDescriptor,
    ValueType,
    MetaType,
}

impl fmt::Display for CoreHolonTypeNameName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreHolonTypeNameName::DanceRequestType => write!(f, "DanceRequestType"),
            CoreHolonTypeNameName::DanceResponseType => write!(f, "DanceResponseType"),
            CoreHolonTypeNameName::HolonCollectionType => write!(f, "HolonCollectionType"),
            CoreHolonTypeNameName::HolonSpaceType => write!(f, "HolonSpaceType"),
            CoreHolonTypeNameName::HolonType => write!(f, "HolonType"),
            CoreHolonTypeNameName::PropertyType => write!(f, "PropertyType"),
            CoreHolonTypeNameName::RelationshipType => write!(f, "RelationshipType"),
            CoreHolonTypeNameName::SchemaType => write!(f, "SchemaType"),
            CoreHolonTypeNameName::TypeDescriptor => write!(f, "TypeDescriptor"),
            CoreHolonTypeNameName::ValueType => write!(f, "ValueType"),
            CoreHolonTypeNameName::MetaType => write!(f, "MetaType"),
        }
    }
}

impl FromStr for CoreHolonTypeNameName {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "DanceRequestType" => Ok(CoreHolonTypeNameName::DanceRequestType),
            "DanceResponseType" => Ok(CoreHolonTypeNameName::DanceResponseType),
            "HolonCollectionType" => Ok(CoreHolonTypeNameName::HolonCollectionType),
            "HolonSpaceType" => Ok(CoreHolonTypeNameName::HolonSpaceType),
            "HolonType" => Ok(CoreHolonTypeNameName::HolonType),
            "PropertyType" => Ok(CoreHolonTypeNameName::PropertyType),
            "RelationshipType" => Ok(CoreHolonTypeNameName::RelationshipType),
            "SchemaType" => Ok(CoreHolonTypeNameName::SchemaType),
            "TypeDescriptor" => Ok(CoreHolonTypeNameName::TypeDescriptor),
            "ValueType" => Ok(CoreHolonTypeNameName::ValueType),
            "MetaType" => Ok(CoreHolonTypeNameName::MetaType),
            _ => Err(()),
        }
    }
}
