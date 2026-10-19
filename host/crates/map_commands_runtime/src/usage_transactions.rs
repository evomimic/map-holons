use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use core_types::{HolonError, HolonId};
use dahn_selection::usage::{
    anchor, find_usage, initialize_live_usage, instances, record_key, single, VisualizerUsage,
};
use holons_core::core_shared_objects::transient_holon_manager::ToHolonCloneModel;
use holons_core::core_shared_objects::{
    holon::HolonCloneModel,
    space_manager::HolonSpaceManager,
    transactions::{TransactionContext, TxId},
    ReadableRelationship, RelationshipMap,
};
use holons_core::{HolonReference, ReadableHolon, TransientReference, WritableHolon};
use map_commands_contract::{
    VisualizerChoiceOrigin, VisualizerSelectionRequest, VisualizerUsageSelection,
    VisualizerUseReport,
};

use crate::usage_persistence::{SaveOutcome, SaveStatus, UsagePersistence, UsageSave};

#[derive(Default)]
struct Progress {
    saved: Option<HolonReference>,
    committed: u64,
}

struct LiveUsage {
    reference: HolonReference,
    progress: Arc<Mutex<Progress>>,
    submitted: u64,
}

struct Snapshot {
    model: HolonCloneModel,
    progress: Arc<Mutex<Progress>>,
    revision: u64,
    write_configuration: bool,
}

/// Authorization already established for this exact presentation in the open round.
struct PreparedUsage {
    request: VisualizerSelectionRequest,
    selected: HolonReference,
    usage: String,
    descriptor: HolonReference,
    usage_type: HolonReference,
}

fn same_request(a: &VisualizerSelectionRequest, b: &VisualizerSelectionRequest) -> bool {
    use map_commands_contract::VisualizerOwner;
    a.subject == b.subject
        && a.requested_kind == b.requested_kind
        && a.slot == b.slot
        && a.theme == b.theme
        && match (&a.owner, &b.owner) {
            (VisualizerOwner::Visualizer(a), VisualizerOwner::Visualizer(b))
            | (VisualizerOwner::Dancer(a), VisualizerOwner::Dancer(b)) => a == b,
            _ => false,
        }
}

struct Report {
    usage: String,
    slot: HolonReference,
    descriptor: HolonReference,
    origin: VisualizerChoiceOrigin,
    report: VisualizerUseReport,
}

#[derive(Default)]
struct PreferenceOrder {
    submitted: u64,
    committed: u64,
}

struct Batch {
    context: Arc<TransactionContext>,
    snapshots: HashMap<String, Snapshot>,
    reports: Vec<Report>,
    prepared: Vec<PreparedUsage>,
    preference_revisions: HashMap<(HolonId, HolonId), (Arc<Mutex<PreferenceOrder>>, u64)>,
}

/// Session-local coordination of live revisions and accepted batches, not a
/// cross-context registry of pending semantic Usage creations.
pub(crate) struct UsageTransactions {
    pub(crate) report_session: String,
    active: Mutex<HashMap<TxId, Batch>>,
    live: Mutex<HashMap<(TxId, String), LiveUsage>>,
    preferences: Arc<Mutex<HashMap<(HolonId, HolonId), (VisualizerUseReport, String)>>>,
    persistence: UsagePersistence,
    preference_order: Mutex<HashMap<(HolonId, HolonId), Arc<Mutex<PreferenceOrder>>>>,
}

impl Default for UsageTransactions {
    fn default() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self {
            report_session: format!(
                "{}-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("host clock")
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ),
            active: Default::default(),
            live: Default::default(),
            preferences: Default::default(),
            persistence: Default::default(),
            preference_order: Default::default(),
        }
    }
}

fn lock_error(error: impl std::fmt::Display) -> HolonError {
    HolonError::FailedToAcquireLock(error.to_string())
}

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

