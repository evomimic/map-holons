//! Well-known identities for the Theme loading contract.
use base_types::MapString;
use integrity_core_types::{PropertyName, RelationshipName};

#[derive(Debug, Clone, Copy)]
/// Canonical Dance and projection descriptor identities owned by the Theme package.
pub enum ThemeTypeName {
    LoadTheme,
    LoadedTheme,
}
impl ThemeTypeName {
    /// Semantic name without a descriptor-family suffix.
    pub fn name(self) -> MapString {
        format!("{self:?}").into()
    }
    /// Exact schema key used by ordinary descriptor lookup.
    pub fn descriptor_key(self) -> MapString {
        match self {
            Self::LoadTheme => "LoadTheme.DanceType",
            Self::LoadedTheme => "LoadedTheme.Projection",
        }
        .into()
    }
}
#[derive(Debug, Clone, Copy)]
/// Scalar members read while constructing or decoding a loaded Theme.
pub enum ThemePropertyTypeName {
    LoadedThemeData,
    DesignTokenName,
    PresentationValue,
}
impl ThemePropertyTypeName {
    /// Canonical relationship-independent property identity.
    pub fn as_property_name(self) -> PropertyName {
        PropertyName(format!("{self:?}").into())
    }
}
#[derive(Debug, Clone, Copy)]
/// Theme graph navigation and transient projection relationships.
pub enum ThemeRelationshipTypeName {
    ForMetaDesignSystem,
    HasThemeTokenAssignment,
    ForDesignToken,
    LoadedThemeSource,
    LoadedMetaDesignSystem,
    LoadedDesignTokens,
}
impl ThemeRelationshipTypeName {
    /// Canonical relationship identity.
    pub fn as_relationship_name(self) -> RelationshipName {
        RelationshipName(format!("{self:?}").into())
    }
}
