use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use core_types::{HolonError, HolonId};
use dahn_selection::usage::{
    anchor, find_usage, initialize_usage, instances, record_key, selection, single, VisualizerUsage,
};
use holons_core::core_shared_objects::{
    space_manager::HolonSpaceManager, transactions::TransactionContext,
};
use holons_core::{HolonReference, ReadableHolon, WritableHolon};
use map_commands_contract::{
    VisualizerChoiceOrigin, VisualizerSelectionRequest, VisualizerUsageSelection,
    VisualizerUseReport,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Operation {
    Initialize(HolonId, HolonId),
    Present(HolonId, HolonId, VisualizerChoiceOrigin),
}

/// Retained transaction work, including pass-two retries after an incomplete commit.
struct PendingCommit {
    context: Arc<TransactionContext>,
    result: HolonReference,
}

/// Local-space coordination, not authoritative usage/configuration storage.
/// The gate spans lookup, staging and commit so concurrent initializers recheck saved state.
pub(crate) struct UsageTransactions {
    pub(crate) report_session: String,
    latest_preferences: Mutex<HashMap<(HolonId, HolonId), (VisualizerUseReport, HolonId)>>,
    pending: Mutex<HashMap<Operation, PendingCommit>>,
}

impl Default for UsageTransactions {
    fn default() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);
        let boot = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("host clock predates Unix epoch")
            .as_nanos();
        Self {
            report_session: format!("{boot}-{}", NEXT_SESSION.fetch_add(1, Ordering::Relaxed)),
            pending: Default::default(),
            latest_preferences: Default::default(),
        }
    }
}

fn lock_error(error: impl std::fmt::Display) -> HolonError {
    HolonError::FailedToAcquireLock(error.to_string())
}

/// Only persisted semantic identities may escape an independent transaction.
fn saved(
    context: &Arc<TransactionContext>,
    reference: &HolonReference,
) -> Result<HolonReference, HolonError> {
    if !matches!(reference, HolonReference::Smart(_)) {
        return Err(HolonError::InvalidParameter(
            "Usage requires persisted Visualizer, slot and subject descriptor identities".into(),
        ));
    }
    Ok(HolonReference::smart_from_id(context.space_read_handle(), reference.holon_id()?))
}

fn open(space: &Arc<HolonSpaceManager>) -> Result<Arc<TransactionContext>, HolonError> {
    space.get_transaction_manager().open_public_transaction(Arc::clone(space))
}

/// Check the lifecycle result, not merely the absence of an execution error.
fn commit(work: &PendingCommit) -> Result<(), HolonError> {
    work.context.commit()?;
    if work.context.is_open() {
        return Err(HolonError::InvalidState(
            "Usage transaction was rejected or incomplete; retry required".into(),
        ));
    }
    Ok(())
}

/// Complete older writes before observing saved state or accepting a newer outcome.
/// Failed work stays retained; partially persisted records never become usable through this service.
fn resume_pending(
    pending: &mut HashMap<Operation, PendingCommit>,
    current: &Operation,
) -> Result<Option<HolonReference>, HolonError> {
    let keys: Vec<_> = pending.keys().cloned().collect();
    let mut result = None;
    for key in keys {
        let work = pending.get(&key).expect("retained transaction");
        commit(work)?;
        if &key == current {
            result = Some(work.result.clone());
        }
        pending.remove(&key);
    }
    Ok(result)
}

impl UsageTransactions {
    pub(crate) fn select(
        &self,
        space: &Arc<HolonSpaceManager>,
        caller: &Arc<TransactionContext>,
        request: VisualizerSelectionRequest,
        selected: HolonReference,
    ) -> Result<VisualizerUsageSelection, HolonError> {
        let subject_type = dahn_selection::usage_subject_type(&request)?;
        let slot = request.slot.clone();
        // Revalidate the actual slot, owner, subject, Theme and candidate through the selector.
        dahn_selection::choose_visualizer(caller, request, selected.clone())?;
        let key = Operation::Initialize(selected.holon_id()?, subject_type.holon_id()?);
        let mut pending = self.pending.lock().map_err(lock_error)?;
        if let Some(usage) = resume_pending(&mut pending, &key)? {
            return selection(caller, usage, true, &self.report_session);
        }
        let context = open(space)?;
        let visualizer = saved(&context, &selected)?;
        let descriptor = saved(&context, &subject_type)?;
        let slot = saved(&context, &slot)?;
        if let Some(usage) = find_usage(&visualizer, &descriptor, &slot)? {
            return selection(caller, usage.holon().clone(), false, &self.report_session);
        }
        let staged = initialize_usage(&context, visualizer, descriptor)?;
        pending.insert(key, PendingCommit { context, result: staged.into() });
        let key = Operation::Initialize(selected.holon_id()?, subject_type.holon_id()?);
        let work = pending.get(&key).expect("inserted usage transaction");
        commit(work)?;
        let result = selection(caller, work.result.clone(), true, &self.report_session)?;
        pending.remove(&key);
        Ok(result)
    }

