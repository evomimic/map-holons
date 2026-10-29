use holons_core::core_shared_objects::transient_holon_manager::ToHolonCloneModel;
use std::{
    any::Any,
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use base_types::MapString;
use core_types::{HolonError, HolonId, LocalId, RelationshipName};
use holons_core::core_shared_objects::{
    space_manager::HolonSpaceManager, transactions::TransactionContext, Holon, WritableRelationship,
};
use holons_core::{
    HolonCollection, HolonCollectionApi, HolonReference, HolonServiceApi, ReadableHolon,
    RelationshipMap, ServiceRoutingPolicy, StagedReference, TransientReference, WritableHolon,
};
use type_names::{CoreRelationshipTypeName, CoreValidationRuleName};

/// Explicit saved snapshots only; unexpected writes and unbounded reads fail tests.
#[derive(Debug, Default)]
struct FixtureStorage {
    holons: HashMap<HolonId, holons_core::core_shared_objects::holon::SavedHolon>,
    relationships: HashMap<(HolonId, RelationshipName), Vec<HolonId>>,
    reads: std::sync::atomic::AtomicUsize,
    node_reads: std::sync::atomic::AtomicUsize,
    key_lookup_reads: std::sync::atomic::AtomicUsize,
    lineage_reads: std::sync::atomic::AtomicUsize,
    keyed_lineages: std::sync::Mutex<HashMap<MapString, Vec<LocalId>>>,
    lineage_read_error: std::sync::Mutex<Option<HolonError>>,
    read_errors: HashMap<HolonId, HolonError>,
    relationship_read_overrides:
        std::sync::Mutex<HashMap<(HolonId, RelationshipName), Option<HolonError>>>,
    reads_by_relationship: std::sync::Mutex<HashMap<RelationshipName, usize>>,
}
impl HolonServiceApi for FixtureStorage {
    fn relationship_cache_policy(
        &self,
        _: &Arc<TransactionContext>,
        source: &HolonId,
        name: &RelationshipName,
    ) -> Result<Option<holons_core::RelationshipCachePolicy>, HolonError> {
        if self
            .relationship_read_overrides
            .lock()
            .unwrap()
            .contains_key(&(source.clone(), name.clone()))
        {
            return Ok(Some(holons_core::RelationshipCachePolicy::Fresh));
        }
        Ok(Some(holons_core::RelationshipCachePolicy::Reuse))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    fn commit_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &[StagedReference],
    ) -> Result<TransientReference, HolonError> {
        panic!("validator must not commit")
    }
    fn delete_holon_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &LocalId,
    ) -> Result<(), HolonError> {
        panic!("validator must not delete")
    }
    fn fetch_all_related_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
        source: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        let mut result = RelationshipMap::new_empty();
        for (id, name) in self.relationships.keys() {
            if id == source {
                let collection = self.fetch_related_holons_internal(context, source, name)?;
                result.insert(name.clone(), Arc::new(std::sync::RwLock::new(collection)));
            }
        }
        Ok(result)
    }
    fn fetch_holon_internal(
        &self,
        _: &Arc<TransactionContext>,
        id: &HolonId,
    ) -> Result<Holon, HolonError> {
        self.node_reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if let Some(error) = self.read_errors.get(id) {
            return Err(error.clone());
        }
        Ok(Holon::Saved(self.holons.get(id).expect("unexpected storage read").clone()))
    }
    fn fetch_related_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
        source: &HolonId,
        name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
        self.reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        *self.reads_by_relationship.lock().unwrap().entry(name.clone()).or_default() += 1;
        if let Some(Some(error)) =
            self.relationship_read_overrides.lock().unwrap().get(&(source.clone(), name.clone()))
        {
            return Err(error.clone());
        }
        assert!(self.holons.contains_key(source), "unexpected storage relationship read");
        let mut collection = HolonCollection::new_transient();
        if let Some(targets) = self.relationships.get(&(source.clone(), name.clone())) {
            collection.add_references(
                targets
                    .iter()
                    .map(|id| {
                        HolonReference::smart_from_id(context.space_read_handle(), id.clone())
                    })
                    .collect(),
            )?;
        }
        Ok(collection)
    }
    fn get_all_holons_internal(
        &self,
        _: &Arc<TransactionContext>,
    ) -> Result<HolonCollection, HolonError> {
        panic!("unexpected unbounded traversal")
    }
    fn load_holons_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: TransientReference,
    ) -> Result<TransientReference, HolonError> {
        panic!("validator must not load")
    }
    fn get_saved_lineage_roots_by_key_internal(
        &self,
        _: &Arc<TransactionContext>,
        key: &MapString,
    ) -> Result<Vec<LocalId>, HolonError> {
        self.lineage_reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if let Some(error) = &*self.lineage_read_error.lock().unwrap() {
            return Err(error.clone());
        }
        Ok(self.keyed_lineages.lock().unwrap().get(key).cloned().unwrap_or_default())
    }

    fn get_saved_holon_by_key_internal(
        &self,
        context: &Arc<TransactionContext>,
        key: &MapString,
    ) -> Result<holons_core::SmartReference, HolonError> {
        self.key_lookup_reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        use type_names::ToPropertyName;
        for (id, holon) in &self.holons {
            if holon.property_map().get(&"Key".to_property_name())
                == Some(&base_types::BaseValue::StringValue(key.clone()))
            {
                return Ok(holons_core::SmartReference::new_from_id(
                    context.space_read_handle(),
                    id.clone(),
                ));
            }
        }
        Err(HolonError::HolonNotFound(key.to_string()))
    }
}

