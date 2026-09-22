//! Explicit content selection for descriptor assessment; ordinary reference reads are unchanged.
use crate::core_shared_objects::transactions::TransactionContext;
use crate::{HolonReference, ProspectiveIdentity, StagedReference};
use core_types::{HolonError, HolonId};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

/// Whether a definition has authoritative content in this Commit attempt.
#[derive(Clone, Debug)]
pub enum ProspectiveSelection {
    /// No live replacement. Saved content supplies updates; creates retain their own content.
    Saved,
    /// The single live staged replacement supplies content.
    Replaced(HolonReference),
    /// No candidate may be selected, even when their content is identical.
    Contested(Arc<[StagedReference]>),
}

/// A blocked content read is distinct from an operational failure and from identity resolution.
#[derive(Clone, Debug)]
pub enum AssessmentReadError {
    /// The caller must emit an UnresolvedLocalDependency finding for the dependent check.
    Contested { source: HolonId, candidates: Arc<[StagedReference]> },
    /// The loaded schema lacks an anchor required by the active validation cohort.
    SchemaIncompatible { missing_anchor: String },
    /// Assessment cannot reliably finish; preserve prior outcomes.
    Operational(HolonError),
}
impl From<HolonError> for AssessmentReadError {
    fn from(error: HolonError) -> Self {
        Self::Operational(error)
    }
}

/// Read selection used by the single descriptor traversal implementation.
/// Implementations select references only; they do not maintain a second semantic graph.
pub trait DescriptorReader {
    /// Keeps content contention separate from operational read failures.
    type Error: From<HolonError>;
    /// Selects authoritative content without changing the supplied reference.
    fn select(&self, reference: &HolonReference) -> Result<HolonReference, Self::Error>;
    /// Exposes runtime errors for structural diagnosis; contention has no runtime error.
    fn operational_error(error: &Self::Error) -> Option<&HolonError>;
}

/// Existing reference semantics outside prospective Commit assessment.
#[derive(Clone, Copy, Debug)]
pub struct CurrentDescriptorReader;
impl DescriptorReader for CurrentDescriptorReader {
    type Error = HolonError;
    fn select(&self, reference: &HolonReference) -> Result<HolonReference, HolonError> {
        Ok(reference.clone())
    }
    fn operational_error(error: &HolonError) -> Option<&HolonError> {
        Some(error)
    }
}

/// One read-only replacement index, discarded before mutation or another Commit attempt.
/// Construct before ownership discovery from the complete Nursery workset. This does not
/// stage schemas, change saved references, or define a persistence plan.
#[derive(Clone, Debug)]
pub struct ProspectiveDescriptorReader {
    context: Arc<TransactionContext>,
    replacements: HashMap<HolonId, Arc<[StagedReference]>>,
}
impl ProspectiveDescriptorReader {
    /// Groups explicit update ancestry in O(n); committed/abandoned entries are excluded.
    /// Repeated handles do not create competitors. No semantic content is read here.
    pub fn new(
        context: &Arc<TransactionContext>,
        candidates: &[StagedReference],
    ) -> Result<Self, HolonError> {
        let mut grouped: HashMap<HolonId, Vec<StagedReference>> = HashMap::new();
        let mut seen = HashSet::new();
        for candidate in candidates {
            let reference = HolonReference::from(candidate);
            let identity = ProspectiveIdentity::for_reference(&reference, context)?;
            if !candidate.is_live_validation_candidate()? || !seen.insert(candidate.temporary_id())
            {
                continue;
            }
            if let ProspectiveIdentity::Saved(source) = identity {
                grouped.entry(source).or_default().push(candidate.clone());
            }
        }
        Ok(Self {
            context: Arc::clone(context),
            replacements: grouped.into_iter().map(|(id, members)| (id, members.into())).collect(),
        })
    }

    /// Identity resolution succeeds even when content selection is contested.
    pub fn selection(
        &self,
        reference: &HolonReference,
    ) -> Result<ProspectiveSelection, HolonError> {
        let identity = ProspectiveIdentity::for_reference(reference, &self.context)?;
        let ProspectiveIdentity::Saved(source) = identity else {
            return Ok(ProspectiveSelection::Saved);
        };
        Ok(match self.replacements.get(&source).map(AsRef::as_ref) {
            None | Some([]) => ProspectiveSelection::Saved,
            Some([single]) => ProspectiveSelection::Replaced(single.into()),
            Some(_) => ProspectiveSelection::Contested(Arc::clone(&self.replacements[&source])),
        })
    }

    /// Returns competing groups; consumers must order diagnostics explicitly.
    pub fn contested_groups(&self) -> impl Iterator<Item = (&HolonId, &[StagedReference])> {
        self.replacements
            .iter()
            .filter(|(_, members)| members.len() > 1)
            .map(|(source, members)| (source, members.as_ref()))
    }
}
impl DescriptorReader for ProspectiveDescriptorReader {
    type Error = AssessmentReadError;
    fn select(&self, reference: &HolonReference) -> Result<HolonReference, Self::Error> {
        let identity = ProspectiveIdentity::for_reference(reference, &self.context)?;
        if let ProspectiveIdentity::Saved(source) = identity {
            match self.replacements.get(&source).map(AsRef::as_ref) {
                Some([single]) => return Ok(single.into()),
                Some(candidates) if candidates.len() > 1 => {
                    return Err(AssessmentReadError::Contested {
                        source: source.clone(),
                        candidates: Arc::clone(&self.replacements[&source]),
                    })
                }
                _ => {
                    // A finished update is not authoritative prospective content. Its saved
                    // source supplies content when this attempt has no live replacement.
                    if matches!(reference, HolonReference::Staged(_)) {
                        return Ok(HolonReference::smart_from_id(
                            self.context.space_read_handle(),
                            source,
                        ));
                    }
                }
            }
        }
        Ok(reference.clone())
    }

    fn operational_error(error: &Self::Error) -> Option<&HolonError> {
        match error {
            AssessmentReadError::Operational(error) => Some(error),
            AssessmentReadError::Contested { .. }
            | AssessmentReadError::SchemaIncompatible { .. } => None,
        }
    }
}

impl<R: DescriptorReader> DescriptorReader for &R {
    type Error = R::Error;
    fn select(&self, reference: &HolonReference) -> Result<HolonReference, Self::Error> {
        (**self).select(reference)
    }
    fn operational_error(error: &Self::Error) -> Option<&HolonError> {
        R::operational_error(error)
    }
}

impl std::fmt::Display for AssessmentReadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SchemaIncompatible { missing_anchor } => write!(formatter, "Required Core validation anchor {missing_anchor} is missing; bootstrap or upgrade the schema before ordinary Commit."),
            Self::Contested { source, candidates } => write!(formatter, "Saved source {source} has {} live staged replacements; dependent assessment is blocked.", candidates.len()),
            Self::Operational(error) => std::fmt::Display::fmt(error, formatter),
        }
    }
}
