use base_types::MapString;
use convert_case::{Case, Casing};
use integrity_core_types::PropertyName;
use strum_macros::VariantNames;

pub trait ToPropertyName {
    fn to_property_name(self) -> PropertyName;
}

impl ToPropertyName for &str {
    fn to_property_name(self) -> PropertyName {
        PropertyName(MapString(self.to_string()))
    }
}

impl ToPropertyName for String {
    fn to_property_name(self) -> PropertyName {
        PropertyName(MapString(self))
    }
}

impl ToPropertyName for MapString {
    fn to_property_name(self) -> PropertyName {
        let snake_case = format!("{self:?}").to_case(Case::Snake);
        PropertyName(MapString(snake_case))
    }
}

impl ToPropertyName for &MapString {
    fn to_property_name(self) -> PropertyName {
        let snake_case = format!("{self:?}").to_case(Case::Snake);
        PropertyName(MapString(snake_case))
    }
}

impl ToPropertyName for CorePropertyTypeName {
    fn to_property_name(self) -> PropertyName {
        self.as_property_name()
    }
}

impl ToPropertyName for &CorePropertyTypeName {
    fn to_property_name(self) -> PropertyName {
        self.clone().as_property_name()
    }
}

impl ToPropertyName for PropertyName {
    #[inline]
    fn to_property_name(self) -> PropertyName {
        self
    }
}

impl ToPropertyName for &PropertyName {
    #[inline]
    fn to_property_name(self) -> PropertyName {
        self.clone()
    }
}

#[derive(Debug, Clone, VariantNames)]
pub enum CorePropertyTypeName {
    AllowsDuplicates,
    DeletionSemantic,
    Description,
    DisplayName,
    DisplayNamePlural,
    InstanceTypeKind,
    IsAbstractType,
    IsDefinitional,
    IsOrdered,
    IsRequired,
    MapBoolean,
    MapBytes,
    MapInteger,
    MapString,
    MaxCardinality,
    MinCardinality,
    SpaceName,
    TypeKind,
    TypeName,
    TypeNamePlural,
}

impl CorePropertyTypeName {
    pub fn as_property_name(&self) -> PropertyName {
        let snake_case = format!("{self:?}").to_case(Case::Snake);
        PropertyName(MapString(snake_case))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variant_string_conversion() {
        assert_eq!(
            PropertyName(MapString("allows_duplicates".to_string())),
            CorePropertyTypeName::AllowsDuplicates.as_property_name()
        );
        assert_eq!(
            PropertyName(MapString("description".to_string())),
            CorePropertyTypeName::Description.as_property_name()
        );
        assert_eq!(
            PropertyName(MapString("type_name_plural".to_string())),
            CorePropertyTypeName::TypeNamePlural.as_property_name()
        );
    }
}
