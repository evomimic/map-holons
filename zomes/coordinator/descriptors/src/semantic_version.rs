use derive_new::new;

use hdk::prelude::*;

use holons_core::core_shared_objects::{Holon, HolonError};
use integrity_core_types::PropertyName;
use base_types::{BaseValue, MapInteger, MapString};


#[derive(new, Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemanticVersion {
    major: i64,
    minor: i64,
    patch: i64,
}

impl Default for SemanticVersion {
    fn default() -> Self {
        SemanticVersion { major: 0, minor: 0, patch: 1 }
    }
}
impl SemanticVersion {
    pub fn to_string(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[allow(dead_code)]
pub fn set_semantic_version(major: i64, minor: i64, patch: i64) -> Result<Holon, HolonError> {
    // ----------------  GET A NEW (EMPTY) HOLON -------------------------------
    let mut version = Holon::new();

    // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
    version
        .with_property_value(
            PropertyName(MapString("major".to_string())),
            Some(BaseValue::IntegerValue(MapInteger(major))),
        )?
        .with_property_value(
            PropertyName(MapString("minor".to_string())),
            Some(BaseValue::IntegerValue(MapInteger(minor))),
        )?
        .with_property_value(
            PropertyName(MapString("patch".to_string())),
            Some(BaseValue::IntegerValue(MapInteger(patch))),
        )?;

    Ok(version)
}

// TODO: Implement and debug the following function
// pub fn define_semantic_version_descriptor(
//     schema: &HolonCollection,
//
// ) -> Holon {
//
//     define_type_descriptor(&(), (), TypeKind::Holon, (), (), false, false);
//     let mut descriptor = Holon::new();
//
//
//     // ----------------  USE THE INTERNAL HOLONS API TO ADD TYPE_HEADER PROPERTIES -----------------
//     descriptor.with_property_value("type_name".to_string(), BaseValue::StringValue("SemanticVersion".to_string()))
//         .with_property_value("description".to_string(), BaseValue::StringValue(
//             "Supports a structured approach to tracking changes to a chain of TypeDescriptor versions.".to_string()))
//         .with_property_value("label".to_string(), BaseValue::StringValue("Semantic Version".to_string()))
//         .with_property_value("base_type".to_string(), BaseValue::StringValue("TypeKind::Holon".to_string()))
//         .with_property_value("is_dependent".to_string(), BaseValue::BooleanValue(true));
//
//     descriptor
//
// }
