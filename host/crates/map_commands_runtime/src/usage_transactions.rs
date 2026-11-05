use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use core_types::{HolonError, HolonId};
use dahn_selection::usage::{
    anchor, initialize_live_usage, instances, record_key, single, VisualizerUsage,
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

use crate::prepared_visualizer_bindings::{
    same_request, PreparedVisualizerBinding, PreparedVisualizerBindings,
};
use crate::usage_persistence::{SaveOutcome, SaveStatus, UsagePersistence, UsageSave};

/// Attribute backend trace events to preparation phases without changing their execution.
/// The enclosing IPC span identifies the selection; nested phases must not be summed twice.
fn profile_usage_phase<T>(
    phase: &'static str,
    operation: impl FnOnce() -> Result<T, HolonError>,
) -> Result<T, HolonError> {
    if std::env::var_os("MAP_PROFILE").is_none() {
        return operation();
    }
    let span = tracing::info_span!(target: "map_profile", "usage.prepare", phase);
    let _entered = span.enter();
    let started = std::time::Instant::now();
    let result = operation();
    tracing::info!(target: "map_profile", usage_phase = phase,
        phase_ms = started.elapsed().as_secs_f64() * 1000.0,
        succeeded = result.is_ok());
    result
}

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
    first_revision: u64,
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
    usage_descriptor: HolonReference,
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

/// Committed answers only; live configuration is applied again for every request.
/// Each entry serializes identical in-flight lookups, leaving different inputs independent.
struct UsageResolutionEntry {
    visualizer: HolonReference,
    descriptor: HolonReference,
    slot: HolonReference,
    result: Mutex<Option<holons_core::dances::VisualizerUsageResolution>>,
}

struct UsageResolutionMemo {
    generation: u64,
    entries: Mutex<Vec<Arc<UsageResolutionEntry>>>,
}

struct Batch {
    owner: TxId,
    context: Arc<TransactionContext>,
    snapshots: HashMap<String, Snapshot>,
    reports: Vec<Report>,
    prepared: Vec<PreparedUsage>,
    selection_metadata: Option<(u64, Arc<Mutex<dahn_selection::PreparedSelectionMetadata>>)>,
    usage_resolutions: Option<Arc<UsageResolutionMemo>>,
    preference_revisions: HashMap<(HolonId, HolonId), (Arc<Mutex<PreferenceOrder>>, u64, u64)>,
}

/// Session-local coordination of live revisions and accepted batches, not a
/// cross-context registry of pending semantic Usage creations.
pub(crate) struct UsageTransactions {
    pub(crate) report_session: String,
    active: Mutex<HashMap<TxId, Batch>>,
    live: Mutex<HashMap<(TxId, String), LiveUsage>>,
    preferences: Arc<Mutex<HashMap<(HolonId, HolonId), (VisualizerUseReport, String)>>>,
    persistence: UsagePersistence,
    bindings: PreparedVisualizerBindings,
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
            bindings: Default::default(),
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
                owner: caller.tx_id(),
                context,
                snapshots: HashMap::new(),
                reports: Vec::new(),
                prepared: Vec::new(),
                selection_metadata: None,
                usage_resolutions: None,
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
        let mut batch =
            self.active.lock().map_err(lock_error)?.remove(&caller.tx_id()).ok_or_else(|| {
                HolonError::InvalidState("No open Usage persistence round".into())
            })?;
        // Metadata belongs to preparation, never to the background save queue.
        batch.selection_metadata = None;
        batch.usage_resolutions = None;
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
        self.bindings.release(tx_id)
    }

    /// Suspend reuse while a semantic command changes the selection context.
    pub(crate) fn begin_binding_change(
        &self,
    ) -> Result<crate::prepared_visualizer_bindings::BindingChange<'_>, HolonError> {
        let change = self.bindings.begin_change()?;
        for batch in self.active.lock().map_err(lock_error)?.values_mut() {
            batch.prepared.clear();
        }
        Ok(change)
    }

    /// Retire derived bindings after an explicit semantic context refresh.
    pub(crate) fn invalidate_bindings(&self) -> Result<(), HolonError> {
        self.bindings.invalidate()?;
        for batch in self.active.lock().map_err(lock_error)?.values_mut() {
            batch.prepared.clear();
        }
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
        caller
            .resolve_visualizer_usage(
                &saved(caller, &selected)?,
                &saved(caller, &descriptor)?,
                &slot,
            )?
            .usage
            .map(|usage| {
                dahn_selection::usage::selection(caller, usage.clone(), false, &self.report_session)
            })
            .transpose()
    }

    fn project(
        &self,
        caller: &Arc<TransactionContext>,
        reference: HolonReference,
        initialized: bool,
        selected: &HolonReference,
        descriptor: &HolonReference,
    ) -> Result<VisualizerUsageSelection, HolonError> {
        if let Some(key) = reference.key()? {
            match caller.lookup().get_transient_holon_by_base_key(&key) {
                Ok(existing) => {
                    let existing: HolonReference = existing.into();
                    if self
                        .live
                        .lock()
                        .map_err(lock_error)?
                        .contains_key(&(caller.tx_id(), existing.reference_id_string()))
                    {
                        if single(&existing, "UsesVisualizer")? != *selected
                            || single(&existing, "ForSubjectType")? != *descriptor
                        {
                            return Err(HolonError::InvalidState(
                                "Retained Usage identity no longer matches its saved source".into(),
                            ));
                        }
                        return Ok(VisualizerUsageSelection {
                            usage: existing,
                            initialized: false,
                            report_session: self.report_session.clone(),
                        });
                    }
                }
                Err(HolonError::HolonNotFound(_)) => {}
                Err(error) => return Err(error),
            }
        }
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
        let prepare_usage = self.active.lock().map_err(lock_error)?.contains_key(&caller.tx_id());
        let generation = self.bindings.generation()?;
        if prepare_usage {
            if let Some(binding) = self.bindings.get(caller.tx_id(), &request)? {
                // Availability is a cheap host catalog/file check; retained metadata
                // never permits executing an implementation that disappeared.
                let identity_unchanged = single(&binding.usage.usage, "UsesVisualizer")?
                    == binding.selected
                    && single(&binding.usage.usage, "ForSubjectType")? == binding.usage_descriptor
                    && single(&binding.usage.usage, "DescribedBy")? == binding.usage_type;
                if identity_unchanged
                    && caller.visualizer_artifact_available(&binding.selected)?
                    && self.bindings.is_current(generation)?
                {
                    let usage =
                        VisualizerUsageSelection { initialized: false, ..binding.usage.clone() };
                    self.prepare(
                        caller,
                        request.clone(),
                        binding.selected.clone(),
                        binding.descriptor,
                        &usage,
                    )?;
                    if std::env::var_os("MAP_PROFILE").is_some() {
                        eprintln!("[MAP-PROFILE] command=select_visualizer binding_reused=true total_ms={:.3}", started.elapsed().as_secs_f64() * 1000.0);
                    }
                    return Ok(map_commands_contract::VisualizerSelection {
                        selected: binding.selected,
                        requested_kind: request.requested_kind,
                        alternatives_available: binding.alternatives_available,
                        usage: Some(usage),
                    });
                }
                self.bindings.invalidate()?;
            }
        }
        let metadata = if prepare_usage && self.bindings.is_current(generation)? {
            let mut active = self.active.lock().map_err(lock_error)?;
            let batch = active.get_mut(&caller.tx_id()).ok_or_else(|| {
                HolonError::InvalidState("Usage round closed during selection".into())
            })?;
            if batch.selection_metadata.as_ref().is_none_or(|(version, _)| *version != generation) {
                batch.selection_metadata = Some((
                    generation,
                    Arc::new(Mutex::new(dahn_selection::PreparedSelectionMetadata::default())),
                ));
            }
            batch.selection_metadata.as_ref().map(|(_, metadata)| Arc::clone(metadata))
        } else {
            None
        };
        let mut metadata_hits = 0;
        let (mut selection, descriptor) = if let Some(metadata) = metadata {
            let mut metadata = metadata.lock().map_err(lock_error)?;
            let before = metadata.hits();
            let selection = dahn_selection::select_visualizer_with_metadata(
                caller,
                request.clone(),
                &mut metadata,
            )?;
            metadata_hits = metadata.hits() - before;
            selection
        } else {
            dahn_selection::select_visualizer_with_subject_type(caller, request.clone())?
        };
        let selection_ms = started.elapsed().as_secs_f64() * 1000.0;
        let usage_started = std::time::Instant::now();
        if prepare_usage {
            let usage = self.select_prevalidated(
                caller,
                request.clone(),
                selection.selected.clone(),
                descriptor.clone(),
            )?;
            self.bindings.retain(
                caller.tx_id(),
                generation,
                PreparedVisualizerBinding {
                    request,
                    selected: selection.selected.clone(),
                    descriptor,
                    usage_descriptor: single(&usage.usage, "ForSubjectType")?,
                    usage: usage.clone(),
                    usage_type: single(&usage.usage, "DescribedBy")?,
                    alternatives_available: selection.alternatives_available,
                },
            )?;
            selection.usage = Some(usage);
        }
        if std::env::var_os("MAP_PROFILE").is_some() {
            eprintln!("[MAP-PROFILE] command=select_visualizer selection_ms={:.3} usage_ms={:.3} usage_prepared={} binding_reused=false metadata_hits={}",
                selection_ms, usage_started.elapsed().as_secs_f64() * 1000.0, prepare_usage, metadata_hits);
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

    /// Reuse a committed lookup only within this round and semantic generation.
    fn resolve_committed_usage(
        &self,
        caller: &Arc<TransactionContext>,
        visualizer: &HolonReference,
        descriptor: &HolonReference,
        slot: &HolonReference,
    ) -> Result<holons_core::dances::VisualizerUsageResolution, HolonError> {
        let generation = self.bindings.generation()?;
        let memo = if self.bindings.is_current(generation)? {
            let mut active = self.active.lock().map_err(lock_error)?;
            let batch = active.get_mut(&caller.tx_id()).ok_or_else(|| {
                HolonError::InvalidState("Usage round closed during lookup".into())
            })?;
            if batch.usage_resolutions.as_ref().is_none_or(|memo| memo.generation != generation) {
                batch.usage_resolutions = Some(Arc::new(UsageResolutionMemo {
                    generation,
                    entries: Mutex::new(Vec::new()),
                }));
            }
            batch.usage_resolutions.clone()
        } else {
            None
        };
        let entry = if let Some(memo) = memo {
            let mut entries = memo.entries.lock().map_err(lock_error)?;
            if let Some(entry) = entries.iter().find(|entry| {
                entry.visualizer == *visualizer
                    && entry.descriptor == *descriptor
                    && entry.slot == *slot
            }) {
                Some(Arc::clone(entry))
            } else if entries.len() < 256 {
                let entry = Arc::new(UsageResolutionEntry {
                    visualizer: visualizer.clone(),
                    descriptor: descriptor.clone(),
                    slot: slot.clone(),
                    result: Mutex::new(None),
                });
                entries.push(Arc::clone(&entry));
                Some(entry)
            } else {
                None
            }
        } else {
            None
        };
        let mut reused = false;
        let result = if let Some(entry) = entry {
            let mut saved = entry.result.lock().map_err(lock_error)?;
            let current = self.bindings.is_current(generation)?;
            if let Some(result) = saved.as_ref().filter(|_| current) {
                reused = true;
                result.clone()
            } else {
                // A failure leaves no answer behind, so a subsequent request can retry.
                let result = caller.resolve_visualizer_usage(visualizer, descriptor, slot)?;
                if self.bindings.is_current(generation)? {
                    *saved = Some(result.clone());
                }
                result
            }
        } else {
            caller.resolve_visualizer_usage(visualizer, descriptor, slot)?
        };
        if std::env::var_os("MAP_PROFILE").is_some() {
            eprintln!(
                "[MAP-PROFILE] usage_lookup memo_hit={} visualizer={} subject_type={} slot={}",
                reused,
                visualizer.reference_id_string(),
                descriptor.reference_id_string(),
                slot.reference_id_string()
            );
        }
        Ok(result)
    }

    /// Only reached immediately after selection or explicit-choice validation.
    fn select_prevalidated(
        &self,
        caller: &Arc<TransactionContext>,
        request: VisualizerSelectionRequest,
        selected: HolonReference,
        descriptor: HolonReference,
    ) -> Result<VisualizerUsageSelection, HolonError> {
        let (slot, visualizer, descriptor) = profile_usage_phase("resolve_saved_inputs", || {
            Ok((
                saved(caller, &request.slot)?,
                saved(caller, &selected)?,
                saved(caller, &descriptor)?,
            ))
        })?;
        if !self.active.lock().map_err(lock_error)?.contains_key(&caller.tx_id()) {
            return Err(HolonError::InvalidState(
                "Select Usage inside an explicit persistence round".into(),
            ));
        }
        let resolution = profile_usage_phase("resolve_committed_usage", || {
            self.resolve_committed_usage(caller, &visualizer, &descriptor, &slot)
        })?;
        let found = resolution.usage;
        let existing = profile_usage_phase("find_transient_usage", || {
            for scope in &resolution.search_path {
                let key = if resolution.usage_type.as_ref() == Some(scope) {
                    match &found {
                        Some(usage) => usage.key()?,
                        None => None,
                    }
                } else {
                    Some(record_key("visualizer-usage", &visualizer, scope)?)
                };
                if let Some(key) = key {
                    match caller.lookup().get_transient_holon_by_base_key(&key) {
                        Ok(existing) => {
                            let existing: HolonReference = existing.into();
                            // Only this session's validated live configurations are eligible.
                            let known = self
                                .live
                                .lock()
                                .map_err(lock_error)?
                                .contains_key(&(caller.tx_id(), existing.reference_id_string()));
                            if known
                                && single(&existing, "UsesVisualizer")? == visualizer
                                && single(&existing, "ForSubjectType")? == *scope
                            {
                                return Ok(Some(VisualizerUsageSelection {
                                    usage: existing,
                                    initialized: false,
                                    report_session: self.report_session.clone(),
                                }));
                            }
                        }
                        Err(HolonError::HolonNotFound(_)) => {}
                        Err(error) => return Err(error),
                    }
                }
                // A nearer committed Usage takes precedence over a farther live default.
                if resolution.usage_type.as_ref() == Some(scope) {
                    break;
                }
            }
            Ok(None)
        })?;
        if std::env::var_os("MAP_PROFILE").is_some() {
            tracing::info!(target: "map_profile", usage_resolution = if existing.is_some() {
                "reused_transient"
            } else if found.is_some() {
                "cloned_saved"
            } else {
                "initialized_new"
            }, saved_usage_found = found.is_some());
        }
        if let Some(result) = existing {
            self.prepare(caller, request, selected, descriptor, &result)?;
            return Ok(result);
        }
        let result = if let Some(usage) = found {
            profile_usage_phase("clone_saved_usage", || {
                self.project(
                    caller,
                    usage.clone(),
                    false,
                    &visualizer,
                    resolution.usage_type.as_ref().ok_or_else(|| {
                        HolonError::InvalidState("Resolved Usage has no bound descriptor".into())
                    })?,
                )
            })?
        } else {
            let live: HolonReference = profile_usage_phase("initialize_defaults", || {
                initialize_live_usage(caller, visualizer, resolution.default_type)
            })?
            .into();
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

    /// Authorize a particular saved or retained live configuration independently of default resolution.
    pub(crate) fn select_existing(
        &self,
        caller: &Arc<TransactionContext>,
        request: VisualizerSelectionRequest,
        selected: HolonReference,
        usage: HolonReference,
    ) -> Result<VisualizerUsageSelection, HolonError> {
        if !self.active.lock().map_err(lock_error)?.contains_key(&caller.tx_id()) {
            return Err(HolonError::InvalidState("Select Usage inside a persistence round".into()));
        }
        dahn_selection::choose_visualizer(caller, request.clone(), selected.clone())?;
        let descriptor = dahn_selection::usage_subject_type(&request)?;
        let value = VisualizerUsage::from_holon(caller, usage.clone())?;
        let scope = value.subject_type()?;
        if value.visualizer()? != selected
            || !holons_core::descriptors::equals_or_extends(&descriptor, &scope)?
        {
            return Err(HolonError::InvalidParameter(
                "Selected Usage does not apply to this Visualizer and subject lineage".into(),
            ));
        }
        let known = self
            .live
            .lock()
            .map_err(lock_error)?
            .contains_key(&(caller.tx_id(), usage.reference_id_string()));
        let result = if known {
            VisualizerUsageSelection {
                usage,
                initialized: false,
                report_session: self.report_session.clone(),
            }
        } else if matches!(usage, HolonReference::Smart(_)) {
            self.project(caller, usage, false, &selected, &scope)?
        } else {
            return Err(HolonError::InvalidParameter(
                "Select a saved Usage or this context's live Usage".into(),
            ));
        };
        self.prepare(caller, request, selected, descriptor, &result)?;
        Ok(result)
    }

    /// Explicitly fork effective configuration for the actual subject descriptor.
    /// Editing an inherited Usage directly continues to edit its shared scope.
    pub(crate) fn specialize(
        &self,
        caller: &Arc<TransactionContext>,
        request: VisualizerSelectionRequest,
        selected: HolonReference,
        usage: HolonReference,
    ) -> Result<VisualizerUsageSelection, HolonError> {
        if !self.active.lock().map_err(lock_error)?.contains_key(&caller.tx_id()) {
            return Err(HolonError::InvalidState(
                "Specialize Usage inside a persistence round".into(),
            ));
        }
        dahn_selection::choose_visualizer(caller, request.clone(), selected.clone())?;
        let descriptor = dahn_selection::usage_subject_type(&request)?;
        let value = VisualizerUsage::from_holon(caller, usage.clone())?;
        if value.visualizer()? != selected
            || !holons_core::descriptors::equals_or_extends(&descriptor, &value.subject_type()?)?
        {
            return Err(HolonError::InvalidParameter(
                "Usage does not apply to this Visualizer and subject lineage".into(),
            ));
        }
        // Respect existing subtype configuration rather than overwriting it with an ancestor copy.
        let resolved = self.select_prevalidated(
            caller,
            request.clone(),
            selected.clone(),
            descriptor.clone(),
        )?;
        if single(&resolved.usage, "ForSubjectType")? == descriptor {
            return Ok(resolved);
        }
        let key = record_key("visualizer-usage", &selected, &descriptor)?;
        let mut model = usage.holon_clone_model()?;
        model.original_id = None;
        model.properties.insert(
            core_types::PropertyName("Key".into()),
            base_types::BaseValue::StringValue(key),
        );
        if let Some(relationships) = model.relationships.take() {
            model.relationships =
                Some(RelationshipMap::from(relationships.clone_for_new_source()?));
        }
        let mut specialized: HolonReference = caller.new_transient_from_clone_model(model)?.into();
        let old_scope = single(&specialized, "ForSubjectType")?;
        specialized.remove_related_holons("ForSubjectType", vec![old_scope])?;
        specialized.add_related_holons("ForSubjectType", vec![descriptor.clone()])?;
        let slots = dahn_selection::usage::related(&specialized, "SelectedForSlot")?;
        if !slots.is_empty() {
            specialized.remove_related_holons("SelectedForSlot", slots)?;
        }
        self.live.lock().map_err(lock_error)?.insert(
            (caller.tx_id(), specialized.reference_id_string()),
            LiveUsage {
                reference: specialized.clone(),
                progress: Default::default(),
                submitted: 0,
            },
        );
        let result = VisualizerUsageSelection {
            usage: specialized,
            initialized: true,
            report_session: self.report_session.clone(),
        };
        self.bindings.invalidate()?;
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
        profile_usage_phase("capture_snapshot", || {
            self.capture_snapshot(caller, &result.usage, result.initialized)
        })?;
        profile_usage_phase("prepare_report", || {
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
                    usage_descriptor: single(&result.usage, "ForSubjectType")?,
                    usage: result.usage.reference_id_string(),
                });
            Ok(())
        })
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
                first_revision: revision,
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
                    .map(|prepared| {
                        (
                            prepared.descriptor.clone(),
                            prepared.usage_type.clone(),
                            prepared.usage_descriptor.clone(),
                        )
                    })
            });
        let descriptor =
            if let Some((descriptor, usage_type, usage_descriptor)) = prepared_descriptor {
                if single(&usage, "UsesVisualizer")? != selected
                    || single(&usage, "ForSubjectType")? != usage_descriptor
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
                if value.visualizer()? != selected
                    || !holons_core::descriptors::equals_or_extends(
                        &descriptor,
                        &value.subject_type()?,
                    )?
                {
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
        let mut establish_binding = false;
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
                establish_binding = true;
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
                batch.preference_revisions.insert(scope, (progress, revision, revision));
            }
        }
        if !batch
            .reports
            .iter()
            .any(|old| old.usage == identity && old.report == report && old.origin == origin)
        {
            batch.reports.push(Report {
                usage: identity,
                slot,
                descriptor: descriptor.clone(),
                origin,
                report,
            });
        }
        if establish_binding {
            // Publication, rather than mere candidate authorization, establishes the choice.
            self.bindings.invalidate()?;
            self.bindings.retain(
                caller.tx_id(),
                self.bindings.generation()?,
                PreparedVisualizerBinding {
                    request,
                    selected,
                    descriptor,
                    usage_descriptor: single(&usage, "ForSubjectType")?,
                    usage_type: single(&usage, "DescribedBy")?,
                    usage: VisualizerUsageSelection {
                        usage,
                        initialized: false,
                        report_session: self.report_session.clone(),
                    },
                    alternatives_available: false,
                },
            )?;
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
    fn conflicts(&self, other: &dyn UsageSave) -> bool {
        let other: &dyn std::any::Any = other;
        let Some(other) = other.downcast_ref::<Self>() else { return true };
        self.batch.snapshots.keys().any(|id| other.batch.snapshots.contains_key(id))
            || self
                .batch
                .preference_revisions
                .keys()
                .any(|scope| other.batch.preference_revisions.contains_key(scope))
    }

    fn coalesce(&mut self, newer: &mut dyn UsageSave) -> bool {
        let newer: &mut dyn std::any::Any = newer;
        let Some(newer) = newer.downcast_mut::<Self>() else { return false };
        if self.batch.owner != newer.batch.owner
            || !self.staged.is_empty()
            || self.reports_prepared
            || !newer.staged.is_empty()
            || newer.reports_prepared
            || !self.batch.snapshots.keys().any(|id| newer.batch.snapshots.contains_key(id))
        {
            return false;
        }
        // A merged revision range must not jump over another batch (including
        // failed work). Keep both Usage and cross-Usage preference dependencies.
        for (id, next) in &newer.batch.snapshots {
            if let Some(previous) = self.batch.snapshots.get(id) {
                if !Arc::ptr_eq(&previous.progress, &next.progress)
                    || previous.revision + 1 != next.first_revision
                {
                    return false;
                }
            }
        }
        for (scope, (progress, first, _)) in &newer.batch.preference_revisions {
            if let Some((previous, _, last)) = self.batch.preference_revisions.get(scope) {
                if !Arc::ptr_eq(previous, progress) || *last + 1 != *first {
                    return false;
                }
            }
        }
        for (id, mut next) in newer.batch.snapshots.drain() {
            if let Some(previous) = self.batch.snapshots.remove(&id) {
                next.first_revision = previous.first_revision;
                next.write_configuration |= previous.write_configuration;
            }
            self.batch.snapshots.insert(id, next);
        }
        for (scope, (progress, mut first, last)) in newer.batch.preference_revisions.drain() {
            if let Some((_, previous_first, _)) = self.batch.preference_revisions.remove(&scope) {
                first = previous_first;
            }
            self.batch.preference_revisions.insert(scope, (progress, first, last));
        }
        self.batch.reports.append(&mut newer.batch.reports);
        self.batch.prepared.append(&mut newer.batch.prepared);
        true
    }
    fn is_ready(&self) -> bool {
        self.batch.snapshots.values().all(|snapshot| {
            snapshot
                .progress
                .lock()
                .map(|progress| progress.committed + 1 == snapshot.first_revision)
                .unwrap_or(false)
        }) && self.batch.preference_revisions.values().all(|(progress, first, _)| {
            progress.lock().map(|order| order.committed + 1 == *first).unwrap_or(false)
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
        for (progress, _, revision) in self.batch.preference_revisions.values() {
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
