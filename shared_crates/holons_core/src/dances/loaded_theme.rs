//! Browser-independent, consolidated Theme loading result.
use crate::core_shared_objects::transactions::TransactionContext;
use crate::{HolonReference, ReadableHolon, WritableHolon};
use base_types::BaseValue;
use core_types::HolonError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use type_names::{
    CoreRelationshipTypeName, ThemePropertyTypeName, ThemeRelationshipTypeName, ThemeTypeName,
};

/// Scalar bindings travel together; references retain exact version identities separately.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeTokenBinding {
    pub token_versioned_key: String,
    pub token_name: String,
    pub presentation_value: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Complete scalar metadata transported with the exact reference identities.
pub struct ThemeData {
    pub theme_key: String,
    pub theme_versioned_key: String,
    pub meta_design_system_key: String,
    pub meta_design_system_versioned_key: String,
    pub bindings: Vec<ThemeTokenBinding>,
}
/// Request-local projection, never a second durable representation of a Theme.
#[derive(Debug)]
pub struct LoadedTheme {
    pub theme: HolonReference,
    pub meta_design_system: HolonReference,
    pub tokens: Vec<HolonReference>,
    pub data: ThemeData,
}
/// Clone local relationship targets without retaining a collection lock.
fn members(
    source: &HolonReference,
    name: ThemeRelationshipTypeName,
) -> Result<Vec<HolonReference>, HolonError> {
    Ok(source
        .related_holons(name.as_relationship_name())?
        .read()
        .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
        .get_members()
        .clone())
}
/// Require the one target needed to construct a usable projection.
fn one(
    source: &HolonReference,
    name: ThemeRelationshipTypeName,
) -> Result<HolonReference, HolonError> {
    let values = members(source, name)?;
    match values.as_slice() {
        [value] => Ok(value.clone()),
        _ => Err(HolonError::InvalidParameter(format!(
            "Expected one {name:?}, found {}",
            values.len()
        ))),
    }
}
/// Decode a required string field without applying semantic token validation.
fn string(source: &HolonReference, name: ThemePropertyTypeName) -> Result<String, HolonError> {
    match source.property_value(name.as_property_name())? {
        Some(BaseValue::StringValue(value)) => Ok(value.0),
        _ => Err(HolonError::InvalidParameter(format!("Missing or malformed {name:?}"))),
    }
}
/// Require the semantic key exposed by the loading contract.
fn key(source: &HolonReference) -> Result<String, HolonError> {
    source
        .key()?
        .map(|value| value.0)
        .ok_or_else(|| HolonError::EmptyField("Theme loading requires semantic keys".into()))
}
/// Resolve assignment values near storage. Coverage, uniqueness, vocabulary and
/// DesignTokenType validation belong to schema validation, not presentation loading.
pub fn resolve_theme(theme: HolonReference) -> Result<LoadedTheme, HolonError> {
    use ThemeRelationshipTypeName::*;
    let meta_design_system = one(&theme, ForMetaDesignSystem)?;
    let mut tokens = Vec::new();
    let mut bindings = Vec::new();
    for assignment in members(&theme, HasThemeTokenAssignment)? {
        let token = one(&assignment, ForDesignToken)?;
        bindings.push(ThemeTokenBinding {
            token_versioned_key: token.versioned_key()?.0,
            token_name: string(&token, ThemePropertyTypeName::DesignTokenName)?,
            presentation_value: string(&assignment, ThemePropertyTypeName::PresentationValue)?,
        });
        tokens.push(token);
    }
    let data = ThemeData {
        theme_key: key(&theme)?,
        theme_versioned_key: theme.versioned_key()?.0,
        meta_design_system_key: key(&meta_design_system)?,
        meta_design_system_versioned_key: meta_design_system.versioned_key()?.0,
        bindings,
    };
    Ok(LoadedTheme { theme, meta_design_system, tokens, data })
}
impl LoadedTheme {
    /// One body and one scalar payload avoid a transient holon per token binding.
    pub fn into_projection(
        self,
        context: &Arc<TransactionContext>,
    ) -> Result<HolonReference, HolonError> {
        use ThemeRelationshipTypeName::*;
        let descriptor = context
            .lookup()
            .get_saved_holon_by_key(&ThemeTypeName::LoadedTheme.descriptor_key())?;
        let mut body: HolonReference = context.mutation().new_holon(None)?.into();
        body.add_related_holons(CoreRelationshipTypeName::DescribedBy, vec![descriptor.into()])?;
        body.with_property_value(
            ThemePropertyTypeName::LoadedThemeData.as_property_name(),
            serde_json::to_string(&self.data)
                .map_err(|e| HolonError::InvalidParameter(e.to_string()))?,
        )?;
        body.add_related_holons(LoadedThemeSource.as_relationship_name(), vec![self.theme])?;
        body.add_related_holons(
            LoadedMetaDesignSystem.as_relationship_name(),
            vec![self.meta_design_system],
        )?;
        if !self.tokens.is_empty() {
            body.add_related_holons(LoadedDesignTokens.as_relationship_name(), self.tokens)?;
        }
        Ok(body)
    }
    /// Decode only the transported transient body; never fetch a source Theme or token.
    pub fn from_projection(body: &HolonReference) -> Result<Self, HolonError> {
        use ThemeRelationshipTypeName::*;
        if !matches!(body, HolonReference::Transient(_)) {
            return Err(HolonError::InvalidParameter("LoadedTheme body must be transient".into()));
        }
        let data: ThemeData =
            serde_json::from_str(&string(body, ThemePropertyTypeName::LoadedThemeData)?).map_err(
                |e| HolonError::InvalidParameter(format!("Malformed LoadedTheme data: {e}")),
            )?;
        let tokens = members(body, LoadedDesignTokens)?;
        if tokens.len() != data.bindings.len()
            || [
                &data.theme_key,
                &data.theme_versioned_key,
                &data.meta_design_system_key,
                &data.meta_design_system_versioned_key,
            ]
            .iter()
            .any(|s| s.is_empty())
            || data
                .bindings
                .iter()
                .any(|b| b.token_name.is_empty() || b.token_versioned_key.is_empty())
        {
            return Err(HolonError::InvalidParameter("Incomplete LoadedTheme projection".into()));
        }
        Ok(Self {
            theme: one(body, LoadedThemeSource)?,
            meta_design_system: one(body, LoadedMetaDesignSystem)?,
            tokens,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_test_holon};

    #[test]
    fn independently_defined_theme_preserves_identity_and_values_without_token_type_reads(
    ) -> Result<(), HolonError> {
        let context = build_context();
        let mut theme: HolonReference = new_test_holon(&context, "Independent.Theme")?.into();
        let mds: HolonReference = new_test_holon(&context, "Independent.MDS")?.into();
        let mut token: HolonReference = new_test_holon(&context, "Independent.Token")?.into();
        token.with_property_value(
            ThemePropertyTypeName::DesignTokenName.as_property_name(),
            "IndependentSpacing",
        )?;
        let mut assignment: HolonReference =
            new_test_holon(&context, "Independent.Assignment")?.into();
        assignment.with_property_value(
            ThemePropertyTypeName::PresentationValue.as_property_name(),
            "0.75rem",
        )?;
        assignment.add_related_holons(
            ThemeRelationshipTypeName::ForDesignToken.as_relationship_name(),
            vec![token.clone()],
        )?;
        theme.add_related_holons(
            ThemeRelationshipTypeName::ForMetaDesignSystem.as_relationship_name(),
            vec![mds.clone()],
        )?;
        theme.add_related_holons(
            ThemeRelationshipTypeName::HasThemeTokenAssignment.as_relationship_name(),
            vec![assignment],
        )?;
        // No DefinesDesignToken or HasDesignTokenType graph is supplied: loading is not validation.
        let loaded = resolve_theme(theme.clone())?;
        assert_eq!(loaded.theme, theme);
        assert_eq!(loaded.meta_design_system, mds);
        assert_eq!(loaded.tokens, vec![token.clone()]);
        assert_eq!(
            loaded.data.bindings,
            vec![ThemeTokenBinding {
                token_versioned_key: token.versioned_key()?.0,
                token_name: "IndependentSpacing".into(),
                presentation_value: "0.75rem".into(),
            }]
        );
        assert_eq!(context.lookup().staged_count()?, 0);
        assert!(context.is_open());
        Ok(())
    }

    #[test]
    fn missing_required_theme_data_fails_explicitly() -> Result<(), HolonError> {
        let context = build_context();
        let theme: HolonReference = new_test_holon(&context, "Broken.Theme")?.into();
        assert!(resolve_theme(theme).is_err());
        let body: HolonReference = new_test_holon(&context, "Broken.Projection")?.into();
        assert!(LoadedTheme::from_projection(&body).is_err());
        Ok(())
    }
}
