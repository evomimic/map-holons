//! Transaction-local derived products for inputs frozen during Commit assessment.
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use core_types::{HolonError, TemporaryId};

use super::TransactionContext;
use crate::{descriptors::DerivedDescriptor, HolonReference};

#[derive(Default)]
struct FrozenDerivedEntry {
    lineage: Option<Vec<HolonReference>>,
    result: Option<DerivedDescriptor>,
}

/// Exact mutable reference identity; staged candidates never alias saved predecessors.
#[derive(Clone, Hash, PartialEq, Eq)]
enum FrozenDescriptorId {
    Staged(TemporaryId),
    Transient(TemporaryId),
}

/// Shared validity of products from one assessment, independent of the saved Session.
#[derive(Debug, Default)]
pub(crate) struct FrozenValidationToken {
    retired: AtomicBool,
}

impl FrozenValidationToken {
    pub(crate) fn check_active(&self) -> Result<(), HolonError> {
        if self.retired.load(Ordering::Acquire) {
            Err(HolonError::ObsoleteSession)
        } else {
            Ok(())
        }
    }
}

#[derive(Default)]
pub(super) struct FrozenValidationState {
    token: Arc<FrozenValidationToken>,
    entries: Mutex<HashMap<FrozenDescriptorId, FrozenDerivedEntry>>,
}

/// Ends frozen-input reuse on every assessment exit, including errors and unwinding.
/// This marks a semantic phase; it does not block transient output construction or
/// introduce a space-wide mutation barrier. Callers must keep descriptor inputs frozen.
#[must_use = "retain the scope guard until assessment finishes"]
pub struct FrozenValidationScope<'a> {
    context: &'a TransactionContext,
    state: Arc<FrozenValidationState>,
}

impl TransactionContext {
    /// Retains already diagnosed mutable input lineage for normal accessor acquisition.
    /// This supplies construction inputs, not a separate acquisition API or a result cache.
    /// Saved inputs remain governed by the space registry; outside assessment nothing is retained.
    pub fn retain_frozen_descriptor_lineage(
        &self,
        lineage: crate::descriptors::ValidExtendsLineage<'_>,
    ) -> Result<(), HolonError> {
        let Some(id) = self.frozen_descriptor_id(lineage.subject())? else { return Ok(()) };
        let state = self.frozen_validation.lock().map_err(lock_error)?.clone();
        let Some(state) = state else { return Ok(()) };
        // Mutable dependencies must belong to the same frozen candidate graph.
        let owner = lineage.subject().transaction_context().expect("mutable lineage owner");
        for member in lineage.members() {
            crate::assert_reference_transaction_compatible(member, &owner)?;
        }
        state.token.check_active()?;
        state
            .entries
            .lock()
            .map_err(lock_error)?
            .entry(id)
            .or_default()
            .lineage
            .get_or_insert_with(|| lineage.members().to_vec());
        Ok(())
    }

    /// Supplies construction with already diagnosed input, without traversing the graph.
    /// Missing or out-of-scope inputs retain the constructor's normal lineage resolution.
    pub(crate) fn frozen_descriptor_lineage(
        &self,
        target: &HolonReference,
    ) -> Result<Option<Vec<HolonReference>>, HolonError> {
        let Some(id) = self.frozen_descriptor_id(target)? else { return Ok(None) };
        let state = self.frozen_validation.lock().map_err(lock_error)?.clone();
        let Some(state) = state else { return Ok(None) };
        state.token.check_active()?;
        let lineage = state
            .entries
            .lock()
            .map_err(lock_error)?
            .get(&id)
            .and_then(|entry| entry.lineage.clone());
        Ok(lineage)
    }

    fn frozen_descriptor_id(
        &self,
        target: &HolonReference,
    ) -> Result<Option<FrozenDescriptorId>, HolonError> {
        match target {
            HolonReference::Smart(_) => Ok(None),
            HolonReference::Staged(reference) if reference.tx_id() == self.tx_id() => {
                Ok(Some(FrozenDescriptorId::Staged(reference.temporary_id())))
            }
            HolonReference::Transient(reference) if reference.tx_id() == self.tx_id() => {
                Ok(Some(FrozenDescriptorId::Transient(reference.temporary_id())))
            }
            _ => Err(HolonError::InvalidParameter(
                "Frozen derived input must belong to this transaction".into(),
            )),
        }
    }
    /// Starts an assessment-local reuse scope before descriptor preparation.
    /// A retry must enter a new scope after correcting inputs. Nested scopes are rejected.
    pub fn begin_frozen_validation(&self) -> Result<FrozenValidationScope<'_>, HolonError> {
        let mut active = self.frozen_validation.lock().map_err(lock_error)?;
        if active.is_some() {
            return Err(HolonError::InvalidState("Frozen validation is already active".into()));
        }
        let state = Arc::new(FrozenValidationState::default());
        *active = Some(state.clone());
        Ok(FrozenValidationScope { context: self, state })
    }

    /// Shares completed mutable-source products only within the current frozen scope.
    /// Construction runs without a cache lock and retains the source transaction owner.
    pub(crate) fn acquire_frozen_derived(
        &self,
        target: &HolonReference,
        construct: impl FnOnce() -> Result<DerivedDescriptor, HolonError>,
    ) -> Result<DerivedDescriptor, HolonError> {
        let state = self.frozen_validation.lock().map_err(lock_error)?.clone();
        let Some(state) = state else { return construct() };
        let id = self.frozen_descriptor_id(target)?.ok_or_else(|| {
            HolonError::InvalidParameter("Frozen derived input must be mutable".into())
        })?;
        state.token.check_active()?;
        if let Some(result) = state
            .entries
            .lock()
            .map_err(lock_error)?
            .get(&id)
            .and_then(|entry| entry.result.as_ref())
        {
            return Ok(result.clone());
        }
        let result = construct()?.in_frozen_validation(state.token.clone());
        let mut entries = state.entries.lock().map_err(lock_error)?;
        state.token.check_active()?;
        Ok(entries.entry(id).or_default().result.get_or_insert(result).clone())
    }
}

impl Drop for FrozenValidationScope<'_> {
    fn drop(&mut self) {
        self.state.token.retired.store(true, Ordering::Release);
        if let Ok(mut active) = self.context.frozen_validation.lock() {
            *active = None;
        }
        // Break retained result -> owner -> scope cycles even if a direct result survives.
        if let Ok(mut entries) = self.state.entries.lock() {
            entries.clear();
        }
    }
}

fn lock_error<T>(error: std::sync::PoisonError<T>) -> HolonError {
    HolonError::FailedToAcquireLock(error.to_string())
}
