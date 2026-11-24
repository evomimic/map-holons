//! Session-owned, exact-version evaluation recipes, independent of descriptor residency.
//!
//! Recipes contain saved identities and scalar facts, never bound transaction handles.
//! The private transaction is solely a transport carrier; realization receives freshly
//! bound projections in its own transaction. Retirement of descriptor projections does
//! not reconsider an established choice. Only an explicit agent request removes a scope.
use super::composition::{
    CompositionData, CompositionRequest, CompositionWork, ResolvedComposition,
};
use crate::core_shared_objects::{
    space_manager::HolonSpaceManager,
    transactions::{TransactionContext, TxId},
    HolonPool,
};
use crate::descriptors::Descriptor;
use crate::{HolonReference, ReadableHolon, SmartReference, TransientReference, WritableHolon};
use core_types::{HolonError, HolonId, TemporaryId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};

/// A demand on an exact owner and Theme for one subject type, independent of occurrence.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct EvaluationScope {
    owner: HolonId,
    subject_type: HolonId,
    /// Descriptor-family occurrences can require a ValueType or an exact Dance identity.
    #[serde(default)]
    subject_semantics: Option<HolonId>,
    /// Some roots are also semantic dependencies and cannot be rebound by type alone.
    #[serde(default)]
    occurrence: Option<HolonId>,
    theme: HolonId,
    slot: Option<HolonId>,
    collection: Option<HolonId>,
}
impl EvaluationScope {
    fn capture(request: &CompositionRequest) -> Result<Option<Self>, HolonError> {
        let subject_type = request.subject.holon_descriptor()?.holon().clone();
        let (Some(owner), Some(subject_type), Some(theme)) =
            (saved(&request.owner), saved(&subject_type), saved(&request.theme))
        else {
            return Ok(None);
        };
        if request.demand_slot.as_ref().is_some_and(|r| saved(r).is_none())
            || request.collection_source.as_ref().is_some_and(|r| saved(r).is_none())
        {
            return Ok(None);
        }
        let mut subject_semantics = None;
        if let Some(slot) = &request.demand_slot {
            if super::CompositionSubject::read(slot)? == super::CompositionSubject::OwnerSubject {
                let kind = super::composition::kind(slot)?;
                if matches!(
                    kind,
                    super::contract::VisualizerKind::Value
                        | super::contract::VisualizerKind::Property
                        | super::contract::VisualizerKind::Action
                ) {
                    let descriptor =
                        super::usage_subject_type(&super::contract::VisualizerSelectionRequest {
                            subject: request.subject.clone(),
                            owner: super::contract::VisualizerOwner::Visualizer(
                                request.owner.clone(),
                            ),
                            slot: slot.clone(),
                            theme: request.theme.clone(),
                            requested_kind: kind,
                        })?;
                    let Some(id) = saved(&descriptor) else { return Ok(None) };
                    subject_semantics = Some(id);
                }
            }
        }
        Ok(Some(Self {
            owner,
            subject_type,
            subject_semantics,
            occurrence: None,
            theme,
            slot: request.demand_slot.as_ref().and_then(saved),
            collection: request.collection_source.as_ref().and_then(saved),
        }))
    }
}
/// A reference recipe distinguishes the varying occurrence from immutable dependencies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum EvaluationReference {
    Subject,
    Saved(HolonId),
}
/// Captured hierarchy and contributing facts, sufficient to reconstruct without graph selection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Evaluation {
    /// The scope's establishing round survives later reconsideration of other scopes.
    established_revision: u64,
    scope: EvaluationScope,
    references: Vec<EvaluationReference>,
    data: CompositionData,
}
/// A revision prevents an in-flight evaluation from publishing after agent reconsideration.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct Decisions {
    revision: u64,
    evaluations: Vec<Evaluation>,
    #[serde(default)]
    reconsidered_scopes: Vec<EvaluationScope>,
}
#[derive(Default)]
struct State {
    decisions: Decisions,
    index: HashMap<EvaluationScope, usize>,
    owner: Option<Arc<TransactionContext>>,
    root: Option<TemporaryId>,
}
impl State {
    fn reindex(&mut self) {
        self.index = self
            .decisions
            .evaluations
            .iter()
            .enumerate()
            .map(|(index, evaluation)| (evaluation.scope.clone(), index))
            .collect();
    }
}
/// Typed registry participant with its own restricted carrier and retained decisions.
///
/// Its lifetime is the runtime space, rather than a public transaction or descriptor
/// cache Session. No TTL, publication watcher, or automatic staleness policy runs here.
pub struct VisualizerEvaluationRegistry {
    space: Weak<HolonSpaceManager>,
    state: Mutex<State>,
}
fn lock_error<T>(error: std::sync::PoisonError<T>) -> HolonError {
    HolonError::FailedToAcquireLock(error.to_string())
}
fn saved(reference: &HolonReference) -> Option<HolonId> {
    match reference {
        HolonReference::Smart(r) => Some(r.holon_id()),
        _ => None,
    }
}
impl VisualizerEvaluationRegistry {
    /// Attachments are weak; the runtime (or guest request) owns the participant.
    pub fn new(space: &Arc<HolonSpaceManager>) -> Self {
        Self { space: Arc::downgrade(space), state: Mutex::new(State::default()) }
    }
    /// Reconstruct a captured answer using the current occurrence and space capability.
    pub fn acquire(
        &self,
        context: &Arc<TransactionContext>,
        request: &CompositionRequest,
    ) -> Result<Option<ResolvedComposition>, HolonError> {
        let space = self
            .space
            .upgrade()
            .ok_or_else(|| HolonError::ServiceNotAvailable("Evaluation space".into()))?;
        if !Arc::ptr_eq(&space, &context.space_manager()) {
            return Err(HolonError::InvalidParameter(
                "Evaluation belongs to a different runtime space".into(),
            ));
        }
        let Some(scope) = EvaluationScope::capture(request)? else { return Ok(None) };
        let state = self.state.lock().map_err(lock_error)?;
        let mut occurrence_scope = scope.clone();
        occurrence_scope.occurrence = saved(&request.subject);
        let exact = state
            .index
            .get(&scope)
            .or_else(|| state.index.get(&occurrence_scope))
            .map(|index| &state.decisions.evaluations[*index]);
        let covered = if exact.is_none()
            && scope.slot.is_some()
            && !state.decisions.reconsidered_scopes.contains(&scope)
        {
            state.decisions.evaluations.iter().find(|e| {
                e.scope.owner == scope.owner && e.scope.subject_type == scope.subject_type
                    && e.scope.theme == scope.theme
                    && e.scope.occurrence.as_ref().is_none_or(|id| saved(&request.subject).as_ref() == Some(id))
                    && e.scope.slot.is_none()
                    && e.data.bindings.iter().any(|b| b.parent.is_none()
                        && matches!(&e.references[b.slot], EvaluationReference::Saved(id) if Some(id) == scope.slot.as_ref())
                                && scope.subject_semantics.as_ref().is_none_or(|descriptor| matches!(&e.references[b.subject_descriptor], EvaluationReference::Saved(id) if id == descriptor))
                        && scope.collection.as_ref().is_none_or(|source| matches!(&e.references[b.subject], EvaluationReference::Saved(id) if id == source)))
            })
        } else {
            None
        };
        let Some(evaluation) = exact.or(covered) else { return Ok(None) };
        let references = evaluation
            .references
            .iter()
            .enumerate()
            .map(|(index, recipe)| {
                Ok(match recipe {
                    EvaluationReference::Subject => request.subject.clone(),
                    EvaluationReference::Saved(id) => {
                        if let Some(metadata) =
                            evaluation.data.metadata.iter().find(|m| m.reference == index)
                        {
                            let mut properties: core_types::PropertyMap = serde_json::from_value(
                                metadata.properties.clone(),
                            )
                            .map_err(|e| {
                                HolonError::InvalidParameter(format!(
                                    "Captured composition metadata: {e}"
                                ))
                            })?;
                            if let Some(key) = &metadata.key {
                                properties.insert(
                                    core_types::PropertyName("Key".into()),
                                    base_types::BaseValue::StringValue(key.clone().into()),
                                );
                            }
                            SmartReference::new_with_properties(
                                context.space_read_handle(),
                                id.clone(),
                                properties,
                            )
                            .into()
                        } else {
                            SmartReference::new_from_id(context.space_read_handle(), id.clone())
                                .into()
                        }
                    }
                })
            })
            .collect::<Result<Vec<_>, HolonError>>()?;
        let mut data = evaluation.data.clone();
        if covered.is_some() {
            // Extract only the requested root Slot and its already-resolved descendants.
            let mut parents = vec![None; data.bindings.len()];
            let mut bindings = Vec::new();
            for (index, mut binding) in data.bindings.into_iter().enumerate() {
                let included = match binding.parent {
                    Some(parent) => parents[parent].is_some(),
                    None => {
                        matches!(&evaluation.references[binding.slot], EvaluationReference::Saved(id) if Some(id) == scope.slot.as_ref())
                    }
                };
                if included {
                    binding.parent = binding.parent.and_then(|parent| parents[parent]);
                    parents[index] = Some(bindings.len());
                    bindings.push(binding);
                }
            }
            data.bindings = bindings;
        }
        data.work = CompositionWork {
            binding_reuses: data.bindings.len(),
            metadata_reuses: data.metadata.len(),
            ..Default::default()
        };
        Ok(Some(ResolvedComposition { references, data }))
    }
    /// Capture a publication ticket before doing semantic work outside the registry lock.
    pub fn revision(&self) -> Result<u64, HolonError> {
        Ok(self.state.lock().map_err(lock_error)?.decisions.revision)
    }

