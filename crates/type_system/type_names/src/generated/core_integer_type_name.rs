
// Auto-generated enum from enum_template.rs
use std::str::FromStr;
use std::fmt;
use strum_macros::EnumIter;

#[derive(Debug, Clone, EnumIter, Default, PartialEq, Eq)]
pub enum CoreIntegerTypeNameName {
    #[default]
    MapIntegerType,
}

impl fmt::Display for CoreIntegerTypeNameName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreIntegerTypeNameName::MapIntegerType => write!(f, "MapIntegerType"),
        }
    }
}

impl FromStr for CoreIntegerTypeNameName {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "MapIntegerType" => Ok(CoreIntegerTypeNameName::MapIntegerType),
            _ => Err(()),
        }
    }
}
