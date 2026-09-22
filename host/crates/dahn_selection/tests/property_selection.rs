use base_types::{BaseValue, MapInteger, MapString};
use core_types::{HolonError, HolonId, LocalId, PropertyMap, PropertyName, RelationshipName};
use holons_core::core_shared_objects::{
    holon::{Holon, SavedHolon},
    space_manager::HolonSpaceManager,
    transactions::TransactionContext,
};
use holons_core::{
    HolonCollection, HolonCollectionApi, HolonReference, HolonServiceApi, ReadableHolon,
    RelationshipCachePolicy, RelationshipMap, ServiceRoutingPolicy, SmartReference,
    StagedReference, TransientReference,
};
use map_commands_contract::{VisualizerKind, VisualizerSelectionRequest};
use std::{any::Any, collections::HashMap, sync::Arc};

/// A persisted graph fixture accessed through the real bound reference layer.
#[derive(Debug, Default)]
struct Graph {
    edges: HashMap<(u8, String), Vec<u8>>,
}
impl Graph {
    fn edge(&mut self, source: u8, name: &str, targets: &[u8]) {
        self.edges.insert((source, name.into()), targets.into());
    }
    fn reference(context: &Arc<TransactionContext>, id: u8) -> HolonReference {
        HolonReference::smart_with_key(
            context.space_read_handle(),
            HolonId::Local(LocalId(vec![id])),
            MapString(format!("node-{id}")),
        )
    }
}
impl HolonServiceApi for Graph {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn relationship_cache_policy(
        &self,
        _: &Arc<TransactionContext>,
        _: &HolonId,
        _: &RelationshipName,
    ) -> Result<RelationshipCachePolicy, HolonError> {
        Ok(RelationshipCachePolicy::Fresh)
    }
    fn get_saved_holon_by_key_internal(
        &self,
        context: &Arc<TransactionContext>,
        key: &MapString,
    ) -> Result<SmartReference, HolonError> {
        let id = match key.0.as_str() {
            "PropertiesVisualizer.HolonType" => 10,
            "ActionVisualizer.HolonType" => 13,
            "PropertyVisualizer.HolonType" => 11,
            "ValueVisualizer.HolonType" => 12,
            "TableCollectionVisualizer.CollectionVisualizer" => 20,
            _ => panic!("unexpected lookup {key}"),
        };
        match Self::reference(context, id) {
            HolonReference::Smart(reference) => Ok(reference),
            _ => unreachable!(),
        }
    }
    fn fetch_holon_internal(
        &self,
        _: &Arc<TransactionContext>,
        id: &HolonId,
    ) -> Result<Holon, HolonError> {
        let mut properties = PropertyMap::new();
        properties.insert(
            PropertyName("Key".into()),
            BaseValue::StringValue(MapString(format!("node-{}", id.local_id().0[0]))),
        );
        properties.insert(
            PropertyName("TypeName".into()),
            BaseValue::StringValue(MapString("Fixture".into())),
        );
        Ok(Holon::Saved(SavedHolon::new(id.local_id().clone(), properties, None, MapInteger(1))))
    }
    fn fetch_related_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
        id: &HolonId,
        name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
        let mut result = HolonCollection::new_transient();
        result.add_references(
            self.edges
                .get(&(id.local_id().0[0], name.to_string()))
                .into_iter()
                .flatten()
                .map(|id| Self::reference(context, *id))
                .collect(),
        )?;
        Ok(result)
    }
    fn commit_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &[StagedReference],
    ) -> Result<TransientReference, HolonError> {
        panic!("read-only selection")
    }
    fn delete_holon_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &LocalId,
    ) -> Result<(), HolonError> {
        panic!("read-only selection")
    }
    fn fetch_all_related_holons_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        panic!("unexpected bulk read")
    }
    fn get_all_holons_internal(
        &self,
        _: &Arc<TransactionContext>,
    ) -> Result<HolonCollection, HolonError> {
        panic!("unexpected scan")
    }
    fn load_holons_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: TransientReference,
    ) -> Result<TransientReference, HolonError> {
        panic!("unexpected load")
    }
}
fn select(
    kind: VisualizerKind,
    candidates: &[u8],
    inherited: bool,
    value_types: &[u8],
) -> Result<u8, HolonError> {
    let mut graph = Graph::default();
    // owner -> describing type; property -> declared value type. A competing
    // applicability edge on the property must never control Value selection.
    graph.edge(1, "DescribedBy", &[2]);
    graph.edge(3, "ValueType", value_types);
    graph.edge(3, "HasApplicableVisualizer", &[21]);
    graph.edge(21, "DescribedBy", &[11]);
    let (subject, start, role) = match kind {
        VisualizerKind::Properties => (1, 2, 10),
        VisualizerKind::Action => (1, 2, 13),
        VisualizerKind::Property => (3, 3, 11),
        VisualizerKind::Value => (3, 4, 12),
        _ => unreachable!(),
    };
    if inherited {
        graph.edge(start, "HasApplicableVisualizer", &[]);
        graph.edge(start, "Extends", &[5]);
    }
    graph.edge(if inherited { 5 } else { start }, "HasApplicableVisualizer", candidates);
    for candidate in candidates {
        graph.edge(*candidate, "DescribedBy", &[role]);
    }
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        Arc::new(graph),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let result = dahn_selection::select_visualizer(
        &context,
        VisualizerSelectionRequest {
            subject: Graph::reference(&context, subject),
            requested_kind: kind,
            parent_visualizer: None,
        },
    )?;
    Ok(result.selected.holon_id()?.local_id().0[0])
}
#[test]
fn selects_each_presentation_role_directly_and_through_inheritance() {
    for kind in [
        VisualizerKind::Properties,
        VisualizerKind::Property,
        VisualizerKind::Value,
        VisualizerKind::Action,
    ] {
        for inherited in [false, true] {
            assert_eq!(select(kind, &[20], inherited, &[4]).unwrap(), 20);
        }
    }
}
#[test]
fn reports_missing_and_ambiguous_candidates_for_every_role() {
    for kind in [
        VisualizerKind::Properties,
        VisualizerKind::Property,
        VisualizerKind::Value,
        VisualizerKind::Action,
    ] {
        assert!(
            matches!(select(kind, &[], false, &[4]), Err(HolonError::NotImplemented(message)) if message.contains("No applicable"))
        );
        assert!(matches!(
            select(kind, &[20, 22], false, &[4]),
            Err(HolonError::MultipleRelatedHolons { count: 2, .. })
        ));
    }
}
#[test]
fn value_selection_requires_exactly_one_declared_value_type() {
    assert!(select(VisualizerKind::Value, &[20], false, &[]).is_err());
    assert!(matches!(
        select(VisualizerKind::Value, &[20], false, &[4, 6]),
        Err(HolonError::MultipleRelatedHolons { count: 2, .. })
    ));
}

