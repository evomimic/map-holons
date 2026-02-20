use base_types::MapString;
use convert_case::{Case, Casing};
use integrity_core_types::PropertyName;
use strum_macros::VariantNames;

pub trait ToPropertyName {
    fn to_property_name(self) -> PropertyName;
}

// --- Internal single point for canonicalization (ClassCase) ---
#[inline]
fn canonical_property_name<S: AsRef<str>>(s: S) -> PropertyName {
    PropertyName(MapString(s.as_ref().to_case(Case::UpperCamel)))
}

// --- to_property_name impls ---

impl ToPropertyName for &str {
    fn to_property_name(self) -> PropertyName {
        canonical_property_name(self) // canonicalize to ClassCase
    }
}

impl ToPropertyName for String {
    fn to_property_name(self) -> PropertyName {
        // Assume already canonical; pass through unchanged
        PropertyName(MapString(self))
    }
}

impl ToPropertyName for MapString {
    fn to_property_name(self) -> PropertyName {
        // Assume already canonical; pass through unchanged
        PropertyName(self)
    }
}

impl ToPropertyName for &MapString {
    fn to_property_name(self) -> PropertyName {
        // Assume already canonical; pass through unchanged (clone)
        PropertyName(self.clone())
    }
}

impl ToPropertyName for CorePropertyTypeName {
    fn to_property_name(self) -> PropertyName {
        self.as_property_name() // canonical via enum method
    }
}

impl ToPropertyName for &CorePropertyTypeName {
    fn to_property_name(self) -> PropertyName {
        self.clone().as_property_name() // canonical via enum method
    }
}

impl ToPropertyName for PropertyName {
    #[inline]
    fn to_property_name(self) -> PropertyName {
        self // pass-through unchanged
    }
}

impl ToPropertyName for &PropertyName {
    #[inline]
    fn to_property_name(self) -> PropertyName {
        self.clone() // pass-through unchanged
    }
}

#[derive(Debug, Clone, VariantNames)]
pub enum CorePropertyTypeName {
    AllowsDuplicates,
    CommitRequestStatus,
    CommitsAttempted,
    DanceSummary,
    DeletionSemantic,
    Description,
    DisplayName,
    DisplayNamePlural,
    ErrorCount,
    ErrorMessage,
    ErrorType,
    Filename,
    HolonKey,
    HolonId,
    HolonsStaged,
    InstanceTypeKind,
    IsAbstractType,
    IsDeclared,
    IsDefinitional,
    IsOrdered,
    IsRequired,
    Key,
    LinksCreated,
    LoadCommitStatus,
    LoaderHolonKey,
    MapBoolean,
    MapBytes,
    MapInteger,
    MapString,
    MaxCardinality,
    MinCardinality,
    ProxyKey,
    ProxyId,
    RelationshipName,
    ResponseStatusCode,
    SpaceName,
    StartUtf8ByteOffset,
    TotalBundles,
    TotalLoaderHolons,
    Type,
    TypeKind,
    TypeName,
    TypeNamePlural,
    HolonsCommitted,
}

impl CorePropertyTypeName {
    /// Canonical property name in ClassCase (UpperCamel).
    pub fn as_property_name(&self) -> PropertyName {
        let pascal = format!("{self:?}").to_case(Case::UpperCamel);
        PropertyName(MapString(pascal))
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
            PropertyName(MapString("Description".to_string())),
            CorePropertyTypeName::Description.as_property_name()
        );
        assert_eq!(
            PropertyName(MapString("TypeNamePlural".to_string())),
            CorePropertyTypeName::TypeNamePlural.as_property_name()
        );
    }

    #[test]
    fn test_to_property_name_str_and_string() {
        assert_eq!(
            PropertyName(MapString("InstanceTypeKind".to_string())),
            "instance_type_kind".to_property_name() // canonicalized
        );
        assert_eq!(
            PropertyName(MapString("AlreadyCanonical".to_string())),
            String::from("AlreadyCanonical").to_property_name() // pass-through
        );
    }
}