impl UsageTransactions {
    /// A round has one independent transaction; no commit is performed here.
    pub(crate) fn begin(
        &self,
        space: &Arc<HolonSpaceManager>,
        caller: &Arc<TransactionContext>,
    ) -> Result<(), HolonError> {
        let context = space.get_transaction_manager().open_public_transaction(Arc::clone(space))?;
        let mut active = self.active.lock().map_err(lock_error)?;
        if active.contains_key(&caller.tx_id()) {
            return Err(HolonError::InvalidState(
                "A Usage persistence round is already open".into(),
            ));
        }
        active.insert(
            caller.tx_id(),
            Batch {
                context,
                snapshots: HashMap::new(),
                reports: Vec::new(),
                prepared: Vec::new(),
                preference_revisions: HashMap::new(),
            },
        );
        Ok(())
    }

    /// Close and transfer the complete snapshot to the queue before acknowledging submission.
    pub(crate) fn submit(
        &self,
        caller: &Arc<TransactionContext>,
    ) -> Result<Option<u64>, HolonError> {
        let batch =
            self.active.lock().map_err(lock_error)?.remove(&caller.tx_id()).ok_or_else(|| {
                HolonError::InvalidState("No open Usage persistence round".into())
            })?;
        if batch.snapshots.is_empty() {
            return Ok(None);
        }
        self.persistence
            .submit(Box::new(BatchSave {
                batch,
                preferences: Arc::clone(&self.preferences),
                staged: HashMap::new(),
                staged_preferences: HashMap::new(),
                reports_prepared: false,
            }))
            .map(Some)
    }

    /// Disposing presentation state does not touch snapshots already owned by the queue.
    pub(crate) fn release_context(&self, tx_id: TxId) -> Result<(), HolonError> {
        self.active.lock().map_err(lock_error)?.remove(&tx_id);
        self.live.lock().map_err(lock_error)?.retain(|(owner, _), _| *owner != tx_id);
        Ok(())
    }

    pub(crate) fn status(&self, id: u64) -> Result<SaveStatus, HolonError> {
        self.persistence.status(id)
    }
    pub(crate) fn retry(&self, id: u64) -> Result<(), HolonError> {
        self.persistence.retry(id)
    }

    /// Pure lookup: never drains the save queue or observes another context's live creations.
    pub(crate) fn find(
        &self,
        caller: &Arc<TransactionContext>,
        request: VisualizerSelectionRequest,
        selected: HolonReference,
    ) -> Result<Option<VisualizerUsageSelection>, HolonError> {
        let descriptor = dahn_selection::usage_subject_type(&request)?;
        let slot = saved(caller, &request.slot)?;
        dahn_selection::choose_visualizer(caller, request.clone(), selected.clone())?;
        find_usage(&saved(caller, &selected)?, &saved(caller, &descriptor)?, &slot)?
            .map(|usage| {
                dahn_selection::usage::selection(
                    caller,
                    usage.holon().clone(),
                    false,
                    &self.report_session,
                )
            })
            .transpose()
    }

    fn project(
        &self,
        caller: &Arc<TransactionContext>,
        reference: HolonReference,
        initialized: bool,
    ) -> Result<VisualizerUsageSelection, HolonError> {
        let source = matches!(reference, HolonReference::Smart(_)).then(|| reference.clone());
        let live: HolonReference = caller.clone_holon(&reference)?.into();
        let key = (caller.tx_id(), live.reference_id_string());
        self.live.lock().map_err(lock_error)?.insert(
            key,
            LiveUsage {
                reference: live.clone(),
                progress: Arc::new(Mutex::new(Progress { saved: source, committed: 0 })),
                submitted: 0,
            },
        );
        Ok(VisualizerUsageSelection {
            usage: live,
            initialized,
            report_session: self.report_session.clone(),
        })
    }

