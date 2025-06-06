
// Auto-generated enum from enum_template.rs
use std::str::FromStr;
use std::fmt;
use strum_macros::EnumIter;

#[derive(Debug, Clone, EnumIter, Default, PartialEq, Eq)]
pub enum CoreBooleanTypeNameName {
    #[default]
    MapBooleanType,
}

impl fmt::Display for CoreBooleanTypeNameName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreBooleanTypeNameName::MapBooleanType => write!(f, "MapBooleanType"),
        }
    }
}

impl FromStr for CoreBooleanTypeNameName {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "MapBooleanType" => Ok(CoreBooleanTypeNameName::MapBooleanType),
            _ => Err(()),
        }
    }
}
