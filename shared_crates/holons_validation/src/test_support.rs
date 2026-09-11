use std::{any::Any, collections::BTreeMap, sync::Arc};

use base_types::MapString;
use core_types::{HolonError, HolonId, LocalId, RelationshipName};
use holons_core::core_shared_objects::{
    space_manager::HolonSpaceManager, transactions::TransactionContext, Holon,
};
use holons_core::{
    HolonCollection, HolonReference, HolonServiceApi, RelationshipMap, ServiceRoutingPolicy,
    StagedReference, TransientReference, WritableHolon,
};
use type_names::{CoreRelationshipTypeName, CoreValidationRuleName};

/// In-memory fixture service: any attempt to reach storage fails the test.
#[derive(Debug)]
struct NoStorage;
impl HolonServiceApi for NoStorage {
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
        _: &Arc<TransactionContext>,
        _: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        panic!("unexpected storage traversal")
    }
    fn fetch_holon_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &HolonId,
    ) -> Result<Holon, HolonError> {
        panic!("unexpected storage read")
    }
    fn fetch_related_holons_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &HolonId,
        _: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
        panic!("unexpected storage relationship read")
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
        _: &Arc<TransactionContext>,
        key: &MapString,
    ) -> Result<holons_core::SmartReference, HolonError> {
        Err(HolonError::HolonNotFound(key.to_string()))
    }
}

/// A small graph with real bound references and all canonical C1 anchors.
pub(super) struct Fixture {
    pub context: Arc<TransactionContext>,
    pub nodes: BTreeMap<String, HolonReference>,
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
            Arc::new(NoStorage),
            None,
            ServiceRoutingPolicy::BlockExternal,
        ));
        let context = manager.get_transaction_manager().open_new_transaction(manager.clone())?;
        Ok(Self { context, nodes: BTreeMap::new() })
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