    /// Select once and prepare Usage from the same validated descriptor context.
    pub(crate) fn select_visualizer(
        &self,
        caller: &Arc<TransactionContext>,
        request: VisualizerSelectionRequest,
    ) -> Result<map_commands_contract::VisualizerSelection, HolonError> {
        let started = std::time::Instant::now();
        let (mut selection, descriptor) =
            dahn_selection::select_visualizer_with_subject_type(caller, request.clone())?;
        let selection_ms = started.elapsed().as_secs_f64() * 1000.0;
        let usage_started = std::time::Instant::now();
        let prepare_usage = self.active.lock().map_err(lock_error)?.contains_key(&caller.tx_id());
        if prepare_usage {
            selection.usage = Some(self.select_prevalidated(
                caller,
                request,
                selection.selected.clone(),
                descriptor,
            )?);
        }
        if std::env::var_os("MAP_PROFILE").is_some() {
            eprintln!("[MAP-PROFILE] command=select_visualizer selection_ms={:.3} usage_ms={:.3} usage_prepared={}",
                selection_ms, usage_started.elapsed().as_secs_f64() * 1000.0, prepare_usage);
        }
        Ok(selection)
    }

    pub(crate) fn select(
        &self,
        _space: &Arc<HolonSpaceManager>,
        caller: &Arc<TransactionContext>,
        request: VisualizerSelectionRequest,
        selected: HolonReference,
    ) -> Result<VisualizerUsageSelection, HolonError> {
        dahn_selection::choose_visualizer(caller, request.clone(), selected.clone())?;
        let descriptor = dahn_selection::usage_subject_type(&request)?;
        self.select_prevalidated(caller, request, selected, descriptor)
    }

    /// Only reached immediately after selection or explicit-choice validation.
    fn select_prevalidated(
        &self,
        caller: &Arc<TransactionContext>,
        request: VisualizerSelectionRequest,
        selected: HolonReference,
        descriptor: HolonReference,
    ) -> Result<VisualizerUsageSelection, HolonError> {
        let slot = saved(caller, &request.slot)?;
        let visualizer = saved(caller, &selected)?;
        let descriptor = saved(caller, &descriptor)?;
        if !self.active.lock().map_err(lock_error)?.contains_key(&caller.tx_id()) {
            return Err(HolonError::InvalidState(
                "Select Usage inside an explicit persistence round".into(),
            ));
        }
        let found = find_usage(&visualizer, &descriptor, &slot)?;
        let key = match &found {
            Some(usage) => usage.holon().key()?,
            None => Some(record_key("visualizer-usage", &visualizer, &descriptor)?),
        };
        if let Some(key) = key {
            match caller.lookup().get_transient_holon_by_base_key(&key) {
                Ok(existing) => {
                    let existing: HolonReference = existing.into();
                    let value = VisualizerUsage::from_holon(caller, existing.clone())?;
                    if value.visualizer()? == visualizer
                        && value.subject_type()? == descriptor
                        && self
                            .live
                            .lock()
                            .map_err(lock_error)?
                            .contains_key(&(caller.tx_id(), existing.reference_id_string()))
                    {
                        let result = VisualizerUsageSelection {
                            usage: existing,
                            initialized: false,
                            report_session: self.report_session.clone(),
                        };
                        self.prepare(caller, request, selected, descriptor, &result)?;
                        return Ok(result);
                    }
                }
                Err(HolonError::HolonNotFound(_)) => {}
                Err(error) => return Err(error),
            }
        }
        let result = if let Some(usage) = found {
            self.project(caller, usage.holon().clone(), false)?
        } else {
            let live: HolonReference =
                initialize_live_usage(caller, visualizer, descriptor.clone())?.into();
            self.live.lock().map_err(lock_error)?.insert(
                (caller.tx_id(), live.reference_id_string()),
                LiveUsage { reference: live.clone(), progress: Default::default(), submitted: 0 },
            );
            VisualizerUsageSelection {
                usage: live,
                initialized: true,
                report_session: self.report_session.clone(),
            }
        };
        self.prepare(caller, request, selected, descriptor, &result)?;
        Ok(result)
    }

