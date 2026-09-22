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
use map_commands_contract::{HolonAction, HolonCommand, MapResult, ReadableHolonAction};
use std::{any::Any, collections::HashMap, sync::Arc};

/// Persisted fixture graph, reached exclusively through real bound references.
#[derive(Debug, Default)]
struct Graph {
    edges: HashMap<(u8, String), Vec<u8>>,
    properties: HashMap<u8, PropertyMap>,
}
impl Graph {
    fn edge(&mut self, source: u8, name: &str, targets: &[u8]) {
        self.edges.insert((source, name.into()), targets.into());
    }
    fn property(&mut self, id: u8, name: &str, value: BaseValue) {
        self.properties.entry(id).or_default().insert(PropertyName(name.into()), value);
    }
    fn reference(context: &Arc<TransactionContext>, id: u8) -> HolonReference {
        HolonReference::smart_with_key(
            context.space_read_handle(),
            HolonId::Local(LocalId(vec![id])),
            MapString(format!("node-{id}")),
        )
    }
    fn context(self) -> Arc<TransactionContext> {
        let space = Arc::new(HolonSpaceManager::new_with_managers(
            None,
            Arc::new(self),
            None,
            ServiceRoutingPolicy::BlockExternal,
        ));
        space.get_transaction_manager().open_public_transaction(space.clone()).unwrap()
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
            "StringValueType.ValueType" => 30,
            "IntegerValueType.ValueType" => 31,
            "BooleanValueType.ValueType" => 32,
            "BytesValueType.ValueType" => 33,
            "EnumValueType.ValueType" => 34,
            "BaseValueValueType.ValueType" => 35,
            "ValueArrayValueType.ValueType" => 36,
            _ => return Err(HolonError::InvalidParameter(format!("Unknown fixture key {key}"))),
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
        let mut properties = self.properties.get(&id.local_id().0[0]).cloned().unwrap_or_default();
        properties.insert(
            PropertyName("Key".into()),
            BaseValue::StringValue(MapString(format!("node-{}", id.local_id().0[0]))),
        );
        properties
            .entry(PropertyName("TypeName".into()))
            .or_insert(BaseValue::StringValue("Fixture".into()));
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
        panic!("read-only fixture")
    }
    fn delete_holon_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &LocalId,
    ) -> Result<(), HolonError> {
        panic!("read-only fixture")
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
async fn read(
    context: &Arc<TransactionContext>,
    id: u8,
    action: ReadableHolonAction,
) -> Result<MapResult, HolonError> {
    crate::holon_handler::handle_holon(HolonCommand {
        context: context.clone(),
        target: Graph::reference(context, id),
        action: HolonAction::Read(action),
    })
    .await
}

#[tokio::test]
async fn retrieves_described_declared_and_inverse_collections_through_bound_references() {
    for count in [0, 1, 3] {
        for inverse in [false, true] {
            let mut graph = Graph::default();
            graph.edge(1, "DescribedBy", &[2]);
            graph.edge(4, "DescribedBy", &[3]);
            graph.edge(2, if inverse { "SourceOf" } else { "InstanceRelationships" }, &[5]);
            graph.edge(5, "Extends", &[6]);
            graph.edge(5, "TargetType", &[3]);
            graph.edge(5, "SourceType", &[2]);
            graph.property(5, "TypeName", BaseValue::StringValue("Children".into()));
            graph.property(
                6,
                "TypeName",
                BaseValue::StringValue(
                    if inverse { "InverseRelationshipType" } else { "DeclaredRelationshipType" }
                        .into(),
                ),
            );
            graph.edge(1, "Children", &vec![4; count]);
            let context = graph.context();
            let result = read(
                &context,
                1,
                ReadableHolonAction::GetDescribedRelatedHolons {
                    name: RelationshipName("Children".into()),
                },
            )
            .await
            .unwrap();
            let MapResult::DescribedCollection(collection) = result else {
                panic!("Expected described collection")
            };
            assert_eq!(collection.members.get_members().len(), count);
            assert_eq!(collection.element_type.holon_id().unwrap().local_id().0[0], 3);
            assert!(read(
                &context,
                1,
                ReadableHolonAction::GetDescribedRelatedHolons {
                    name: RelationshipName("Missing".into())
                }
            )
            .await
            .is_err());
        }
    }
}

#[tokio::test]
async fn reads_actual_default_values_with_different_scalar_types() {
    let mut graph = Graph::default();
    // Both rows share a broad DefaultValue definition but declare different ValueTypes.
    graph.edge(1, "DescribedBy", &[3]);
    graph.edge(2, "DescribedBy", &[3]);
    graph.edge(3, "InstanceProperties", &[4]);
    graph.property(4, "TypeName", BaseValue::StringValue("DefaultValue".into()));
    graph.edge(4, "ValueType", &[35]);
    graph.edge(1, "ValueType", &[30]);
    graph.edge(2, "ValueType", &[31]);
    graph.property(1, "DefaultValue", BaseValue::StringValue("text".into()));
    graph.property(2, "DefaultValue", BaseValue::IntegerValue(7.into()));
    let context = graph.context();
    for id in [1, 2] {
        assert!(matches!(
            read(
                &context,
                id,
                ReadableHolonAction::GetPropertyValue { name: PropertyName("DefaultValue".into()) }
            )
            .await
            .unwrap(),
            MapResult::Value(_)
        ));
    }
    assert!(
        matches!(read(&context, 4, ReadableHolonAction::GetPropertyValueKind).await.unwrap(), MapResult::Value(BaseValue::StringValue(kind)) if kind.0 == "AnyBaseValue")
    );
}

#[tokio::test]
async fn reads_actual_values_without_validation_and_preserves_absence() {
    let mut graph = Graph::default();
    graph.edge(1, "DescribedBy", &[3]);
    graph.edge(2, "DescribedBy", &[3]);
    graph.edge(3, "InstanceProperties", &[4]);
    graph.property(4, "TypeName", BaseValue::StringValue("Name".into()));
    graph.edge(4, "ValueType", &[30]);
    graph.property(1, "Name", BaseValue::IntegerValue(7.into()));
    let context = graph.context();
    assert!(matches!(
        read(
            &context,
            1,
            ReadableHolonAction::GetPropertyValue { name: PropertyName("Name".into()) }
        )
        .await
        .unwrap(),
        MapResult::Value(BaseValue::IntegerValue(_))
    ));
    assert!(matches!(
        read(
            &context,
            2,
            ReadableHolonAction::GetPropertyValue { name: PropertyName("Name".into()) }
        )
        .await
        .unwrap(),
        MapResult::None
    ));
    assert!(
        matches!(read(&context, 3, ReadableHolonAction::GetInstanceProperties).await.unwrap(), MapResult::Collection(properties) if properties.get_members().len() == 1)
    );
}