/// A small graph with real bound references and the canonical rule anchors.
pub(super) struct Fixture {
    pub context: Arc<TransactionContext>,
    pub nodes: BTreeMap<String, HolonReference>,
    saved_storage: Option<Arc<FixtureStorage>>,
}

pub(super) fn subject_rules(
) -> impl Iterator<Item = (CoreValidationRuleName, &'static str, &'static str)> {
    CoreValidationRuleName::all().filter_map(|name| {
        let registration = crate::registries::registration(name);
        matches!(
            registration.implementation,
            Some(crate::registries::RuleImplementation::Subject(_))
        )
        .then_some((name, registration.family.key(), registration.declaring.key()))
    })
}

impl Fixture {
    /// Opens an empty schema snapshot for bootstrap precondition tests.
    pub fn empty() -> Result<Self, HolonError> {
        let manager = Arc::new(HolonSpaceManager::new_with_managers(
            None,
            Arc::new(FixtureStorage::default()),
            None,
            ServiceRoutingPolicy::BlockExternal,
        ));
        let context = manager.get_transaction_manager().open_public_transaction(manager.clone())?;
        Ok(Self { context, nodes: BTreeMap::new(), saved_storage: None })
    }

    pub fn new() -> Result<Self, HolonError> {
        let mut fixture = Self::empty()?;
        for key in [
            "TypeDescriptor",
            "MetaTypeDescriptor.HolonType",
            "BaseValueValueType.ValueType",
            "ValueArrayValueType.ValueType",
            "Contract",
            "Title.PropertyType",
            "Key.PropertyType",
        ] {
            fixture.node(key)?;
        }
        let mut binding_roots = std::collections::HashSet::new();
        for (rule, family, target) in subject_rules() {
            fixture.node(rule.as_str())?;
            fixture.node(family)?;
            fixture.node(target)?;
            fixture.link(rule.as_str(), CoreRelationshipTypeName::DescribedBy, family)?;
            if binding_roots.insert(target) {
                fixture.link(target, CoreRelationshipTypeName::Extends, "TypeDescriptor")?;
            }
            fixture.link(target, CoreRelationshipTypeName::ValidationBindings, rule.as_str())?;
        }
        // Product rules are available to tests that explicitly bind them.
        for rule in CoreValidationRuleName::all() {
            if matches!(
                crate::registries::registration(rule).implementation,
                Some(
                    crate::registries::RuleImplementation::DescriptorProduct
                        | crate::registries::RuleImplementation::SchemaAggregate(_)
                )
            ) {
                fixture.node(rule.as_str())?;
            }
        }
        fixture.node("Schema.HolonType")?;
        fixture.link("Contract", CoreRelationshipTypeName::Extends, "HolonType.TypeDescriptor")?;
        for (key, name) in [("Title.PropertyType", "Title"), ("Key.PropertyType", "Key")] {
            fixture.nodes.get_mut(key).unwrap().with_property_value("TypeName", name)?;
            fixture.link(key, CoreRelationshipTypeName::Extends, "PropertyType.TypeDescriptor")?;
            fixture.link(key, CoreRelationshipTypeName::ValueType, "StringValueType.ValueType")?;
            fixture.link("Contract", CoreRelationshipTypeName::InstanceProperties, key)?;
        }
        fixture
            .nodes
            .get_mut("Title.PropertyType")
            .unwrap()
            .with_property_value("IsValueRequired", true)?;
        // Commit candidates carry DescribedBy; license it through the inherited
        // authored contract without relying on any materialized inverse index.
        fixture.node("DeclaredRelationshipType")?;
        fixture.node("DescribedBy.Relationship")?;
        fixture
            .nodes
            .get_mut("DescribedBy.Relationship")
            .unwrap()
            .with_property_value("TypeName", "DescribedBy")?;
        fixture.link(
            "DescribedBy.Relationship",
            CoreRelationshipTypeName::Extends,
            "DeclaredRelationshipType",
        )?;
        fixture.link(
            "HolonType.TypeDescriptor",
            CoreRelationshipTypeName::InstanceRelationships,
            "DescribedBy.Relationship",
        )?;
        Ok(fixture)
    }