    fn prepare(
        &self,
        caller: &Arc<TransactionContext>,
        request: VisualizerSelectionRequest,
        selected: HolonReference,
        descriptor: HolonReference,
        result: &VisualizerUsageSelection,
    ) -> Result<(), HolonError> {
        self.capture_snapshot(caller, &result.usage, result.initialized)?;
        self.active
            .lock()
            .map_err(lock_error)?
            .get_mut(&caller.tx_id())
            .ok_or_else(|| HolonError::InvalidState("No open Usage persistence round".into()))?
            .prepared
            .push(PreparedUsage {
                request,
                selected,
                descriptor,
                usage_type: single(&result.usage, "DescribedBy")?,
                usage: result.usage.reference_id_string(),
            });
        Ok(())
    }

    /// Snapshot within the open batch; repeated changes coalesce into its next revision.
    pub(crate) fn capture(
        &self,
        caller: &Arc<TransactionContext>,
        usage: &HolonReference,
    ) -> Result<(), HolonError> {
        self.capture_snapshot(caller, usage, true)
    }

    fn capture_snapshot(
        &self,
        caller: &Arc<TransactionContext>,
        usage: &HolonReference,
        write_configuration: bool,
    ) -> Result<(), HolonError> {
        let identity = usage.reference_id_string();
        let source = self
            .live
            .lock()
            .map_err(lock_error)?
            .get(&(caller.tx_id(), identity.clone()))
            .map(|entry| entry.reference.clone())
            .ok_or_else(|| {
                HolonError::InvalidParameter("Usage does not belong to this presentation".into())
            })?;
        // Copy authored state only. Constructing a destination transient would
        // populate defaults and perform descriptor reads on the rendering path.
        let mut model = source.holon_clone_model()?;
        if let Some(relationships) = model.relationships.take() {
            model.relationships =
                Some(RelationshipMap::from(relationships.clone_for_new_source()?));
        }
        let mut active = self.active.lock().map_err(lock_error)?;
        let batch = active.get_mut(&caller.tx_id()).ok_or_else(|| {
            HolonError::InvalidState("Usage round closed during snapshot capture".into())
        })?;
        let mut live = self.live.lock().map_err(lock_error)?;
        let entry = live.get_mut(&(caller.tx_id(), identity.clone())).ok_or_else(|| {
            HolonError::InvalidParameter(
                "Usage presentation was disposed during snapshot capture".into(),
            )
        })?;
        let revision = if let Some(previous) = batch.snapshots.get(&identity) {
            previous.revision
        } else {
            entry.submitted += 1;
            entry.submitted
        };
        let write_configuration = write_configuration
            || batch.snapshots.get(&identity).is_some_and(|snapshot| snapshot.write_configuration);
        batch.snapshots.insert(
            identity,
            Snapshot {
                model,
                progress: Arc::clone(&entry.progress),
                revision,
                write_configuration,
            },
        );
        Ok(())
    }

