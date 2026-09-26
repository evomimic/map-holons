use holons_core::core_shared_objects::transient_holon_manager::ToHolonCloneModel;
use std::{
    any::Any,
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use base_types::MapString;
use core_types::{HolonError, HolonId, LocalId, RelationshipName};
use holons_core::core_shared_objects::{
    space_manager::HolonSpaceManager, transactions::TransactionContext, Holon,
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
}
impl HolonServiceApi for FixtureStorage {
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
        Ok(Holon::Saved(self.holons.get(id).expect("unexpected storage read").clone()))
    }
    fn fetch_related_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
        source: &HolonId,
        name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
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
    fn get_saved_holon_by_key_internal(
        &self,
        context: &Arc<TransactionContext>,
        key: &MapString,
    ) -> Result<holons_core::SmartReference, HolonError> {
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

pub(super) const RULES: [(CoreValidationRuleName, &str, &str); 7] = {
    use CoreValidationRuleName::*;
    [
        (
            RequiredPropertyPresence,
            "PropertyValidationRule.HolonType",
            "PropertyType.TypeDescriptor",
        ),
        (NoUndescribedProperties, "HolonValidationRule.HolonType", "HolonType.TypeDescriptor"),
        (BaseValueKindMatchesString, "StringValidationRule.HolonType", "StringValueType.ValueType"),
        (
            BaseValueKindMatchesInteger,
            "IntegerValidationRule.HolonType",
            "IntegerValueType.ValueType",
        ),
        (
            BaseValueKindMatchesBoolean,
            "BooleanValidationRule.HolonType",
            "BooleanValueType.ValueType",
        ),
        (BaseValueKindMatchesBytes, "BytesValidationRule.HolonType", "BytesValueType.ValueType"),
        (BaseValueKindMatchesEnum, "EnumValueValidationRule.HolonType", "EnumValueType.ValueType"),
    ]
};

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
        for (rule, family, target) in RULES {
            fixture.node(rule.as_str())?;
            fixture.node(family)?;
            fixture.node(target)?;
            fixture.link(rule.as_str(), CoreRelationshipTypeName::DescribedBy, family)?;
            fixture.link(target, CoreRelationshipTypeName::Extends, "TypeDescriptor")?;
            fixture.link(target, CoreRelationshipTypeName::ValidationBindings, rule.as_str())?;
        }
        // Standalone subject tests bind only subject rules. The binding inventory
        // still resolves all descriptor and Schema rule identities for placement checks.
        for rule in [
            CoreValidationRuleName::AtMostOneDirectParent,
            CoreValidationRuleName::AcyclicExtendsLineage,
            CoreValidationRuleName::ExtendsLineageTerminatesAtTypeDescriptor,
            CoreValidationRuleName::UniqueTypeDescriptorRoot,
            CoreValidationRuleName::LocalInstanceKindAnchorDesignation,
            CoreValidationRuleName::InstanceKindAnchorsAreAbstract,
            CoreValidationRuleName::TypeDescriptorRootKindException,
            CoreValidationRuleName::DescribingCategoryCompatibility,
            CoreValidationRuleName::DescriptorMetaTypeCorrespondence,
            CoreValidationRuleName::NoInheritedMemberRedeclaration,
            CoreValidationRuleName::UniqueSemanticMemberNames,
            CoreValidationRuleName::WellFormedEffectiveMemberDefinitions,
            CoreValidationRuleName::ContractMemberKindCompatibility,
            CoreValidationRuleName::InheritedValueConstraintNonRelaxation,
            CoreValidationRuleName::SchemaDependenciesAcyclic,
            CoreValidationRuleName::CrossSchemaDependenciesDeclared,
        ] {
            fixture.node(rule.as_str())?;
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

    pub fn subject(&self) -> Result<HolonReference, HolonError> {
        let mut subject = self.context.mutation().new_holon(Some(MapString("subject".into())))?;
        subject.with_descriptor(self.nodes["Contract"].clone())?;
        Ok(subject.into())
    }

    /// Stages a subject before attaching its contract so tests can author missing inputs
    /// explicitly without staging-time default population supplying them.
    pub fn staged_subject(&self, key: &str) -> Result<StagedReference, HolonError> {
        let subject = self.context.mutation().new_holon(Some(MapString(key.into())))?;
        let mut subject = self.context.mutation().stage_new_holon(subject)?;
        subject.with_descriptor(self.nodes["Contract"].clone())?;
        Ok(subject)
    }
}

impl Fixture {
    /// Saves the fixture graph as service snapshots and opens a fresh empty Nursery.
    /// All relationship endpoints are rebound as saved references on read.
    pub fn saved_snapshot(&self) -> Result<Self, HolonError> {
        use holons_core::core_shared_objects::holon::SavedHolon;
        let ids: BTreeMap<_, _> = self
            .nodes
            .keys()
            .enumerate()
            .map(|(index, key)| (key.clone(), HolonId::Local(LocalId(vec![index as u8 + 1; 39]))))
            .collect();
        let mut storage = FixtureStorage::default();
        for (key, reference) in &self.nodes {
            let model = reference.holon_clone_model()?;
            let id = ids[key].clone();
            storage.holons.insert(
                id.clone(),
                SavedHolon::new(id.local_id().clone(), model.properties, None, model.version),
            );
            if let Some(relationships) = model.relationships {
                for (name, members) in relationships.iter() {
                    let targets = members
                        .read()
                        .unwrap()
                        .get_members()
                        .iter()
                        .map(|target| Ok(ids[&target.key()?.unwrap().to_string()].clone()))
                        .collect::<Result<Vec<_>, HolonError>>()?;
                    storage.relationships.insert((id.clone(), name), targets);
                }
            }
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
    /// complete meta-contract. Tests can deliberately author malformed replacements.
    pub fn replacement(&self, key: &str) -> Result<StagedReference, HolonError> {
        self.replacement_with(key, |_| Ok(()))
    }

    pub fn replacement_with(
        &self,
        key: &str,
        edit: impl FnOnce(
            &mut holons_core::core_shared_objects::holon::HolonCloneModel,
        ) -> Result<(), HolonError>,
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
        edit(&mut model)?;
        let transient = self.context.mutation().new_holon(Some(MapString(key.into())))?;
        let staged = self.context.mutation().stage_new_holon(transient)?;
        *staged.get_holon_to_commit(&self.context)?.write().unwrap() =
            Holon::Staged(StagedHolon::new_for_update_from_clone_model(
                model,
                saved.holon_id()?.local_id().clone(),
            )?);
        Ok(staged)
    }
}
