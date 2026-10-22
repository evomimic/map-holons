//! Assessment-local historical key associations, without version or head selection.
use crate::AssessmentEvent;
use base_types::MapString;
use core_types::{HolonError, HolonId, TemporaryId};
use holons_core::{
    core_shared_objects::{holon::StagedState, transactions::TransactionContext},
    LineageIdentity, ReadableHolon, StagedReference,
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
};

/// Immutable contextual evidence for one exact effective key in the local HolonSpace.
/// Different keyed entries for one lineage count once; committed history is never released.
#[derive(Debug)]
pub struct KeyEvidence {
    pub(crate) key: MapString,
    saved: BTreeSet<LineageIdentity>,
    committed: BTreeSet<LineageIdentity>,
    live: BTreeSet<LineageIdentity>,
}
impl KeyEvidence {
    pub(crate) fn conflicting_lineages(
        &self,
        subject: &LineageIdentity,
    ) -> BTreeSet<LineageIdentity> {
        self.saved
            .iter()
            .chain(&self.committed)
            .chain(&self.live)
            .filter(|lineage| *lineage != subject)
            .cloned()
            .collect()
    }
}

/// A subject's lineage paired with the contextual evidence for its effective key.
#[derive(Clone, Debug)]
pub struct KeyClaim {
    pub(crate) evidence: Arc<KeyEvidence>,
    pub(crate) lineage: LineageIdentity,
}

/// Prepares one bounded lookup per distinct applicable key. Abandoned candidates and
/// intermediate keys reserve nothing; committed entries supply persisted evidence on retry.
/// No-op, graph-only, and keyless subjects request no evidence or retrospective key composition.
pub(crate) fn prepare_key_evidence(
    context: &Arc<TransactionContext>,
    candidates: &[StagedReference],
    observer: &mut impl FnMut(AssessmentEvent),
) -> Result<HashMap<TemporaryId, KeyClaim>, HolonError> {
    let mut by_key: BTreeMap<MapString, Vec<(TemporaryId, LineageIdentity)>> = BTreeMap::new();
    for candidate in candidates {
        if !matches!(
            candidate.staged_state()?,
            StagedState::ForCreate | StagedState::ForUpdateNewVersion
        ) {
            continue;
        }
        let Some(key) = candidate.key()? else { continue };
        let lineage = LineageIdentity::for_reference(&candidate.into(), context)?;
        by_key.entry(key).or_default().push((candidate.temporary_id(), lineage));
    }
    if by_key.is_empty() {
        return Ok(HashMap::new());
    }
    let mut committed_by_key: BTreeMap<MapString, BTreeSet<LineageIdentity>> = BTreeMap::new();
    for candidate in context.staged_references()? {
        if !matches!(candidate.staged_state()?, StagedState::Committed(_)) {
            continue;
        }
        let Some(key) = candidate.key()? else { continue };
        if by_key.contains_key(&key) {
            // Resolve the committed record's lineage through the Reference Layer/cache.
            let lineage = LineageIdentity::for_reference(&candidate.into(), context)?;
            committed_by_key.entry(key).or_default().insert(lineage);
        }
    }
    let mut result = HashMap::new();
    for (key, claims) in by_key {
        observer(AssessmentEvent::PreparationOperation {
            operation: "saved_lineage_roots_by_key",
            role: "key-evidence",
            starting: true,
        });
        let saved = context.lookup().get_saved_lineage_roots_by_key(&key);
        observer(AssessmentEvent::PreparationOperation {
            operation: "saved_lineage_roots_by_key",
            role: "key-evidence",
            starting: false,
        });
        let saved =
            saved?.into_iter().map(|root| LineageIdentity::Saved(HolonId::Local(root))).collect();
        let evidence = Arc::new(KeyEvidence {
            committed: committed_by_key.remove(&key).unwrap_or_default(),
            live: claims.iter().map(|(_, lineage)| lineage.clone()).collect(),
            key,
            saved,
        });
        for (candidate, lineage) in claims {
            result.insert(candidate, KeyClaim { evidence: evidence.clone(), lineage });
        }
    }
    Ok(result)
}