    pub(crate) fn record(
        &self,
        _space: &Arc<HolonSpaceManager>,
        caller: &Arc<TransactionContext>,
        request: VisualizerSelectionRequest,
        selected: HolonReference,
        usage: HolonReference,
        origin: VisualizerChoiceOrigin,
        report: VisualizerUseReport,
    ) -> Result<(), HolonError> {
        let identity = usage.reference_id_string();
        let prepared_descriptor =
            self.active.lock().map_err(lock_error)?.get(&caller.tx_id()).and_then(|batch| {
                batch
                    .prepared
                    .iter()
                    .find(|prepared| {
                        prepared.usage == identity
                            && prepared.selected == selected
                            && same_request(&prepared.request, &request)
                    })
                    .map(|prepared| (prepared.descriptor.clone(), prepared.usage_type.clone()))
            });
        let descriptor = if let Some((descriptor, usage_type)) = prepared_descriptor {
            if single(&usage, "UsesVisualizer")? != selected
                || single(&usage, "ForSubjectType")? != descriptor
                || single(&usage, "DescribedBy")? != usage_type
            {
                return Err(HolonError::InvalidParameter(
                    "Prepared Usage identity was changed".into(),
                ));
            }
            descriptor
        } else {
            // Unprepared reports still undergo the ordinary eligibility checks.
            let descriptor = dahn_selection::usage_subject_type(&request)?;
            dahn_selection::choose_visualizer(caller, request.clone(), selected.clone())?;
            let value = VisualizerUsage::from_holon(caller, usage.clone())?;
            if value.visualizer()? != selected || value.subject_type()? != descriptor {
                return Err(HolonError::InvalidParameter(
                    "Reported Usage does not match the Visualizer and descriptor".into(),
                ));
            }
            descriptor
        };
        let slot = request.slot.clone();
        if report.session != self.report_session
            || report.occurrence_id.is_empty()
            || report.sequence == 0
            || report.sequence > 9_007_199_254_740_991
        {
            return Err(HolonError::InvalidParameter(
                "Invalid or expired presentation report identity".into(),
            ));
        }
        if origin == VisualizerChoiceOrigin::Explicit {
            let scope = (slot.holon_id()?, descriptor.holon_id()?);
            let mut latest = self.preferences.lock().map_err(lock_error)?;
            if let Some((previous, previous_usage)) = latest.get(&scope) {
                if previous.sequence == report.sequence
                    && (previous != &report || previous_usage != &identity)
                {
                    return Err(HolonError::InvalidParameter(
                        "Choice token reused for a different presentation".into(),
                    ));
                }
            }
            if latest.get(&scope).is_none_or(|(previous, _)| previous.sequence <= report.sequence) {
                latest.insert(scope, (report.clone(), identity.clone()));
            }
        }
        self.capture_snapshot(caller, &usage, false)?;
        let mut active = self.active.lock().map_err(lock_error)?;
        let batch = active
            .get_mut(&caller.tx_id())
            .ok_or_else(|| HolonError::InvalidState("No open Usage persistence round".into()))?;
        if origin == VisualizerChoiceOrigin::Explicit {
            let scope = (slot.holon_id()?, descriptor.holon_id()?);
            if !batch.preference_revisions.contains_key(&scope) {
                let progress = self
                    .preference_order
                    .lock()
                    .map_err(lock_error)?
                    .entry(scope.clone())
                    .or_default()
                    .clone();
                let revision = {
                    let mut order = progress.lock().map_err(lock_error)?;
                    order.submitted += 1;
                    order.submitted
                };
                batch.preference_revisions.insert(scope, (progress, revision));
            }
        }
        if !batch
            .reports
            .iter()
            .any(|old| old.usage == identity && old.report == report && old.origin == origin)
        {
            batch.reports.push(Report { usage: identity, slot, descriptor, origin, report });
        }
        Ok(())
    }
}

struct BatchSave {
    batch: Batch,
    preferences: Arc<Mutex<HashMap<(HolonId, HolonId), (VisualizerUseReport, String)>>>,
    staged: HashMap<String, HolonReference>,
    staged_preferences: HashMap<(HolonId, HolonId), HolonReference>,
    reports_prepared: bool,
}

impl UsageSave for BatchSave {
    fn is_ready(&self) -> bool {
        self.batch.snapshots.values().all(|snapshot| {
            snapshot
                .progress
                .lock()
                .map(|progress| progress.committed + 1 == snapshot.revision)
                .unwrap_or(false)
        }) && self.batch.preference_revisions.values().all(|(progress, revision)| {
            progress.lock().map(|order| order.committed + 1 == *revision).unwrap_or(false)
        })
    }