    pub fn node(&mut self, key: &str) -> Result<HolonReference, HolonError> {
        if let Some(node) = self.nodes.get(key) {
            return Ok(node.clone());
        }
        let mut node = self.context.mutation().new_holon(Some(MapString(key.into())))?;
        node.with_property_value("TypeName", key)?
            .with_property_value("IsAbstractType", false)?
            .with_property_value("IsValueRequired", false)?;
        if [
            "StringValueType.ValueType",
            "IntegerValueType.ValueType",
            "BooleanValueType.ValueType",
            "BytesValueType.ValueType",
            "EnumValueType.ValueType",
            "BaseValueValueType.ValueType",
            "ValueArrayValueType.ValueType",
        ]
        .contains(&key)
        {
            node.with_property_value("TypeName", key.split('.').next().unwrap())?
                .with_property_value("DefinesInstanceTypeKind", true)?;
        }
        // Stage before adding describing types, keeping fixture construction separate
        // from input completion and the report-only behavior under test.
        let node: HolonReference = self.context.mutation().stage_new_holon(node)?.into();
        self.nodes.insert(key.into(), node.clone());
        Ok(node)
    }

    pub fn link(
        &mut self,
        source: &str,
        relationship: CoreRelationshipTypeName,
        target: &str,
    ) -> Result<(), HolonError> {
        let target = self.nodes[target].clone();
        self.nodes.get_mut(source).unwrap().add_related_holons(relationship, vec![target])?;
        Ok(())
    }

    /// Stages and describes a subject. Contract declares no Title default, so
    /// attachment leaves Title absent until the test supplies it.
    pub fn staged_subject(&self, key: &str) -> Result<StagedReference, HolonError> {
        let subject = self.context.mutation().new_holon(Some(MapString(key.into())))?;
        let mut subject = self.context.mutation().stage_new_holon(subject)?;
        subject.with_descriptor(self.nodes["Contract"].clone())?;
        Ok(subject)
    }
}

impl Fixture {
    pub fn lineage_lookup_reads(&self) -> usize {
        self.saved_storage
            .as_ref()
            .map_or(0, |storage| storage.lineage_reads.load(std::sync::atomic::Ordering::Relaxed))
    }

    pub fn fail_lineage_lookup(&self, error: HolonError) {
        *self.saved_storage.as_ref().unwrap().lineage_read_error.lock().unwrap() = Some(error);
    }

    /// Assigns record-derived roots to saved fixture versions, retaining each version's
    /// key as historical association evidence for its lineage.
    pub fn saved_snapshot_with_lineages(&self, roots: &[(&str, &str)]) -> Result<Self, HolonError> {
        self.saved_snapshot_configured(&[], roots)
    }