    /// Reject a projection prepared while the agent superseded this evaluation.
    pub fn check_revision(&self, revision: u64) -> Result<(), HolonError> {
        if self.revision()? != revision {
            return Err(HolonError::InvalidState(
                "Visualizer evaluation superseded by agent reconsideration".into(),
            ));
        }
        Ok(())
    }

    /// Capture only saved dependencies and a safely replaceable occurrence identity.
    /// Transient declarations or personal edits are evaluated normally and not retained.
    pub fn retain(
        &self,
        request: &CompositionRequest,
        result: &ResolvedComposition,
        revision: u64,
    ) -> Result<bool, HolonError> {
        result.validate()?;
        let Some(mut scope) = EvaluationScope::capture(request)? else { return Ok(false) };
        let subject_index = result.references.iter().position(|r| r == &request.subject);
        let occurrence_only = subject_index.is_some_and(|i| {
            result.data.metadata.iter().any(|m| m.reference == i)
                || result.data.theme == Some(i)
                || result.data.populations.iter().any(|p| p.source == i || p.members.contains(&i))
                || result.data.bindings.iter().any(|b| {
                    [b.owner, b.slot, b.subject_descriptor, b.selected, b.implementation]
                        .contains(&i)
                        || b.usage == Some(i)
                        || b.governing_record == Some(i)
                        || b.subject == i
                            && b.subject_binding == super::CompositionSubject::OwnerSubject
                            && saved(&result.references[b.subject_descriptor]).as_ref()
                                != Some(
                                    scope.subject_semantics.as_ref().unwrap_or(&scope.subject_type),
                                )
                })
        });
        if occurrence_only {
            let Some(subject) = saved(&request.subject) else { return Ok(false) };
            scope.occurrence = Some(subject);
        }
        let references: Option<Vec<_>> = result
            .references
            .iter()
            .enumerate()
            .map(|(i, r)| {
                if Some(i) == subject_index && !occurrence_only {
                    Some(EvaluationReference::Subject)
                } else {
                    saved(r).map(EvaluationReference::Saved)
                }
            })
            .collect();
        let Some(references) = references else { return Ok(false) };
        let mut data = result.data.clone();
        data.reusable_across_rounds = true;
        let evaluation = Evaluation { established_revision: revision, scope, references, data };
        let mut state = self.state.lock().map_err(lock_error)?;
        if state.decisions.revision != revision {
            return Err(HolonError::InvalidState(
                "Visualizer evaluation superseded by agent reconsideration".into(),
            ));
        }
        if !state.index.contains_key(&evaluation.scope) {
            let index = state.decisions.evaluations.len();
            state.index.insert(evaluation.scope.clone(), index);
            state.decisions.evaluations.push(evaluation);
        }
        Ok(true)
    }
    /// Agent-directed reconsideration affects this demand, without changing mounted views.
    pub fn reconsider(&self, request: &CompositionRequest) -> Result<(), HolonError> {
        let scope = EvaluationScope::capture(request)?.ok_or_else(|| {
            HolonError::InvalidParameter("Reconsideration requires saved evaluation scope".into())
        })?;
        let mut state = self.state.lock().map_err(lock_error)?;
        state.decisions.revision = state
            .decisions
            .revision
            .checked_add(1)
            .ok_or_else(|| HolonError::InvalidState("Evaluation revision exhausted".into()))?;
        let mut occurrence_scope = scope.clone();
        occurrence_scope.occurrence = saved(&request.subject);
        state.decisions.evaluations.retain(|e| e.scope != scope && e.scope != occurrence_scope);
        state.reindex();
        if !state.decisions.reconsidered_scopes.contains(&scope) {
            state.decisions.reconsidered_scopes.push(scope);
        }
        Ok(())
    }
    /// Retire the resident carrier while preserving exact decision recipes.
    /// Reopening creates a new private owner; retired temporary IDs are never revived.
    pub fn evict_projections(&self) -> Result<(), HolonError> {
        let owner = {
            let mut state = self.state.lock().map_err(lock_error)?;
            state.root = None;
            state.owner.take()
        };
        if let Some(owner) = owner {
            owner.dispose()?;
        }
        Ok(())
    }