    fn attempt(&mut self) -> Result<SaveOutcome, HolonError> {
        if !self.reports_prepared {
            let context = &self.batch.context;
            for (identity, snapshot) in &self.batch.snapshots {
                if !self.staged.contains_key(identity) {
                    let previous = snapshot.progress.lock().map_err(lock_error)?.saved.clone();
                    let reference: HolonReference = if let Some(previous) = previous {
                        let previous = saved(context, &previous)?;
                        let slots = dahn_selection::usage::related(&previous, "SelectedForSlot")?;
                        let association_changed = self.batch.reports.iter().any(|report| {
                            report.usage == *identity && !slots.contains(&report.slot)
                        });
                        if snapshot.write_configuration || association_changed {
                            context
                                .mutation()
                                .stage_new_version_from_id(previous.holon_id()?)?
                                .into()
                        } else {
                            previous
                        }
                    } else {
                        let reference =
                            context.new_transient_from_clone_model(snapshot.model.clone())?;
                        context.mutation().stage_new_holon(reference)?.into()
                    };
                    // Retain the staged identity before fallible configuration mutations.
                    self.staged.insert(identity.clone(), reference);
                }
                if snapshot.write_configuration {
                    let reference =
                        context.new_transient_from_clone_model(snapshot.model.clone())?;
                    apply_snapshot(
                        self.staged.get_mut(identity).expect("prepared Usage"),
                        &reference,
                    )?;
                }
            }
            // Keep partially prepared work on errors; explicit retry never restages creation.
            self.prepare_reports()?;
            self.reports_prepared = true;
        }
        // A repeated automatic presentation may contain no durable changes.
        let changed = self.staged.values().any(|value| matches!(value, HolonReference::Staged(_)))
            || !self.staged_preferences.is_empty();
        if changed {
            let response = self.batch.context.commit()?;
            if self.batch.context.is_open() {
                return match response.property_value("CommitRequestStatus")? {
                    Some(base_types::BaseValue::StringValue(status))
                        if status.0 == "Incomplete" =>
                    {
                        Ok(SaveOutcome::Incomplete)
                    }
                    Some(base_types::BaseValue::StringValue(status)) if status.0 == "Rejected" => {
                        Err(HolonError::InvalidState(
                            "Usage commit was rejected; correction is required".into(),
                        ))
                    }
                    _ => Err(HolonError::InvalidState(
                        "Usage commit has no confirmed completion outcome".into(),
                    )),
                };
            }
        }
        for (identity, snapshot) in &self.batch.snapshots {
            let reference = &self.staged[identity];
            let mut progress = snapshot.progress.lock().map_err(lock_error)?;
            progress.saved = Some(HolonReference::smart_from_id(
                self.batch.context.space_read_handle(),
                reference.holon_id()?,
            ));
            progress.committed = snapshot.revision;
        }
        for (progress, revision) in self.batch.preference_revisions.values() {
            progress.lock().map_err(lock_error)?.committed = *revision;
        }
        Ok(SaveOutcome::Complete)
    }
}

fn apply_snapshot(
    target: &mut HolonReference,
    snapshot: &TransientReference,
) -> Result<(), HolonError> {
    let old = target.holon_clone_model()?;
    let new = snapshot.holon_clone_model()?;
    for name in old.properties.keys() {
        if !new.properties.contains_key(name) {
            target.remove_property_value(name)?;
        }
    }
    for (name, value) in new.properties {
        target.with_property_value(name, value)?;
    }
    let old = target.all_related_holons()?;
    let new = snapshot.all_related_holons()?;
    let members = |map: &holons_core::RelationshipMap,
                   name: &core_types::RelationshipName|
     -> Result<Vec<HolonReference>, HolonError> {
        map.get_collection_for_relationship(name)
            .map(|collection| {
                collection.read().map(|value| value.get_members().clone()).map_err(lock_error)
            })
            .transpose()
            .map(Option::unwrap_or_default)
    };
    for (name, _) in old.iter() {
        if name.to_string() == "SelectedForSlot" {
            continue;
        }
        let previous = members(&old, &name)?;
        let current = members(&new, &name)?;
        let removed =
            previous.into_iter().filter(|value| !current.contains(value)).collect::<Vec<_>>();
        if !removed.is_empty() {
            target.remove_related_holons(name, removed)?;
        }
    }
    for (name, _) in new.iter() {
        if name.to_string() == "SelectedForSlot" {
            continue;
        }
        let previous = members(&old, &name)?;
        let current = members(&new, &name)?;
        let added =
            current.into_iter().filter(|value| !previous.contains(value)).collect::<Vec<_>>();
        if !added.is_empty() {
            target.add_related_holons(name, added)?;
        }
    }
    Ok(())
}

