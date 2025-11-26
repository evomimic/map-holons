use base_types::MapString;
use convert_case::{Case, Casing};
use strum_macros::VariantNames;

#[derive(Debug, Clone, VariantNames)]
pub enum CoreHolonTypeName {
    Collection,
    CommitResponseType,
    Dance,
    DanceType,
    Holon,
    HolonErrorType,
    HolonLoadError,
    HolonSpace,
    HolonType,
    SchemaHolonType,
    SchemaType,
    TypeDescriptor,
}

impl CoreHolonTypeName {
    pub fn as_holon_name(&self) -> MapString {
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
            MapString("Collection".to_string()),
            CoreHolonTypeName::Collection.as_holon_name()
        );
        assert_eq!(MapString("Dance".to_string()), CoreHolonTypeName::Dance.as_holon_name());
        assert_eq!(MapString("Holon".to_string()), CoreHolonTypeName::Holon.as_holon_name());
        assert_eq!(
            MapString("HolonSpace".to_string()),
            CoreHolonTypeName::HolonSpace.as_holon_name()
        );
        assert_eq!(
            MapString("HolonType".to_string()),
            CoreHolonTypeName::HolonType.as_holon_name()
        );
        assert_eq!(
            MapString("SchemaHolonType".to_string()),
            CoreHolonTypeName::SchemaHolonType.as_holon_name()
        );
    }
}