    /// Capture only this participant's private pool, never the agent or descriptor pool.
    /// The untyped root is an internal scalar carrier, not a public composition result.
    pub fn capture<T>(
        &self,
        project: impl FnOnce(&Arc<TransactionContext>, &TemporaryId) -> Result<T, HolonError>,
    ) -> Result<T, HolonError> {
        let mut state = self.state.lock().map_err(lock_error)?;
        if state.owner.is_none() {
            let space = self
                .space
                .upgrade()
                .ok_or_else(|| HolonError::ServiceNotAvailable("Evaluation space".into()))?;
            state.owner =
                Some(space.get_transaction_manager().open_session_derived_transaction(space)?);
        }
        let owner = state.owner.as_ref().unwrap().clone();
        if state.root.is_none() {
            state.root = Some(owner.mutation().new_holon(None)?.temporary_id());
        }
        let root_id = state.root.as_ref().unwrap();
        let mut root = TransientReference::from_temporary_id(owner.context_handle(), root_id);
        let decisions = state.decisions.clone();
        root.with_property_value(
            "CompositionData",
            serde_json::to_string(&decisions)
                .map_err(|e| HolonError::InvalidParameter(e.to_string()))?,
        )?;
        project(&owner, root_id)
    }
    /// Bind a private carrier without admitting its temporary IDs into public transactions.
    pub fn restore(
        agent: &Arc<TransactionContext>,
        owner_id: TxId,
        root: TemporaryId,
        bind: impl FnOnce(&Arc<TransactionContext>) -> Result<HolonPool, HolonError>,
    ) -> Result<Arc<Self>, HolonError> {
        if owner_id == agent.tx_id()
            || agent.descriptor_registry().and_then(|r| r.owned_context(owner_id)).is_some()
        {
            return Err(HolonError::InvalidParameter(
                "Evaluation carrier must have a distinct owner".into(),
            ));
        }
        let space = agent.space_manager();
        let registry = Arc::new(Self::new(&space));
        let owner = space
            .get_transaction_manager()
            .open_private_restricted_cache_read_transaction_with_id(space, owner_id)?;
        // Install cleanup before binding potentially cyclic malformed input.
        registry.state.lock().map_err(lock_error)?.owner = Some(owner.clone());
        let pool = bind(&owner)?;
        if pool.len() != 1 {
            return Err(HolonError::InvalidParameter(
                "Evaluation carrier must contain only its root".into(),
            ));
        }
        owner.import_transient_holons(pool)?;
        let reference = TransientReference::from_temporary_id(owner.context_handle(), &root);
        if reference.all_related_holons()?.count() != 0
            || reference.into_model()?.property_map.len() != 1
        {
            return Err(HolonError::InvalidParameter(
                "Evaluation carrier root must contain only scalar decisions".into(),
            ));
        }
        let value = reference
            .property_value("CompositionData")?
            .ok_or_else(|| HolonError::InvalidParameter("Missing evaluation decisions".into()))?;
        let base_types::BaseValue::StringValue(value) = value else {
            return Err(HolonError::InvalidParameter("Invalid evaluation decisions".into()));
        };
        let decisions: Decisions = serde_json::from_str(&value.0)
            .map_err(|e| HolonError::InvalidParameter(e.to_string()))?;
        for (index, evaluation) in decisions.evaluations.iter().enumerate() {
            if decisions.evaluations[..index].iter().any(|e| e.scope == evaluation.scope) {
                return Err(HolonError::InvalidParameter("Duplicate evaluation scope".into()));
            }
            let references = evaluation
                .references
                .iter()
                .map(|r| match r {
                    EvaluationReference::Subject => HolonReference::Transient(reference.clone()),
                    EvaluationReference::Saved(id) => {
                        SmartReference::new_from_id(agent.space_read_handle(), id.clone()).into()
                    }
                })
                .collect();
            ResolvedComposition { references, data: evaluation.data.clone() }.validate()?;
            if !evaluation.data.reusable_across_rounds
                || evaluation
                    .references
                    .iter()
                    .filter(|r| matches!(r, EvaluationReference::Subject))
                    .count()
                    > 1
            {
                return Err(HolonError::InvalidParameter(
                    "Invalid retained evaluation recipe".into(),
                ));
            }
        }
        let mut state = registry.state.lock().map_err(lock_error)?;
        state.decisions = decisions;
        state.reindex();
        state.root = Some(root);
        drop(state);
        Ok(registry)
    }
    /// Merge newly covered demands only if no agent request superseded this evaluation.
    pub fn adopt(&self, returned: &Self) -> Result<(), HolonError> {
        let incoming = returned.state.lock().map_err(lock_error)?.decisions.clone();
        let incoming_owner =
            returned.state.lock().map_err(lock_error)?.owner.as_ref().map(|o| o.tx_id());
        let incoming_root = returned.state.lock().map_err(lock_error)?.root.clone();
        let mut state = self.state.lock().map_err(lock_error)?;
        if state.owner.as_ref().map(|o| o.tx_id()) != incoming_owner || state.root != incoming_root
        {
            return Err(HolonError::InvalidParameter(
                "Returned evaluation carrier changed owner".into(),
            ));
        }
        if state.decisions.revision != incoming.revision {
            return Err(HolonError::InvalidState(
                "Visualizer evaluation superseded by agent reconsideration".into(),
            ));
        }
        for evaluation in &incoming.evaluations {
            if let Some(current) =
                state.decisions.evaluations.iter().find(|e| e.scope == evaluation.scope)
            {
                let mut current = current.clone();
                let mut incoming = evaluation.clone();
                current.data.work = Default::default();
                incoming.data.work = Default::default();
                if current != incoming {
                    return Err(HolonError::InvalidState("Concurrent evaluation conflicts with established choice; retry acquisition".into()));
                }
            }
        }
        for evaluation in incoming.evaluations {
            if !state.index.contains_key(&evaluation.scope) {
                let index = state.decisions.evaluations.len();
                state.index.insert(evaluation.scope.clone(), index);
                state.decisions.evaluations.push(evaluation);
            }
        }
        Ok(())
    }
}
impl Drop for VisualizerEvaluationRegistry {
    fn drop(&mut self) {
        if let Ok(state) = self.state.get_mut() {
            if let Some(owner) = state.owner.take() {
                let _ = owner.dispose();
            }
        }
    }
}