impl BatchSave {
    fn prepare_reports(&mut self) -> Result<(), HolonError> {
        let context = &self.batch.context;
        for report in &self.batch.reports {
            let reference = &self.staged[&report.usage];
            let slot = saved(context, &report.slot)?;
            let descriptor = saved(context, &report.descriptor)?;
            if !dahn_selection::usage::related(reference, "SelectedForSlot")?.contains(&slot) {
                let mut reference = reference.clone();
                reference.add_related_holons("SelectedForSlot", vec![slot.clone()])?;
            }
            let current = self
                .preferences
                .lock()
                .map_err(lock_error)?
                .get(&(slot.holon_id()?, descriptor.holon_id()?))
                .cloned();
            if report.origin != VisualizerChoiceOrigin::Explicit
                || current.as_ref().is_none_or(|(latest, identity)| {
                    latest != &report.report || identity != &report.usage
                })
            {
                continue;
            }
            let scope = (slot.holon_id()?, descriptor.holon_id()?);
            if let Some(preference) = self.staged_preferences.get_mut(&scope) {
                let previous = dahn_selection::usage::related(preference, "PreferredUsage")?;
                if previous != vec![reference.clone()] {
                    if !previous.is_empty() {
                        preference.remove_related_holons("PreferredUsage", previous)?;
                    }
                    preference.add_related_holons("PreferredUsage", vec![reference.clone()])?;
                }
                continue;
            }
            let preferences = instances(context, "VisualizerSlotPreference.HolonType")?
                .into_iter()
                .filter_map(|p| {
                    match (single(&p, "PreferenceForSlot"), single(&p, "PreferenceForSubjectType"))
                    {
                        (Ok(s), Ok(d)) if s == slot && d == descriptor => Some(Ok(p)),
                        (Err(e), _) | (_, Err(e)) => Some(Err(e)),
                        _ => None,
                    }
                })
                .collect::<Result<Vec<_>, HolonError>>()?;
            match preferences.as_slice() {
                [] => {
                    let mut preference = context.mutation().new_holon(Some(record_key(
                        "slot-preference",
                        &slot,
                        &descriptor,
                    )?))?;
                    preference.add_related_holons(
                        "DescribedBy",
                        vec![anchor(context, "VisualizerSlotPreference.HolonType")?],
                    )?;
                    preference.add_related_holons("PreferenceForSlot", vec![slot])?;
                    preference.add_related_holons("PreferenceForSubjectType", vec![descriptor])?;
                    preference.add_related_holons("PreferredUsage", vec![reference.clone()])?;
                    let staged = context.mutation().stage_new_holon(preference)?;
                    self.staged_preferences.insert(scope, staged.into());
                }
                [preference] => {
                    let old = single(preference, "PreferredUsage")?;
                    if old != *reference {
                        let mut staged =
                            context.mutation().stage_new_version_from_id(preference.holon_id()?)?;
                        self.staged_preferences.insert(scope.clone(), staged.clone().into());
                        staged.remove_related_holons("PreferredUsage", vec![old])?;
                        staged.add_related_holons("PreferredUsage", vec![reference.clone()])?;
                    }
                }
                _ => {
                    return Err(HolonError::InvalidState(
                        "Multiple remembered preferences for slot and subject descriptor".into(),
                    ))
                }
            }
        }
        Ok(())
    }
}