    pub(crate) fn record(
        &self,
        space: &Arc<HolonSpaceManager>,
        caller: &Arc<TransactionContext>,
        request: VisualizerSelectionRequest,
        selected: HolonReference,
        usage: HolonReference,
        origin: VisualizerChoiceOrigin,
        report: VisualizerUseReport,
    ) -> Result<(), HolonError> {
        let subject_type = dahn_selection::usage_subject_type(&request)?;
        let slot = request.slot.clone();
        dahn_selection::choose_visualizer(caller, request, selected.clone())?;
        let key = Operation::Present(slot.holon_id()?, usage.holon_id()?, origin);
        let mut pending = self.pending.lock().map_err(lock_error)?;
        // Validate the report before resuming any retained work for it.
        let reported = VisualizerUsage::from_holon(caller, usage.clone())?;
        if reported.visualizer()? != selected || reported.subject_type()? != subject_type {
            return Err(HolonError::InvalidParameter(
                "Reported usage does not match selected Visualizer and subject descriptor".into(),
            ));
        }
        if report.session != self.report_session
            || report.occurrence_id.is_empty()
            || report.sequence == 0
            || report.sequence > 9_007_199_254_740_991
        {
            return Err(HolonError::InvalidParameter(
                "Invalid or expired presentation report identity".into(),
            ));
        }
        // One gate covers independent transactions and all occurrences of this preference.
        // Older pending commits finish before a newer write; delayed deliveries cannot undo it.
        let mut preferences_order = self.latest_preferences.lock().map_err(lock_error)?;
        let mut remember = origin == VisualizerChoiceOrigin::Explicit;
        if remember {
            let scope = (slot.holon_id()?, subject_type.holon_id()?);
            if let Some((latest, latest_usage)) = preferences_order.get(&scope) {
                if report.sequence < latest.sequence {
                    remember = false;
                }
                if report.sequence == latest.sequence
                    && (report != *latest || usage.holon_id()? != *latest_usage)
                {
                    return Err(HolonError::InvalidParameter(
                        "Choice token reused for a different presentation".into(),
                    ));
                }
            }
            if remember {
                preferences_order.insert(scope, (report, usage.holon_id()?));
            }
        }
        resume_pending(&mut pending, &key)?;
        let context = open(space)?;
        let visualizer = saved(&context, &selected)?;
        let descriptor = saved(&context, &subject_type)?;
        let slot = saved(&context, &slot)?;
        let reference = saved(&context, &usage)?;
        let usage = VisualizerUsage::from_holon(&context, reference.clone())?;
        if usage.visualizer()? != visualizer || usage.subject_type()? != descriptor {
            return Err(HolonError::InvalidParameter(
                "Reported usage does not match selected Visualizer and subject descriptor".into(),
            ));
        }
        let mut changed = false;
        if !usage.selected_slots()?.contains(&slot) {
            let mut staged = context.mutation().stage_new_version_from_id(reference.holon_id()?)?;
            staged.add_related_holons("SelectedForSlot", vec![slot.clone()])?;
            changed = true;
        }
        if remember {
            let mut preferences = Vec::new();
            for preference in instances(&context, "VisualizerSlotPreference.HolonType")? {
                if single(&preference, "PreferenceForSlot")? == slot
                    && single(&preference, "PreferenceForSubjectType")? == descriptor
                {
                    preferences.push(preference);
                }
            }
            match preferences.as_slice() {
                [] => {
                    let mut preference = context.mutation().new_holon(Some(record_key(
                        "slot-preference",
                        &slot,
                        &descriptor,
                    )?))?;
                    preference.add_related_holons(
                        "DescribedBy",
                        vec![anchor(&context, "VisualizerSlotPreference.HolonType")?],
                    )?;
                    preference.add_related_holons("PreferenceForSlot", vec![slot.clone()])?;
                    preference.add_related_holons("PreferenceForSubjectType", vec![descriptor])?;
                    preference.add_related_holons("PreferredUsage", vec![reference.clone()])?;
                    context.mutation().stage_new_holon(preference)?;
                    changed = true;
                }
                [preference] => {
                    let old = single(preference, "PreferredUsage")?;
                    if old != reference {
                        let mut staged =
                            context.mutation().stage_new_version_from_id(preference.holon_id()?)?;
                        staged.remove_related_holons("PreferredUsage", vec![old])?;
                        staged.add_related_holons("PreferredUsage", vec![reference.clone()])?;
                        changed = true;
                    }
                }
                _ => {
                    return Err(HolonError::InvalidState(
                        "Multiple remembered preferences for slot and subject descriptor".into(),
                    ))
                }
            }
        }
        if changed {
            pending.insert(key, PendingCommit { context, result: reference });
            let key = Operation::Present(slot.holon_id()?, usage.holon().holon_id()?, origin);
            commit(pending.get(&key).expect("inserted presentation transaction"))?;
            pending.remove(&key);
        }
        Ok(())
    }
}
