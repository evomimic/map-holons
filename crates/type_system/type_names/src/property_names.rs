use base_types::MapString;
use convert_case::{Case, Casing};
use integrity_core_types::PropertyName;
use strum_macros::VariantNames;

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
            PropertyName(MapString("AllowsDuplicates".to_string())),
            CorePropertyTypeName::AllowsDuplicates.as_property_name()
        );
        assert_eq!(
            PropertyName(MapString("DeletionSemantic".to_string())),
            CorePropertyTypeName::DeletionSemantic.as_property_name()
        );
        assert_eq!(
            PropertyName(MapString("Description".to_string())),
            CorePropertyTypeName::Description.as_property_name()
        );
        assert_eq!(
            PropertyName(MapString("DisplayName".to_string())),
            CorePropertyTypeName::DisplayName.as_property_name()
        );
        assert_eq!(
            PropertyName(MapString("DisplayNamePlural".to_string())),
            CorePropertyTypeName::DisplayNamePlural.as_property_name()
        );
        assert_eq!(
            PropertyName(MapString("InstanceTypeKind".to_string())),
            CorePropertyTypeName::InstanceTypeKind.as_property_name()
        );
        assert_eq!(
            PropertyName(MapString("IsAbstractType".to_string())),
            CorePropertyTypeName::IsAbstractType.as_property_name()
        );
        assert_eq!(
            PropertyName(MapString("IsDefinitional".to_string())),
            CorePropertyTypeName::IsDefinitional.as_property_name()
        );
        assert_eq!(
            PropertyName(MapString("IsOrdered".to_string())),
            CorePropertyTypeName::IsOrdered.as_property_name()
        );
        assert_eq!(
            PropertyName(MapString("IsRequired".to_string())),
            CorePropertyTypeName::IsRequired.as_property_name()
        );
        assert_eq!(
            PropertyName(MapString("MapBoolean".to_string())),
            CorePropertyTypeName::MapBoolean.as_property_name()
        );
        assert_eq!(
            PropertyName(MapString("MapBytes".to_string())),
            CorePropertyTypeName::MapBytes.as_property_name()
        );
        assert_eq!(
            PropertyName(MapString("MapInteger".to_string())),
            CorePropertyTypeName::MapInteger.as_property_name()
        );
        assert_eq!(
            PropertyName(MapString("MapString".to_string())),
            CorePropertyTypeName::MapString.as_property_name()
        );
        assert_eq!(
            PropertyName(MapString("MinCardinality".to_string())),
            CorePropertyTypeName::MinCardinality.as_property_name()
        );
        assert_eq!(
            PropertyName(MapString("MaxCardinality".to_string())),
            CorePropertyTypeName::MaxCardinality.as_property_name()
        );
        assert_eq!(
            PropertyName(MapString("SpaceName".to_string())),
            CorePropertyTypeName::SpaceName.as_property_name()
        );
        assert_eq!(
            PropertyName(MapString("TypeKind".to_string())),
            CorePropertyTypeName::TypeKind.as_property_name()
        );
        assert_eq!(
            PropertyName(MapString("TypeName".to_string())),
            CorePropertyTypeName::TypeName.as_property_name()
        );
        assert_eq!(
            PropertyName(MapString("TypeNamePlural".to_string())),
            CorePropertyTypeName::TypeNamePlural.as_property_name()
        );
    }
}