    /// Simulates published records whose keyed Owns maintenance has not completed.
    pub fn remove_saved_key_association(&self, key: &str) {
        self.saved_storage
            .as_ref()
            .unwrap()
            .keyed_lineages
            .lock()
            .unwrap()
            .remove(&MapString(key.into()));
    }

    pub fn backend_relationship_reads(&self) -> usize {
        self.saved_storage
            .as_ref()
            .map_or(0, |storage| storage.reads.load(std::sync::atomic::Ordering::Relaxed))
    }

    /// Uses fresh reads for one saved edge, optionally injecting a backend failure.
    pub fn control_saved_relationship_reads(
        &self,
        key: &str,
        edge: CoreRelationshipTypeName,
        error: Option<HolonError>,
    ) -> Result<(), HolonError> {
        let source = self.nodes[key].holon_id()?;
        self.saved_storage
            .as_ref()
            .expect("saved fixture")
            .relationship_read_overrides
            .lock()
            .unwrap()
            .insert((source, edge.as_relationship_name()), error);
        Ok(())
    }

    /// Counts backend node fetch attempts, including failed reads.
    pub fn backend_node_reads(&self) -> usize {
        self.saved_storage
            .as_ref()
            .map_or(0, |storage| storage.node_reads.load(std::sync::atomic::Ordering::Relaxed))
    }

    /// Counts saved definition lookups by key, separately from lineage evidence reads.
    pub fn saved_key_lookup_reads(&self) -> usize {
        self.saved_storage.as_ref().map_or(0, |storage| {
            storage.key_lookup_reads.load(std::sync::atomic::Ordering::Relaxed)
        })
    }

    pub fn backend_reads_for(&self, edge: &CoreRelationshipTypeName) -> usize {
        self.backend_reads_for_name(&edge.as_relationship_name())
    }

    pub fn backend_reads_for_name(&self, name: &RelationshipName) -> usize {
        self.saved_storage.as_ref().map_or(0, |storage| {
            *storage.reads_by_relationship.lock().unwrap().get(name).unwrap_or(&0)
        })
    }

    /// Saves the fixture graph as service snapshots and opens a fresh empty Nursery.
    /// All relationship endpoints are rebound as saved references on read.
    pub fn saved_snapshot(&self) -> Result<Self, HolonError> {
        self.saved_snapshot_with_read_errors(&[])
    }

    /// Injects backend failures while preserving ordinary bound-reference traversal.
    pub fn saved_snapshot_with_read_errors(
        &self,
        failures: &[(&str, HolonError)],
    ) -> Result<Self, HolonError> {
        self.saved_snapshot_configured(failures, &[])
    }

    fn saved_snapshot_configured(
        &self,
        failures: &[(&str, HolonError)],
        roots: &[(&str, &str)],
    ) -> Result<Self, HolonError> {
        use holons_core::core_shared_objects::holon::SavedHolon;
        let ids: BTreeMap<_, _> = self
            .nodes
            .keys()
            .enumerate()
            .map(|(index, key)| (key.clone(), HolonId::Local(LocalId(vec![index as u8 + 1; 39]))))
            .collect();
        let reference_ids: HashMap<_, _> = self
            .nodes
            .iter()
            .map(|(key, reference)| (reference.clone(), ids[key].clone()))
            .collect();
        let mut storage = FixtureStorage::default();
        for (key, reference) in &self.nodes {
            let model = reference.holon_clone_model()?;
            let id = ids[key].clone();
            let root = roots
                .iter()
                .find(|(version, _)| *version == key)
                .map(|(_, root)| ids[*root].local_id().clone());
            if let Some(key) = reference.key()? {
                let lineage = root.clone().unwrap_or_else(|| id.local_id().clone());
                let entries = storage.keyed_lineages.get_mut().unwrap().entry(key).or_default();
                // Preserve individual keyed associations so preparation must deduplicate lineages.
                entries.push(lineage);
            }
            storage.holons.insert(
                id.clone(),
                SavedHolon::new(id.local_id().clone(), model.properties, root, model.version),
            );
            if let Some(relationships) = model.relationships {
                for (name, members) in relationships.iter() {
                    let targets = members
                        .read()
                        .unwrap()
                        .get_members()
                        .iter()
                        .map(|target| Ok(reference_ids[target].clone()))
                        .collect::<Result<Vec<_>, HolonError>>()?;
                    storage.relationships.insert((id.clone(), name), targets);
                }
            }
        }
        for (key, error) in failures {
            storage.read_errors.insert(ids[*key].clone(), error.clone());
        }
        let storage = Arc::new(storage);
        let manager = Arc::new(HolonSpaceManager::new_with_managers(
            None,
            storage.clone(),
            None,
            ServiceRoutingPolicy::BlockExternal,
        ));
        let context = manager.get_transaction_manager().open_public_transaction(manager.clone())?;
        let nodes = ids
            .into_iter()
            .map(|(key, id)| (key, HolonReference::smart_from_id(context.space_read_handle(), id)))
            .collect();
        Ok(Self { context, nodes, saved_storage: Some(storage) })
    }

