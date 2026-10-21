//! Required assessment-anchor resolution and schema compatibility diagnostics.
use crate::{
    assessment_support::{path, targets},
    handlers::finding,
    ValidationCollector,
};
use base_types::MapString;
use core_types::{CommitValidationViolationKind, HolonError, HolonId};
use holons_core::core_shared_objects::holon::StagedState;
use holons_core::{
    core_shared_objects::transactions::TransactionContext, resolve_assessment_anchor,
    HolonReference, LineageIdentity, ReadableHolon, SmartReference, StagedReference,
};
use std::{collections::HashMap, sync::Arc};
use type_names::CoreRelationshipTypeName;

/// Missing Core vocabulary requires bootstrap/upgrade; unreadable inputs remain operational errors.
#[derive(Debug)]
pub enum AnchorResolutionError {
    /// A mandatory Core definition is absent.
    SchemaIncompatible { missing_anchor: String },
    /// Reference resolution failed for an existing or ambiguous input.
    Operational(HolonError),
}
impl std::fmt::Display for AnchorResolutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SchemaIncompatible { missing_anchor } => write!(
                f,
                "Required validation anchor {missing_anchor} is missing; \
                 bootstrap or upgrade the Core Schema before retrying."
            ),
            Self::Operational(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for AnchorResolutionError {}
impl From<HolonError> for AnchorResolutionError {
    fn from(error: HolonError) -> Self {
        Self::Operational(error)
    }
}
impl From<AnchorResolutionError> for HolonError {
    fn from(error: AnchorResolutionError) -> Self {
        match error {
            AnchorResolutionError::Operational(error) => error,
            error @ AnchorResolutionError::SchemaIncompatible { .. } => {
                HolonError::CommitFailure(error.to_string())
            }
        }
    }
}
/// Resolves assessment anchors without substituting version-producing successors.
pub fn resolve_validation_anchor(
    context: &Arc<TransactionContext>,
    key: &str,
) -> Result<HolonReference, AnchorResolutionError> {
    resolve_assessment_anchor(context, key).map_err(|error| match error {
        HolonError::HolonNotFound(_) => {
            AnchorResolutionError::SchemaIncompatible { missing_anchor: key.into() }
        }
        error => AnchorResolutionError::Operational(error),
    })
}

/// Required assessment anchors whose successor publication is temporarily unsupported.
/// Kept in sync with the interim inventory in the Commit validation implementation plan.
pub(crate) const REQUIRED_ANCHOR_KEYS: &[&str] = &[
    "TypeDescriptor",
    "HolonType.TypeDescriptor",
    "MetaTypeDescriptor.HolonType",
    "MetaHolonType.MetaTypeDescriptor",
    "PropertyType.TypeDescriptor",
    "DeclaredRelationshipType.RelationshipType",
    "ValueType.TypeDescriptor",
    "StringValueType.ValueType",
    "IntegerValueType.ValueType",
    "BooleanValueType.ValueType",
    "BytesValueType.ValueType",
    "EnumValueType.ValueType",
    "ConstraintType.HolonType",
    "StringLengthConstraint.ConstraintType",
    "BytesLengthConstraint.ConstraintType",
    "NumericRangeConstraint.ConstraintType",
    "ItemCountConstraint.ConstraintType",
    "CardinalityConstraint.ConstraintType",
    "UniqueItemsConstraint.ConstraintType",
    "Schema.HolonType",
    "Rule.HolonType",
    "HolonValidationRule.HolonType",
    "PropertyValidationRule.HolonType",
    "StringValidationRule.HolonType",
    "IntegerValidationRule.HolonType",
    "BooleanValidationRule.HolonType",
    "BytesValidationRule.HolonType",
    "EnumValueValidationRule.HolonType",
    "RequiredPropertyPresence.ValidationRule",
    "NoUndescribedProperties.ValidationRule",
    "BaseValueKindMatchesString.ValidationRule",
    "BaseValueKindMatchesInteger.ValidationRule",
    "BaseValueKindMatchesBoolean.ValidationRule",
    "BaseValueKindMatchesBytes.ValidationRule",
    "BaseValueKindMatchesEnum.ValidationRule",
    "AtMostOneDirectParent.ValidationRule",
    "AcyclicExtendsLineage.ValidationRule",
    "ExtendsLineageTerminatesAtTypeDescriptor.ValidationRule",
    "UniqueTypeDescriptorRoot.ValidationRule",
    "LocalInstanceKindAnchorDesignation.ValidationRule",
    "InstanceKindAnchorsAreAbstract.ValidationRule",
    "TypeDescriptorRootKindException.ValidationRule",
    "DescribingCategoryCompatibility.ValidationRule",
    "DescriptorMetaTypeCorrespondence.ValidationRule",
    "NoInheritedMemberRedeclaration.ValidationRule",
    "UniqueSemanticMemberNames.ValidationRule",
    "WellFormedEffectiveMemberDefinitions.ValidationRule",
    "ContractMemberKindCompatibility.ValidationRule",
    "InheritedValueConstraintNonRelaxation.ValidationRule",
    "SchemaDependenciesAcyclic.ValidationRule",
    "KeyUniqueness.ValidationRule",
];

/// Temporarily rejects required-anchor successor publication until relationship-driven
/// assessment inputs replace global readiness discovery. The source comes from
/// constructor-owned Predecessor; candidate keys never establish anchor identity.
/// Findings remain candidate-scoped and do not stop independent assessment.
pub(crate) fn assess_required_anchor_updates(
    context: &Arc<TransactionContext>,
    candidates: &[StagedReference],
    collector: &mut ValidationCollector,
) -> Result<(), HolonError> {
    let mut anchor_lineages: Option<HashMap<LineageIdentity, &'static str>> = None;
    for candidate in candidates {
        if !candidate.is_in_state(context, StagedState::ForUpdateNewVersion)? {
            continue;
        }
        let subject: HolonReference = candidate.into();
        let predecessors = targets(&subject, CoreRelationshipTypeName::Predecessor)?;
        let [HolonReference::Smart(predecessor)] = predecessors.as_slice() else {
            return Err(HolonError::InvalidState(format!(
                "Candidate {} requires one saved Predecessor before anchor assessment",
                subject.full_reference_id_string(),
            )));
        };
        // Read the saved record key, not a SmartLink's cached occurrence key.
        let source =
            SmartReference::new_from_id(context.space_read_handle(), predecessor.holon_id());
        let source_key = source.key()?;
        let direct_anchor = source_key
            .as_ref()
            .and_then(|key| REQUIRED_ANCHOR_KEYS.iter().copied().find(|anchor| *anchor == key.0));
        let anchor = if direct_anchor.is_some() {
            direct_anchor
        } else {
            let lineages = match &mut anchor_lineages {
                Some(lineages) => lineages,
                None => anchor_lineages.insert(saved_anchor_lineages(context)?),
            };
            let lineage = LineageIdentity::for_reference(&source.clone().into(), context)?;
            lineages.get(&lineage).copied()
        };
        if let Some(anchor) = anchor {
            let message = format!(
                "Candidate {} cannot publish a new version of required Core anchor {anchor} \
                 from saved source {}. Version-producing updates of this anchor are currently \
                 unsupported; retain the saved definition or abandon the successor.",
                subject.full_reference_id_string(),
                source.full_reference_id_string(),
            );
            finding(
                collector,
                CommitValidationViolationKind::RuleViolation {
                    code: "UnsupportedRequiredAnchorUpdate".into(),
                },
                None,
                &path(&subject),
                None,
                message,
            );
        }
    }
    Ok(())
}

/// Loads historical lineage evidence with one key lookup per required anchor.
/// The caller retains this map for one assessment and initializes it only when
/// a version-producing candidate's saved source key does not identify an anchor.
fn saved_anchor_lineages(
    context: &Arc<TransactionContext>,
) -> Result<HashMap<LineageIdentity, &'static str>, HolonError> {
    let mut lineages = HashMap::new();
    for &key in REQUIRED_ANCHOR_KEYS {
        for root in context.lookup().get_saved_lineage_roots_by_key(&MapString(key.into()))? {
            lineages.entry(LineageIdentity::Saved(HolonId::Local(root))).or_insert(key);
        }
    }
    Ok(lineages)
}
