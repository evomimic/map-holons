use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

use base_types::{BaseValue, MapInteger, MapString};
use core_types::{HolonError, PropertyName};
use holons_core::core_shared_objects::transactions::{TransactionContext, TxId};
use holons_core::{HolonReference, WritableHolon};
use map_commands_contract::{
    DiscoveryStopReason, VisualizerDiscovery, VisualizerOwner, VisualizerSelectionRequest,
};

/// Retains only explicitly requested evidence. Projection consumes the snapshot;
/// closing its source transaction or releasing the inspector drops unused evidence.
pub(crate) struct DiscoveryEvidence {
    namespace: u128,
    next: AtomicU64,
    snapshots: Mutex<HashMap<MapString, Snapshot>>,
}

impl Default for DiscoveryEvidence {
    fn default() -> Self {
        Self {
            namespace: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("host clock predates Unix epoch")
                .as_nanos(),
            next: Default::default(),
            snapshots: Default::default(),
        }
    }
}

struct Snapshot {
    source: TxId,
    request: VisualizerSelectionRequest,
    discovery: VisualizerDiscovery,
}

impl DiscoveryEvidence {
    pub fn retain(
        &self,
        source: TxId,
        request: VisualizerSelectionRequest,
        discovery: &VisualizerDiscovery,
    ) -> Result<MapString, HolonError> {
        let mut snapshots =
            self.snapshots.lock().map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?;
        if snapshots.values().filter(|s| s.source == source).count() >= 128 {
            return Err(HolonError::InvalidState("Close unused Visualizer information sessions before retaining more discovery evidence".into()));
        }
        let key = MapString(format!(
            "discovery:{}:{}:{}",
            self.namespace,
            source.value(),
            self.next.fetch_add(1, Ordering::Relaxed)
        ));
        snapshots.insert(key.clone(), Snapshot { source, request, discovery: discovery.clone() });
        Ok(key)
    }

    pub fn release(&self, source: &TxId, key: &MapString) -> Result<(), HolonError> {
        let mut snapshots =
            self.snapshots.lock().map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?;
        if snapshots.get(key).is_some_and(|s| &s.source != source) {
            return Err(HolonError::InvalidParameter(
                "Discovery evidence belongs to another transaction".into(),
            ));
        }
        snapshots.remove(key);
        Ok(())
    }

    pub fn release_transaction(&self, source: &TxId) -> Result<(), HolonError> {
        self.snapshots
            .lock()
            .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
            .retain(|_, s| &s.source != source);
        Ok(())
    }

