use base_types::BaseValue;
use crate::property_name::PropertyName;
use std::collections::BTreeMap;


// ===============================
// 📦 Type Aliases
// ===============================
// This file will also define the type alias for the RelationshipMap
// and the RelationshipValue, which are not yet defined.

/// The type of a property’s value at runtime.
pub type PropertyValue = BaseValue;

/// The map from property names to optional property values.
pub type PropertyMap = BTreeMap<PropertyName, Option<PropertyValue>>;
