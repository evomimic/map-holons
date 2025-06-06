
// Auto-generated enum from enum_template.rs
use std::str::FromStr;
use std::fmt;
use strum_macros::EnumIter;

#[derive(Debug, Clone, EnumIter, Default, PartialEq, Eq)]
pub enum CoreStringTypeNameName {
    #[default]
    MapStringType,
    PropertyNameType,
    RelationshipNameType,
    SemanticVersionType,
}

impl fmt::Display for CoreStringTypeNameName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreStringTypeNameName::MapStringType => write!(f, "MapStringType"),
            CoreStringTypeNameName::PropertyNameType => write!(f, "PropertyNameType"),
            CoreStringTypeNameName::RelationshipNameType => write!(f, "RelationshipNameType"),
            CoreStringTypeNameName::SemanticVersionType => write!(f, "SemanticVersionType"),
        }
    }
}

impl FromStr for CoreStringTypeNameName {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "MapStringType" => Ok(CoreStringTypeNameName::MapStringType),
            "PropertyNameType" => Ok(CoreStringTypeNameName::PropertyNameType),
            "RelationshipNameType" => Ok(CoreStringTypeNameName::RelationshipNameType),
            "SemanticVersionType" => Ok(CoreStringTypeNameName::SemanticVersionType),
            _ => Err(()),
        }
    }
}
