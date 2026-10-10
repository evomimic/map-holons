use base_types::{BaseValue, MapEnumValue, MapInteger, MapString};
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
use map_commands_contract::{VisualizerKind, VisualizerOwner, VisualizerSelectionRequest};
use std::{any::Any, collections::HashMap, sync::Arc};

/// A persisted graph fixture accessed through the real bound reference layer.
#[derive(Debug, Default)]
struct Graph {
    edges: HashMap<(u8, String), Vec<u8>>,
    anchors: Vec<u8>,
    holon_root: Option<u8>,
    unavailable: Vec<u8>,
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
    fn visualizer_artifact_available_internal(
        &self,
        _: &Arc<TransactionContext>,
        candidate: &HolonReference,
    ) -> Result<bool, HolonError> {
        Ok(!self.unavailable.contains(&candidate.holon_id()?.local_id().0[0]))
    }
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
            "TableCollectionVisualizer.CollectionVisualizer" => 20,
            "Dancer.HolonType" => 40,
            "RootedNavigationVisualizer.HolonType" => 41,
            _ => panic!("unexpected canonical role lookup {key}"),
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
            PropertyName("VisualizerImplementationRuntime".into()),
            BaseValue::EnumValue(MapEnumValue("TypeScript".into())),
        );
        for (name, value) in [
            ("VisualizerModuleFormat", "ESModule"),
            ("VisualizerImplementationKey", "fixture.implementation"),
            ("Entrypoint", "default"),
            ("VisualizerArtifactDigest", "sha256:fixture"),
        ] {
            properties.insert(PropertyName(name.into()), BaseValue::StringValue(value.into()));
        }
        properties.insert(
            PropertyName("DefinesInstanceTypeKind".into()),
            BaseValue::BooleanValue(self.anchors.contains(&id.local_id().0[0]).into()),
        );
        properties.insert(
            PropertyName("Key".into()),
            BaseValue::StringValue(MapString(if self.holon_root == Some(id.local_id().0[0]) {
                "HolonType.TypeDescriptor".into()
            } else {
                format!("node-{}", id.local_id().0[0])
            })),
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
                .map(|id| {
                    if self.holon_root == Some(*id) {
                        HolonReference::smart_with_key(
                            context.space_read_handle(),
                            HolonId::Local(LocalId(vec![*id])),
                            MapString("HolonType.TypeDescriptor".into()),
                        )
                    } else {
                        Self::reference(context, *id)
                    }
                })
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
        VisualizerKind::PropertyMap => (1, 2, 10),
        VisualizerKind::ActionBar => (1, 2, 13),
        VisualizerKind::Action => (6, 6, 14),
        VisualizerKind::Property => (3, 3, 11),
        VisualizerKind::Value => (3, 4, 12),
        _ => unreachable!(),
    };
    graph.edge(30, "AcceptsVisualizerType", &[role]);
    graph.edge(40, "HasSlot", &[30]);
    graph.edge(60, "ForMetaDesignSystem", &[61]);
    for candidate in [20, 21, 22] {
        graph.edge(candidate, "ImplementedBy", &[70]);
    }
    if inherited {
        graph.edge(start, "HasApplicableVisualizer", &[]);
        graph.edge(start, "Extends", &[5]);
    }
    graph.edge(if inherited { 5 } else { start }, "HasApplicableVisualizer", candidates);
    for candidate in candidates {
        graph.edge(*candidate, "DescribedBy", &[role]);
    }
    graph.edge(60, "ForMetaDesignSystem", &[61]);
    for candidate in [20, 21, 22] {
        graph.edge(candidate, "ImplementedBy", &[70]);
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
            owner: VisualizerOwner::Visualizer(Graph::reference(&context, 40)),
            theme: Graph::reference(&context, 60),
            slot: Graph::reference(&context, 30),
        },
    )?;
    Ok(result.selected.holon_id()?.local_id().0[0])
}
#[test]
fn selects_each_presentation_role_directly_and_through_inheritance() {
    for kind in [
        VisualizerKind::PropertyMap,
        VisualizerKind::Property,
        VisualizerKind::Value,
        VisualizerKind::ActionBar,
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
        VisualizerKind::PropertyMap,
        VisualizerKind::Property,
        VisualizerKind::Value,
        VisualizerKind::ActionBar,
        VisualizerKind::Action,
    ] {
        assert!(
            matches!(select(kind, &[], false, &[4]), Err(HolonError::NotImplemented(message)) if message.contains("No viable"))
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
    graph.edge(60, "ForMetaDesignSystem", &[61]);
    for candidate in [20, 21, 22] {
        graph.edge(candidate, "ImplementedBy", &[70]);
    }
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
}

#[test]
fn collection_selection_does_not_validate_member_types_on_read() {
    assert!(select_collection(800, 2, true, false).is_ok());
}

fn slot_select(graph: Graph, slot: u8, parent: Option<u8>) -> Result<u8, HolonError> {
    slot_select_kind(graph, slot, parent, VisualizerKind::Property)
}

fn slot_select_kind(
    mut graph: Graph,
    slot: u8,
    parent: Option<u8>,
    kind: VisualizerKind,
) -> Result<u8, HolonError> {
    graph.edge(60, "ForMetaDesignSystem", &[61]);
    for candidate in [20, 21, 22] {
        graph.edge(candidate, "ImplementedBy", &[70]);
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
            subject: Graph::reference(&context, 1),
            requested_kind: kind,
            slot: Graph::reference(&context, slot),
            owner: VisualizerOwner::Visualizer(Graph::reference(&context, parent.unwrap_or(40))),
            theme: Graph::reference(&context, 60),
        },
    )?;
    Ok(result.selected.holon_id()?.local_id().0[0])
}

fn slot_graph() -> Graph {
    let mut graph = Graph::default();
    graph.edge(1, "Extends", &[2]);
    graph.edge(2, "Extends", &[3]);
    graph.anchors.extend([2, 3]);
    graph.holon_root = Some(3);
    graph.edge(30, "AcceptsVisualizerType", &[10]);
    graph.edge(31, "AcceptsVisualizerType", &[11]);
    graph.edge(40, "HasSlot", &[30]);
    graph.edge(20, "DescribedBy", &[12]);
    graph.edge(12, "Extends", &[10]);
    graph.edge(21, "DescribedBy", &[11]);
    graph.edge(22, "DescribedBy", &[10]);
    graph
}

#[test]
fn slot_selection_filters_by_accepted_type_and_prefers_leaf() {
    let mut graph = slot_graph();
    graph.edge(1, "HasApplicableVisualizer", &[21, 20]);
    graph.edge(2, "HasApplicableVisualizer", &[22]);
    assert_eq!(slot_select(graph, 30, Some(40)).unwrap(), 20);
}

#[test]
fn slot_selection_prefers_eligible_family_default_and_falls_back_to_holon_type() {
    let mut graph = slot_graph();
    graph.edge(1, "HasApplicableVisualizer", &[21]);
    graph.edge(2, "HasApplicableVisualizer", &[20]);
    graph.edge(3, "HasApplicableVisualizer", &[22]);
    assert_eq!(slot_select(graph, 30, None).unwrap(), 20);
    let mut graph = slot_graph();
    // An ineligible family-level candidate does not block the HolonType default.
    graph.edge(2, "HasApplicableVisualizer", &[21]);
    graph.edge(3, "HasApplicableVisualizer", &[20]);
    assert_eq!(slot_select(graph, 30, None).unwrap(), 20);
}

#[test]
fn slot_selection_never_searches_above_holon_type() {
    let mut graph = slot_graph();
    graph.edge(3, "Extends", &[4]);
    graph.edge(4, "HasApplicableVisualizer", &[20]);
    assert!(matches!(slot_select(graph, 30, None),
        Err(HolonError::NotImplemented(message)) if message.contains("permitted descriptor lineage")));
}

#[test]
fn slot_selection_does_not_bypass_ambiguous_family_candidates_for_a_generic_default() {
    let mut graph = slot_graph();
    graph.edge(2, "HasApplicableVisualizer", &[20, 22]);
    graph.edge(3, "HasApplicableVisualizer", &[20]);
    assert!(matches!(
        slot_select(graph, 30, None),
        Err(HolonError::MultipleRelatedHolons { count: 2, .. })
    ));
}

#[test]
fn slot_selection_rejects_foreign_slots_empty_contracts_and_ambiguity() {
    let mut graph = slot_graph();
    graph.edge(1, "HasApplicableVisualizer", &[20]);
    assert!(matches!(slot_select(graph, 31, Some(40)), Err(HolonError::InvalidParameter(_))));
    assert!(matches!(slot_select(slot_graph(), 99, None), Err(HolonError::InvalidParameter(_))));
    let mut graph = slot_graph();
    graph.edge(1, "HasApplicableVisualizer", &[20, 22]);
    assert!(matches!(
        slot_select(graph, 30, None),
        Err(HolonError::MultipleRelatedHolons { count: 2, .. })
    ));
}

#[test]
fn node_selection_starts_at_described_type_and_falls_back_through_extends() {
    for specialized in [true, false] {
        let mut graph = slot_graph();
        // Subject 1 is an ordinary response; descriptor 4 is its concrete type.
        graph.edge(1, "DescribedBy", &[4]);
        graph.edge(4, "Extends", &[2]);
        graph.edge(2, "HasApplicableVisualizer", &[21]);
        graph.edge(3, "HasApplicableVisualizer", &[22]);
        if specialized {
            graph.edge(4, "HasApplicableVisualizer", &[20]);
        }
        // Applicability on the subject itself must not control Node selection.
        graph.edge(1, "HasApplicableVisualizer", &[21]);
        assert_eq!(
            slot_select_kind(graph, 30, Some(40), VisualizerKind::Node).unwrap(),
            if specialized { 20 } else { 22 }
        );
    }
}

#[test]
fn home_dancer_selection_preserves_navigation_with_an_auxiliary_slot() {
    let mut graph = Graph::default();
    graph.edge(1, "DescribedBy", &[2]);
    graph.edge(44, "DescribedBy", &[45]);
    graph.edge(44, "AffordsDancer", &[43]);
    graph.edge(43, "DescribedBy", &[40]);
    graph.edge(43, "HasExperienceVisualizerSlot", &[50, 47]);
    graph.edge(50, "AcceptsVisualizerType", &[42]);
    graph.edge(47, "AcceptsVisualizerType", &[41]);
    graph.edge(45, "HasApplicableVisualizer", &[46, 49, 51]);
    graph.edge(46, "DescribedBy", &[41]);
    graph.edge(46, "HasSlot", &[48]);
    graph.edge(48, "AcceptsVisualizerType", &[10]);
    graph.edge(49, "DescribedBy", &[10]);
    graph.edge(51, "DescribedBy", &[42]);
    graph.edge(60, "ForMetaDesignSystem", &[61]);
    for candidate in [20, 21, 22] {
        graph.edge(candidate, "ImplementedBy", &[70]);
    }
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        Arc::new(graph),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    let context = space.get_transaction_manager().open_public_transaction(space.clone()).unwrap();
    let selected = dahn_selection::select_home_dancer(
        &context,
        dahn_selection::HomeDancerSelectionContext {
            active_holon_space: Graph::reference(&context, 44),
            selected_theme: Graph::reference(&context, 1),
            selected_meta_design_system: Graph::reference(&context, 1),
            runtime: dahn_selection::HomeDancerRuntime::Local,
            person: None,
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(selected.dancer.holon_id().unwrap().local_id().0, vec![43]);
    assert_eq!(selected.rooted_navigation_visualizer.holon_id().unwrap().local_id().0, vec![46]);
    assert_eq!(selected.root_node_visualizer.holon_id().unwrap().local_id().0, vec![49]);
}

fn candidate_context(mut graph: Graph) -> Arc<TransactionContext> {
    graph.edge(60, "ForMetaDesignSystem", &[61]);
    for candidate in [20, 21, 22] {
        graph.edge(candidate, "ImplementedBy", &[70]);
    }
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        Arc::new(graph),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    space.get_transaction_manager().open_public_transaction(space.clone()).unwrap()
}
fn candidate_request(context: &Arc<TransactionContext>) -> VisualizerSelectionRequest {
    VisualizerSelectionRequest {
        subject: Graph::reference(context, 1),
        requested_kind: VisualizerKind::Property,
        owner: VisualizerOwner::Visualizer(Graph::reference(context, 40)),
        slot: Graph::reference(context, 30),
        theme: Graph::reference(context, 60),
    }
}
#[test]
fn discovery_retains_general_alternatives_and_all_declaration_provenance() {
    let mut graph = slot_graph();
    graph.edge(1, "HasApplicableVisualizer", &[20, 20]);
    graph.edge(2, "HasApplicableVisualizer", &[20, 22]);
    graph.edge(3, "HasApplicableVisualizer", &[22]);
    // The HolonType parent must not be read by discovery.
    graph.edge(3, "Extends", &[4, 5]);
    let context = candidate_context(graph);
    let discovery = dahn_selection::discover_visualizers(
        &context,
        candidate_request(&context),
        Some(Graph::reference(&context, 22)),
    )
    .unwrap();
    assert_eq!(discovery.ancestry.len(), 3);
    assert_eq!(
        discovery.stop_reason,
        map_commands_contract::DiscoveryStopReason::HolonTypeBoundary
    );
    assert_eq!(discovery.candidates.len(), 2);
    assert_eq!(discovery.candidates[0].declared_on.len(), 2);
    assert_eq!(discovery.candidates[1].declared_on.len(), 2);
    assert_eq!(
        discovery.current_selection.unwrap().assessment,
        map_commands_contract::VisualizerAssessment::Viable
    );
    assert_eq!(
        dahn_selection::select_visualizer(&context, candidate_request(&context)).unwrap().selected,
        Graph::reference(&context, 20)
    );
    assert_eq!(
        dahn_selection::choose_visualizer(
            &context,
            candidate_request(&context),
            Graph::reference(&context, 22)
        )
        .unwrap()
        .selected,
        Graph::reference(&context, 22)
    );
}
#[test]
fn explicit_choice_survives_automatic_ambiguity_but_not_theme_incompatibility() {
    let mut graph = slot_graph();
    graph.edge(1, "HasApplicableVisualizer", &[20, 22]);
    let context = candidate_context(graph);
    assert!(matches!(
        dahn_selection::select_visualizer(&context, candidate_request(&context)),
        Err(HolonError::MultipleRelatedHolons { .. })
    ));
    assert!(dahn_selection::choose_visualizer(
        &context,
        candidate_request(&context),
        Graph::reference(&context, 22)
    )
    .is_ok());
    // A later command sees changed facts; the earlier choice is no authorization token.
    let mut graph = slot_graph();
    graph.edge(1, "HasApplicableVisualizer", &[20, 22]);
    graph.edge(22, "ConsumesDesignToken", &[80]);
    let context = candidate_context(graph);
    let discovery = dahn_selection::discover_visualizers(
        &context,
        candidate_request(&context),
        Some(Graph::reference(&context, 22)),
    )
    .unwrap();
    assert_eq!(
        discovery.current_selection.unwrap().assessment,
        map_commands_contract::VisualizerAssessment::IncompatibleTheme
    );
    assert!(dahn_selection::choose_visualizer(
        &context,
        candidate_request(&context),
        Graph::reference(&context, 22)
    )
    .is_err());
}
#[test]
fn discovery_separates_a_no_longer_applicable_current_choice() {
    let context = candidate_context(slot_graph());
    let discovery = dahn_selection::discover_visualizers(
        &context,
        candidate_request(&context),
        Some(Graph::reference(&context, 20)),
    )
    .unwrap();
    assert!(discovery.candidates.is_empty());
    let current = discovery.current_selection.unwrap();
    assert!(current.declared_on.is_empty());
    assert_eq!(current.assessment, map_commands_contract::VisualizerAssessment::NoLongerApplicable);
    assert!(dahn_selection::choose_visualizer(
        &context,
        candidate_request(&context),
        current.visualizer
    )
    .is_err());
}
#[test]
fn automatic_success_is_lazy_but_discovery_reports_malformed_ancestry() {
    let mut graph = slot_graph();
    graph.edge(1, "HasApplicableVisualizer", &[20]);
    graph.edge(1, "Extends", &[2, 3]);
    let context = candidate_context(graph);
    assert!(dahn_selection::select_visualizer(&context, candidate_request(&context)).is_ok());
    assert!(
        dahn_selection::discover_visualizers(&context, candidate_request(&context), None).is_err()
    );
}
#[test]
fn dancer_owner_uses_experience_slots_and_theme_tokens_are_exact_identities() {
    let mut graph = slot_graph();
    graph.edge(40, "HasSlot", &[]);
    graph.edge(40, "HasExperienceVisualizerSlot", &[30]);
    graph.edge(1, "HasApplicableVisualizer", &[20]);
    graph.edge(20, "ConsumesDesignToken", &[80]);
    graph.edge(61, "DefinesDesignToken", &[80]);
    let context = candidate_context(graph);
    let mut request = candidate_request(&context);
    assert!(dahn_selection::select_visualizer(&context, request).is_err());
    request = candidate_request(&context);
    request.owner = VisualizerOwner::Dancer(Graph::reference(&context, 40));
    assert!(dahn_selection::select_visualizer(&context, request).is_ok());
}

#[test]
fn unavailable_local_artifacts_are_visible_but_cannot_be_chosen() {
    let mut graph = slot_graph();
    graph.edge(1, "HasApplicableVisualizer", &[20]);
    graph.edge(2, "HasApplicableVisualizer", &[22]);
    graph.unavailable.push(20);
    let context = candidate_context(graph);
    let discovery =
        dahn_selection::discover_visualizers(&context, candidate_request(&context), None).unwrap();
    assert_eq!(
        discovery.candidates[0].assessment,
        map_commands_contract::VisualizerAssessment::ImplementationUnavailable
    );
    assert_eq!(
        dahn_selection::select_visualizer(&context, candidate_request(&context)).unwrap().selected,
        Graph::reference(&context, 22)
    );
    assert!(dahn_selection::choose_visualizer(
        &context,
        candidate_request(&context),
        Graph::reference(&context, 20)
    )
    .is_err());
}
#[test]
fn malformed_implementation_is_an_evaluation_error_not_a_rejection() {
    let mut graph = slot_graph();
    graph.edge(1, "HasApplicableVisualizer", &[20]);
    let context = candidate_context(graph);
    // Fresh relationship reads allow changed declarations to be tested through the reference layer.
    let mut malformed = slot_graph();
    malformed.edge(1, "HasApplicableVisualizer", &[20]);
    malformed.edge(60, "ForMetaDesignSystem", &[61]);
    malformed.edge(20, "ImplementedBy", &[70, 71]);
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        Arc::new(malformed),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    let malformed_context =
        space.get_transaction_manager().open_public_transaction(space.clone()).unwrap();
    assert!(
        dahn_selection::discover_visualizers(&context, candidate_request(&context), None).is_ok()
    );
    assert!(matches!(
        dahn_selection::discover_visualizers(
            &malformed_context,
            candidate_request(&malformed_context),
            None
        ),
        Err(HolonError::MultipleRelatedHolons { .. })
    ));
}

#[test]
fn node_discovery_uses_the_subjects_described_type_and_allows_a_general_choice() {
    let mut graph = slot_graph();
    graph.edge(1, "DescribedBy", &[4]);
    graph.edge(1, "HasApplicableVisualizer", &[22]);
    graph.edge(4, "Extends", &[2]);
    graph.edge(4, "HasApplicableVisualizer", &[20]);
    graph.edge(3, "HasApplicableVisualizer", &[22]);
    let context = candidate_context(graph);
    let request = || {
        let mut request = candidate_request(&context);
        request.requested_kind = VisualizerKind::Node;
        request
    };
    let discovery = dahn_selection::discover_visualizers(&context, request(), None).unwrap();
    assert_eq!(discovery.ancestry[0], Graph::reference(&context, 4));
    assert_eq!(
        discovery.stop_reason,
        map_commands_contract::DiscoveryStopReason::HolonTypeBoundary
    );
    assert_eq!(discovery.candidates.len(), 2);
    assert_eq!(discovery.candidates[1].declared_on.len(), 1);
    assert_eq!(
        dahn_selection::select_visualizer(&context, request()).unwrap().selected,
        Graph::reference(&context, 20)
    );
    assert_eq!(
        dahn_selection::choose_visualizer(&context, request(), Graph::reference(&context, 22))
            .unwrap()
            .selected,
        Graph::reference(&context, 22)
    );
}

#[test]
fn structure_discovery_reports_actual_lineage_exhaustion_without_node_requirements() {
    let mut graph = slot_graph();
    graph.holon_root = None;
    graph.edge(1, "DescribedBy", &[2]);
    graph.edge(3, "Extends", &[]);
    let context = candidate_context(graph);
    let mut request = candidate_request(&context);
    request.requested_kind = VisualizerKind::Structure;
    let discovery = dahn_selection::discover_visualizers(&context, request, None).unwrap();
    assert_eq!(discovery.stop_reason, map_commands_contract::DiscoveryStopReason::LineageExhausted);
    assert_eq!(discovery.ancestry[0], Graph::reference(&context, 2));
}