    /// Reads the captured graph once, without running discovery or eligibility again.
    /// The destination owns all new transient records and releases them on disposal.
    pub fn project(
        &self,
        context: &Arc<TransactionContext>,
        key: &MapString,
    ) -> Result<HolonReference, HolonError> {
        let snapshot = self
            .snapshots
            .lock()
            .map_err(|e| HolonError::FailedToAcquireLock(e.to_string()))?
            .remove(key)
            .ok_or_else(|| {
                HolonError::InvalidParameter(
                    "Discovery snapshot is released or already projected".into(),
                )
            })?;
        // All records share three schema types. Resolve each saved anchor once
        // within this projection instead of crossing the guest boundary per record.
        let mut descriptors = HashMap::new();
        let mut subject =
            described(context, &mut descriptors, "VisualizerDiscovery.Projection", key.clone())?;
        let reason = match snapshot.discovery.stop_reason {
            DiscoveryStopReason::HolonTypeBoundary => "holon_type_boundary",
            DiscoveryStopReason::LineageExhausted => "lineage_exhausted",
        };
        text(&mut subject, "DiscoveryStopReason", reason)?;
        text(
            &mut subject,
            "DiscoveryRequestedKind",
            match snapshot.request.requested_kind {
                map_commands_contract::VisualizerKind::Node => "node",
                map_commands_contract::VisualizerKind::Property => "property",
                map_commands_contract::VisualizerKind::Value => "value",
                map_commands_contract::VisualizerKind::PropertyMap => "property_map",
                map_commands_contract::VisualizerKind::ActionBar => "action_bar",
                map_commands_contract::VisualizerKind::Action => "action",
                map_commands_contract::VisualizerKind::RootedNavigation => "rooted_navigation",
                map_commands_contract::VisualizerKind::Structure => "structure",
                map_commands_contract::VisualizerKind::Canvas => "canvas",
                map_commands_contract::VisualizerKind::Collection => "collection",
            },
        )?;
        text(&mut subject, "SelectionRationale", "Selection rationale is unavailable; this evidence explains candidate identification only.")?;
        subject.add_related_holons("DiscoverySubject", vec![snapshot.request.subject])?;
        subject.add_related_holons("DiscoverySlot", vec![snapshot.request.slot])?;
        subject.add_related_holons("DiscoveryTheme", vec![snapshot.request.theme])?;
        let owner = match snapshot.request.owner {
            VisualizerOwner::Visualizer(owner) | VisualizerOwner::Dancer(owner) => owner,
        };
        subject.add_related_holons("DiscoveryOwner", vec![owner])?;
        if let Some(current) = &snapshot.discovery.current_selection {
            subject.add_related_holons(
                "DiscoveryCurrentVisualizer",
                vec![current.visualizer.clone()],
            )?;
        }
        if let Some(endpoint) = snapshot.discovery.ancestry.last() {
            subject.add_related_holons("DiscoveryEndpoint", vec![endpoint.clone()])?;
        }
        let mut levels = Vec::new();
        for (index, descriptor) in snapshot.discovery.ancestry.iter().enumerate() {
            let mut level = described(
                context,
                &mut descriptors,
                "VisualizerDiscoveryLevel.Projection",
                MapString(format!("{}:level:{index}", key)),
            )?;
            level.with_property_value(
                PropertyName("DiscoveryLevelIndex".into()),
                BaseValue::IntegerValue(MapInteger(index as i64)),
            )?;
            level.add_related_holons("DiscoveryDescriptor", vec![descriptor.clone()])?;
            levels.push(level);
        }
        subject.add_related_holons("DiscoveryLevels", levels)?;
        let mut candidates = snapshot.discovery.candidates;
        if let Some(current) = snapshot.discovery.current_selection {
            if !candidates.iter().any(|c| c.visualizer == current.visualizer) {
                candidates.push(current);
            }
        }
        let mut records = Vec::new();
        for (index, candidate) in candidates.into_iter().enumerate() {
            let mut record = described(
                context,
                &mut descriptors,
                "VisualizerDiscoveryCandidate.Projection",
                MapString(format!("{}:candidate:{index}", key)),
            )?;
            text(
                &mut record,
                "DiscoveryAssessment",
                match candidate.assessment {
                    map_commands_contract::VisualizerAssessment::Viable => "viable",
                    map_commands_contract::VisualizerAssessment::IncompatibleSlot => {
                        "incompatible_slot"
                    }
                    map_commands_contract::VisualizerAssessment::IncompatibleTheme => {
                        "incompatible_theme"
                    }
                    map_commands_contract::VisualizerAssessment::ImplementationUnavailable => {
                        "implementation_unavailable"
                    }
                    map_commands_contract::VisualizerAssessment::NoLongerApplicable => {
                        "no_longer_applicable"
                    }
                },
            )?;
            record.add_related_holons("DiscoveryVisualizer", vec![candidate.visualizer])?;
            record.add_related_holons("DiscoveryDeclarations", candidate.declared_on)?;
            records.push(record);
        }
        subject.add_related_holons("DiscoveryCandidates", records)?;
        Ok(subject)
    }
}

fn described(
    context: &Arc<TransactionContext>,
    descriptors: &mut HashMap<String, HolonReference>,
    key: &str,
    instance_key: MapString,
) -> Result<HolonReference, HolonError> {
    let descriptor = match descriptors.get(key) {
        Some(descriptor) => descriptor.clone(),
        None => {
            let descriptor: HolonReference =
                context.lookup().get_saved_holon_by_key(&MapString::from(key))?.into();
            descriptors.insert(key.to_owned(), descriptor.clone());
            descriptor
        }
    };
    let mut reference: HolonReference = context.mutation().new_holon(Some(instance_key))?.into();
    reference.with_descriptor(descriptor)?;
    Ok(reference)
}

fn text(reference: &mut HolonReference, name: &str, value: &str) -> Result<(), HolonError> {
    reference
        .with_property_value(
            PropertyName(name.into()),
            BaseValue::StringValue(MapString::from(value)),
        )
        .map(|_| ())
}