fn select_collection(
    count: usize,
    slot: u8,
    accepts_collection: bool,
    compatible_member: bool,
) -> Result<map_commands_contract::VisualizerSelection, HolonError> {
    let mut graph = Graph::default();
    graph.edge(1, "HasSlot", &[2, 3]);
    graph.edge(2, "AcceptsVisualizerType", &[if accepts_collection { 14 } else { 11 }]);
    graph.edge(3, "AcceptsVisualizerType", &[14]);
    graph.edge(20, "DescribedBy", &[14]);
    graph.edge(5, "DescribedBy", &[if compatible_member { 4 } else { 6 }]);
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        Arc::new(graph),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let collection = map_commands_contract::DescribedHolonCollection {
        members: HolonCollection::from_parts(
            holons_core::CollectionState::Fetched,
            (0..count).map(|_| Graph::reference(&context, 5)).collect(),
            Default::default(),
        ),
        element_type: Graph::reference(&context, 4),
    };
    dahn_selection::select_collection_visualizer(
        &context,
        collection,
        Graph::reference(&context, 1),
        Graph::reference(&context, slot),
    )
}

#[test]
fn selects_described_collections_including_empty_and_single_member() {
    for count in [0, 1, 3] {
        let result = select_collection(count, 2, true, true).unwrap();
        assert_eq!(result.requested_kind, VisualizerKind::Collection);
        assert_eq!(result.selected.holon_id().unwrap().local_id().0[0], 20);
    }
}

#[test]
fn collection_selection_checks_the_destination_slot_not_any_parent_slot() {
    assert!(select_collection(0, 2, false, true).is_err());
    assert!(select_collection(0, 9, true, true).is_err());
    assert!(select_collection(1, 2, true, false).is_err());
}
