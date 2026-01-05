use base_types::MapString;
use convert_case::{Case, Casing};
use strum_macros::VariantNames;

#[derive(Debug, Clone, VariantNames)]
pub enum CoreValueTypeName {
    MapBytesValueType,
    MapEnumValueType,
    MapValueArrayType,
    ValueBoolean,
    ValueBytes,
    ValueEnum,
    ValueInteger,
    ValueString,
    ValueArrayBoolean,
    ValueArrayBytes,
    ValueArrayEnum,
    ValueArrayInteger,
    ValueArrayString,
}

impl CoreValueTypeName {
    pub fn as_value_name(&self) -> MapString {
        let class_case = format!("{self:?}").to_case(Case::Pascal);
        MapString(class_case)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variant_string_conversion() {
        assert_eq!(
            MapString("MapBytesValueType".to_string()),
            CoreValueTypeName::MapBytesValueType.as_value_name()
        );
        assert_eq!(
            MapString("MapEnumValueType".to_string()),
            CoreValueTypeName::MapEnumValueType.as_value_name()
        );
        assert_eq!(
            MapString("MapValueArrayType".to_string()),
            CoreValueTypeName::MapValueArrayType.as_value_name()
        );
        assert_eq!(
            MapString("ValueBoolean".to_string()),
            CoreValueTypeName::ValueBoolean.as_value_name()
        );
        assert_eq!(
            MapString("ValueBytes".to_string()),
            CoreValueTypeName::ValueBytes.as_value_name()
        );
        assert_eq!(
            MapString("ValueEnum".to_string()),
            CoreValueTypeName::ValueEnum.as_value_name()
        );
        assert_eq!(
            MapString("ValueInteger".to_string()),
            CoreValueTypeName::ValueInteger.as_value_name()
        );
        assert_eq!(
            MapString("ValueString".to_string()),
            CoreValueTypeName::ValueString.as_value_name()
        );
        assert_eq!(
            MapString("ValueArrayBoolean".to_string()),
            CoreValueTypeName::ValueArrayBoolean.as_value_name()
        );
        assert_eq!(
            MapString("ValueArrayBytes".to_string()),
            CoreValueTypeName::ValueArrayBytes.as_value_name()
        );
        assert_eq!(
            MapString("ValueArrayEnum".to_string()),
            CoreValueTypeName::ValueArrayEnum.as_value_name()
        );
        assert_eq!(
            MapString("ValueArrayInteger".to_string()),
            CoreValueTypeName::ValueArrayInteger.as_value_name()
        );
        assert_eq!(
            MapString("ValueArrayString".to_string()),
            CoreValueTypeName::ValueArrayString.as_value_name()
        );
    }
}
