use base_types::MapString;
use convert_case::{Case, Casing};
use strum_macros::VariantNames;

/// A strongly-typed wrapper around the shared descriptor `type_name` for operator types.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OperatorName(pub MapString);

/// Converts common operator-name inputs into a typed `OperatorName`.
pub trait ToOperatorName {
    /// Returns the typed operator name represented by this value.
    fn to_operator_name(self) -> OperatorName;
}

// --- Internal single point for canonicalization (ClassCase) ---
#[inline]
fn canonical_operator_name<S: AsRef<str>>(operator_name: S) -> OperatorName {
    OperatorName(MapString(operator_name.as_ref().to_case(Case::UpperCamel)))
}

// --- to_operator_name impls ---

impl ToOperatorName for &str {
    fn to_operator_name(self) -> OperatorName {
        canonical_operator_name(self)
    }
}

impl ToOperatorName for String {
    fn to_operator_name(self) -> OperatorName {
        OperatorName(MapString(self))
    }
}

impl ToOperatorName for MapString {
    fn to_operator_name(self) -> OperatorName {
        OperatorName(self)
    }
}

impl ToOperatorName for &MapString {
    fn to_operator_name(self) -> OperatorName {
        OperatorName(self.clone())
    }
}

impl ToOperatorName for CoreOperatorTypeName {
    fn to_operator_name(self) -> OperatorName {
        self.as_operator_name()
    }
}

impl ToOperatorName for &CoreOperatorTypeName {
    fn to_operator_name(self) -> OperatorName {
        self.clone().as_operator_name()
    }
}

impl ToOperatorName for OperatorName {
    fn to_operator_name(self) -> OperatorName {
        self
    }
}

impl ToOperatorName for &OperatorName {
    fn to_operator_name(self) -> OperatorName {
        self.clone()
    }
}

/// Stable MAP Core operator type names backed by concrete `OperatorType` descriptors.
#[derive(Debug, Clone, PartialEq, Eq, VariantNames)]
pub enum CoreOperatorTypeName {
    EqualsOperator,
    LessThanOperator,
}

impl CoreOperatorTypeName {
    /// Canonical operator type name in ClassCase (UpperCamel).
    pub fn as_operator_name(&self) -> OperatorName {
        // Use the Rust variant identifier as the core inventory source, then apply
        // the shared type-name case convention used by the neighboring modules.
        let operator_name = format!("{self:?}").to_case(Case::UpperCamel);
        OperatorName(MapString(operator_name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::VariantNames as _;

    fn expected_operator_name(operator_name: &str) -> OperatorName {
        OperatorName(MapString(operator_name.to_string()))
    }

    fn all_core_operator_type_names() -> Vec<CoreOperatorTypeName> {
        vec![CoreOperatorTypeName::EqualsOperator, CoreOperatorTypeName::LessThanOperator]
    }

    #[test]
    fn test_variant_string_conversion() {
        let variants = all_core_operator_type_names();
        assert_eq!(CoreOperatorTypeName::VARIANTS.len(), variants.len());

        for (variant_name, core_operator_type_name) in
            CoreOperatorTypeName::VARIANTS.iter().zip(variants)
        {
            assert_eq!(
                expected_operator_name(variant_name),
                core_operator_type_name.as_operator_name()
            );
        }
    }

    #[test]
    fn test_to_operator_name_accepts_common_input_shapes() {
        assert_eq!(expected_operator_name("EqualsOperator"), "equals_operator".to_operator_name());
        assert_eq!(
            expected_operator_name("AlreadyCanonical"),
            String::from("AlreadyCanonical").to_operator_name()
        );

        let map_string = MapString("LessThanOperator".to_string());
        assert_eq!(
            expected_operator_name("LessThanOperator"),
            map_string.clone().to_operator_name()
        );
        assert_eq!(expected_operator_name("LessThanOperator"), (&map_string).to_operator_name());

        let operator_name = expected_operator_name("EqualsOperator");
        assert_eq!(
            expected_operator_name("EqualsOperator"),
            operator_name.clone().to_operator_name()
        );
        assert_eq!(expected_operator_name("EqualsOperator"), (&operator_name).to_operator_name());

        let core_operator_type_name = CoreOperatorTypeName::LessThanOperator;
        assert_eq!(
            expected_operator_name("LessThanOperator"),
            core_operator_type_name.clone().to_operator_name()
        );
        assert_eq!(
            expected_operator_name("LessThanOperator"),
            (&core_operator_type_name).to_operator_name()
        );
    }
}
