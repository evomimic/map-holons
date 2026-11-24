//! Descriptor-authored composition vocabulary, independent of executable readiness.
use crate::{HolonReference, ReadableHolon};
use base_types::BaseValue;
use core_types::HolonError;
use type_names::{DahnPropertyTypeName, ToPropertyName};

macro_rules! vocabulary {
    ($name:ident, $property:ident, [$($member:ident),+]) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        pub enum $name { $($member),+ }
        impl $name {
            pub fn read(slot: &HolonReference) -> Result<Self, HolonError> {
                let name = DahnPropertyTypeName::$property.to_property_name();
                match slot.property_value(name.clone())? {
                    Some(BaseValue::EnumValue(value)) => match value.to_string().as_str() {
                        $(stringify!($member) => Ok(Self::$member),)+
                        other => Err(HolonError::InvalidParameter(format!("Unknown {name} variant: {other}"))),
                    },
                    _ => Err(HolonError::InvalidParameter(format!("Missing or non-enum {name}"))),
                }
            }
        }
    }
}
vocabulary!(CompositionActivation, CompositionActivation, [Presentation, Demand]);
vocabulary!(
    CompositionSubject,
    CompositionSubject,
    [OwnerSubject, ScalarPropertyLabel, ScalarPropertyValue, AffordedAction, SelectedCollection]
);