    /// Restores an update snapshot for a saved schema definition without requiring its
    /// complete meta-contract. Tests can deliberately author malformed successors.
    pub fn stage_update_of(&self, key: &str) -> Result<StagedReference, HolonError> {
        self.stage_update_snapshot(key, |_| Ok(()), false)
    }

    pub fn stage_update_of_with(
        &self,
        key: &str,
        edit: impl FnOnce(
            &mut holons_core::core_shared_objects::holon::HolonCloneModel,
        ) -> Result<(), HolonError>,
    ) -> Result<StagedReference, HolonError> {
        self.stage_update_snapshot(key, edit, true)
    }

    pub fn stage_update_snapshot(
        &self,
        key: &str,
        edit: impl FnOnce(
            &mut holons_core::core_shared_objects::holon::HolonCloneModel,
        ) -> Result<(), HolonError>,
        changed: bool,
    ) -> Result<StagedReference, HolonError> {
        use holons_core::core_shared_objects::holon::{HolonCloneModel, StagedHolon};
        let saved = &self.nodes[key];
        let HolonReference::Smart(reference) = saved else { panic!("saved fixture required") };
        let mut model = HolonCloneModel {
            version: base_types::MapInteger(2),
            original_id: None,
            properties: reference.into_model()?.property_map,
            relationships: Some(
                self.saved_storage
                    .as_ref()
                    .unwrap()
                    .fetch_all_related_holons_internal(&self.context, &saved.holon_id()?)?,
            ),
        };
        // Match staging's constructor-owned ancestry without marking it as an authored edit.
        let mut relationships = model.relationships.as_ref().unwrap().clone_for_staged()?;
        relationships.map.remove(&CoreRelationshipTypeName::Predecessor.as_relationship_name());
        relationships.add_related_holons_with_keys(
            CoreRelationshipTypeName::Predecessor.as_relationship_name(),
            vec![(saved.clone(), None)],
        )?;
        model.relationships = Some(holons_core::RelationshipMap::from(relationships));
        edit(&mut model)?;
        let transient = self.context.mutation().new_holon(Some(MapString(key.into())))?;
        let staged = self.context.mutation().stage_new_holon(transient)?;
        let mut holon = StagedHolon::new_for_update_from_clone_model(
            model,
            saved.holon_id()?.local_id().clone(),
        )?;
        if changed {
            holon.note_property_mutation()?;
        }
        *staged.get_holon_to_commit(&self.context)?.write().unwrap() = Holon::Staged(holon);
        Ok(staged)
    }
}

pub(super) fn c2_kind_semantics(
    fixture: &mut Fixture,
) -> Result<holons_core::DescriptorKindSemantics, HolonError> {
    use type_names::CorePropertyTypeName;
    fixture.node("MetaHolonType.MetaTypeDescriptor")?;
    fixture.link(
        "MetaTypeDescriptor.HolonType",
        CoreRelationshipTypeName::Extends,
        "HolonType.TypeDescriptor",
    )?;
    fixture.link(
        "MetaHolonType.MetaTypeDescriptor",
        CoreRelationshipTypeName::Extends,
        "MetaTypeDescriptor.HolonType",
    )?;
    for (key, anchor) in [
        ("TypeDescriptor", false),
        ("HolonType.TypeDescriptor", true),
        ("MetaTypeDescriptor.HolonType", false),
        ("MetaHolonType.MetaTypeDescriptor", false),
        ("PropertyType.TypeDescriptor", true),
        ("Contract", false),
    ] {
        fixture
            .nodes
            .get_mut(key)
            .unwrap()
            .with_property_value(CorePropertyTypeName::DefinesInstanceTypeKind, anchor)?;
    }
    fixture
        .nodes
        .get_mut("HolonType.TypeDescriptor")
        .unwrap()
        .with_property_value(CorePropertyTypeName::IsAbstractType, true)?;
    Ok(holons_core::DescriptorKindSemantics::new(&fixture.context))
}

/// Complete the roots and member structure consumed by prospective Commit assessment.
pub(super) fn readiness_fixture() -> Result<Fixture, HolonError> {
    let mut fixture = Fixture::new()?;
    c2_kind_semantics(&mut fixture)?;
    for key in [
        "Schema.HolonType",
        "Rule.HolonType",
        "ConstraintType.HolonType",
        "StringLengthConstraint.ConstraintType",
        "BytesLengthConstraint.ConstraintType",
        "NumericRangeConstraint.ConstraintType",
        "ItemCountConstraint.ConstraintType",
        "CardinalityConstraint.ConstraintType",
        "UniqueItemsConstraint.ConstraintType",
        "ValueType.TypeDescriptor",
        "DeclaredRelationshipType.RelationshipType",
    ] {
        fixture.node(key)?;
    }
    fixture.link(
        "DeclaredRelationshipType",
        CoreRelationshipTypeName::Extends,
        "DeclaredRelationshipType.RelationshipType",
    )?;
    fixture.link(
        "DeclaredRelationshipType.RelationshipType",
        CoreRelationshipTypeName::Extends,
        "TypeDescriptor",
    )?;
    fixture.link(
        "ValueType.TypeDescriptor",
        CoreRelationshipTypeName::Extends,
        "TypeDescriptor",
    )?;
    // Native value anchors inherit from ValueType rather than directly from TypeDescriptor.
    for (_, _, key) in subject_rules()
        .filter(|(name, _, _)| crate::registries::registration(*name).native_kind.is_some())
    {
        let type_descriptor = fixture.nodes["TypeDescriptor"].clone();
        fixture
            .nodes
            .get_mut(key)
            .unwrap()
            .remove_related_holons(CoreRelationshipTypeName::Extends, vec![type_descriptor])?;
        fixture.link(key, CoreRelationshipTypeName::Extends, "ValueType.TypeDescriptor")?;
        fixture.nodes.get_mut(key).unwrap().with_property_value("IsAbstractType", true)?;
    }
    for key in [
        "Title.PropertyType",
        "Key.PropertyType",
        "DeclaredRelationshipType",
        "DescribedBy.Relationship",
    ] {
        fixture
            .nodes
            .get_mut(key)
            .unwrap()
            .with_property_value("DefinesInstanceTypeKind", false)?;
    }
    for key in [
        "PropertyType.TypeDescriptor",
        "ValueType.TypeDescriptor",
        "DeclaredRelationshipType.RelationshipType",
        "StringValueType.ValueType",
    ] {
        fixture
            .nodes
            .get_mut(key)
            .unwrap()
            .with_property_value("DefinesInstanceTypeKind", true)?
            .with_property_value("IsAbstractType", true)?;
    }
    fixture.link(
        "DescribedBy.Relationship",
        CoreRelationshipTypeName::SourceType,
        "HolonType.TypeDescriptor",
    )?;
    fixture.link(
        "DescribedBy.Relationship",
        CoreRelationshipTypeName::TargetType,
        "HolonType.TypeDescriptor",
    )?;
    for rule in CoreValidationRuleName::all() {
        let registration = crate::registries::registration(rule);
        if matches!(
            registration.implementation,
            Some(
                crate::registries::RuleImplementation::DescriptorProduct
                    | crate::registries::RuleImplementation::SchemaAggregate(_)
            )
        ) {
            fixture.node(rule.as_str())?;
            fixture.link(
                rule.as_str(),
                CoreRelationshipTypeName::DescribedBy,
                registration.family.key(),
            )?;
        }
    }
    Ok(fixture)
}
