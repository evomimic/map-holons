use crate::core_shared_objects::holon::SavedHolon;
use crate::core_shared_objects::space_manager::HolonSpaceManager;
use crate::core_shared_objects::transactions::TransactionContext;
use crate::core_shared_objects::{Holon, HolonCollection, RelationshipMap, ServiceRoutingPolicy};
use crate::reference_layer::{
    HolonReference, HolonServiceApi, StagedReference, TransientReference, WritableHolon,
};
use crate::HolonCollectionApi;
use base_types::MapString;
use core_types::{HolonError, HolonId, LocalId, RelationshipName};
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use type_names::{CoreHolonTypeName, CorePropertyTypeName, CoreRelationshipTypeName};

// Minimal fail-fast holon service for descriptor unit tests.
//
// Descriptor runtime tests stay entirely in-memory. Explicit saved snapshots
// support reference-layer reads; other service operations fail rather than
// accidentally crossing into production storage.
#[derive(Debug, Default)]
struct TestHolonService {
    saved_holons: HashMap<HolonId, SavedHolon>,
    saved_relationships: HashMap<(HolonId, RelationshipName), Vec<HolonId>>,
}

fn unreachable_in_descriptor_tests<T>() -> Result<T, HolonError> {
    Err(HolonError::NotImplemented("TestHolonService".to_string()))
}

impl HolonServiceApi for TestHolonService {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn commit_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _staged_references: &[StagedReference],
    ) -> Result<TransientReference, HolonError> {
        unreachable_in_descriptor_tests()
    }

    fn delete_holon_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _local_id: &LocalId,
    ) -> Result<(), HolonError> {
        unreachable_in_descriptor_tests()
    }

    fn fetch_all_related_holons_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _source_id: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        unreachable_in_descriptor_tests()
    }

    fn fetch_holon_internal(
        &self,
        _context: &Arc<TransactionContext>,
        id: &HolonId,
    ) -> Result<Holon, HolonError> {
        self.saved_holons
            .get(id)
            .cloned()
            .map(Holon::Saved)
            .ok_or_else(|| HolonError::HolonNotFound(format!("test saved holon: {id:?}")))
    }

    fn fetch_related_holons_internal(
        &self,
        context: &Arc<TransactionContext>,
        source_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
        if !self.saved_holons.contains_key(source_id) {
            return unreachable_in_descriptor_tests();
        }
        let mut collection = HolonCollection::new_transient();
        if let Some(targets) =
            self.saved_relationships.get(&(source_id.clone(), relationship_name.clone()))
        {
            collection.add_references(
                targets
                    .iter()
                    .map(|id| HolonReference::smart_from_id(context.context_handle(), id.clone()))
                    .collect(),
            )?;
        }
        Ok(collection)
    }

    fn get_all_holons_internal(
        &self,
        _context: &Arc<TransactionContext>,
    ) -> Result<HolonCollection, HolonError> {
        unreachable_in_descriptor_tests()
    }

    fn get_saved_holon_by_key_internal(
        &self,
        context: &Arc<TransactionContext>,
        key: &MapString,
    ) -> Result<crate::SmartReference, HolonError> {
        use type_names::ToPropertyName;
        let property_name = CorePropertyTypeName::Key.to_property_name();
        let matches: Vec<_> = self
            .saved_holons
            .iter()
            .filter(|(_, holon)| {
                holon.property_map().get(&property_name)
                    == Some(&base_types::BaseValue::StringValue(key.clone()))
            })
            .collect();
        match matches.as_slice() {
            [(id, _)] => {
                Ok(crate::SmartReference::new_from_id(context.context_handle(), (*id).clone()))
            }
            [] => Err(HolonError::HolonNotFound(key.to_string())),
            _ => Err(HolonError::DuplicateError("saved fixture key".into(), key.to_string())),
        }
    }

    fn load_holons_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _bundle: TransientReference,
    ) -> Result<TransientReference, HolonError> {
        unreachable_in_descriptor_tests()
    }
}

/// Builds a fresh in-memory transaction context for descriptor tests.
///
/// This mirrors the transaction-context test harness so descriptor tests can
/// stage transient and staged holons without involving host or guest services.
pub(crate) fn build_context() -> Arc<TransactionContext> {
    build_context_with_saved_holons(Vec::new(), HashMap::new())
}

/// Supplies persisted snapshots through the service boundary for smart-reference tests.
/// Production storage and reference-layer cache behavior remain outside this fixture.
pub(crate) fn build_context_with_saved_holons(
    holons: Vec<SavedHolon>,
    relationships: HashMap<(HolonId, RelationshipName), Vec<HolonId>>,
) -> Arc<TransactionContext> {
    let saved_holons = holons
        .into_iter()
        .map(|holon| (HolonId::Local(holon.get_local_id().unwrap()), holon))
        .collect();
    let holon_service: Arc<dyn HolonServiceApi> =
        Arc::new(TestHolonService { saved_holons, saved_relationships: relationships });
    let space_manager = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        holon_service,
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));

    space_manager
        .get_transaction_manager()
        .open_new_transaction(Arc::clone(&space_manager))
        .expect("default transaction should open")
}

/// Creates a transient holon with a deterministic test key.
///
/// Descriptor tests use keyed transients because the underlying runtime rejects
/// keyless transient creation in normal mutation flows.
pub(crate) fn new_test_holon(
    context: &Arc<TransactionContext>,
    key: &str,
) -> Result<TransientReference, HolonError> {
    context.mutation().new_holon(Some(MapString(key.to_string())))
}

/// Returns the canonical schema name for a core holon type.
pub(crate) fn core_holon_type_name(core_holon_type_name: CoreHolonTypeName) -> String {
    core_holon_type_name.as_holon_name().to_string()
}

/// Creates a transient descriptor holon with the shared local header properties.
pub(crate) fn new_descriptor_holon(
    context: &Arc<TransactionContext>,
    key: &str,
    type_name: &str,
    _descriptor_family: &str,
) -> Result<TransientReference, HolonError> {
    let mut descriptor = new_test_holon(context, key)?;
    descriptor
        .with_property_value(CorePropertyTypeName::TypeName, type_name)?
        .with_property_value(CorePropertyTypeName::IsAbstractType, false)?;
    Ok(descriptor)
}

/// Creates a holon-type descriptor with the required Phase B structural flags.
pub(crate) fn new_holon_type_descriptor(
    context: &Arc<TransactionContext>,
    key: &str,
    type_name: &str,
) -> Result<TransientReference, HolonError> {
    let mut descriptor = new_descriptor_holon(context, key, type_name, "Holon")?;
    descriptor
        .with_property_value(CorePropertyTypeName::AllowsAdditionalProperties, false)?
        .with_property_value(CorePropertyTypeName::AllowsAdditionalRelationships, false)?;
    Ok(descriptor)
}

/// Creates a property descriptor with structural fields and its value type edge.
pub(crate) fn new_property_descriptor_holon(
    context: &Arc<TransactionContext>,
    key: &str,
    type_name: &str,
    is_required: bool,
    value_type: HolonReference,
) -> Result<TransientReference, HolonError> {
    let mut descriptor = new_descriptor_holon(context, key, type_name, "Property")?;
    descriptor.with_property_value(CorePropertyTypeName::IsValueRequired, is_required)?;
    descriptor.add_related_holons(CoreRelationshipTypeName::ValueType, vec![value_type])?;
    Ok(descriptor)
}

/// Creates a relationship descriptor with structural fields and source/target edges.
pub(crate) fn new_relationship_descriptor_holon(
    context: &Arc<TransactionContext>,
    key: &str,
    type_name: &str,
    source_type: HolonReference,
    target_type: HolonReference,
) -> Result<TransientReference, HolonError> {
    let mut descriptor = new_descriptor_holon(context, key, type_name, "Relationship")?;
    descriptor
        .with_property_value(CorePropertyTypeName::IsDefinitional, false)?
        .with_property_value(CorePropertyTypeName::IsOrdered, false)?
        .with_property_value(CorePropertyTypeName::AllowsDuplicates, false)?;
    descriptor.add_related_holons(CoreRelationshipTypeName::SourceType, vec![source_type])?;
    descriptor.add_related_holons(CoreRelationshipTypeName::TargetType, vec![target_type])?;
    Ok(descriptor)
}

/// Creates the minimal declared-relationship classifier used by Schema 2
/// fixtures. Individual declaration descriptors extend this holon so the
/// runtime can classify their direction without name-based shortcuts.
pub(crate) fn new_declared_relationship_type_descriptor(
    context: &Arc<TransactionContext>,
    key: &str,
) -> Result<TransientReference, HolonError> {
    new_descriptor_holon(
        context,
        key,
        &core_holon_type_name(CoreHolonTypeName::DeclaredRelationshipType),
        "Relationship",
    )
}

/// Creates a declared relationship descriptor with its Schema 2 classifier.
pub(crate) fn new_declared_relationship_descriptor_holon(
    context: &Arc<TransactionContext>,
    key: &str,
    type_name: &str,
    source_type: HolonReference,
    target_type: HolonReference,
) -> Result<TransientReference, HolonError> {
    let mut descriptor =
        new_relationship_descriptor_holon(context, key, type_name, source_type, target_type)?;
    let declared_type =
        context.mutation().stage_new_holon(new_declared_relationship_type_descriptor(
            context,
            &format!("{key}-declared-relationship-type"),
        )?)?;
    descriptor.add_related_holons(CoreRelationshipTypeName::Extends, vec![declared_type.into()])?;
    Ok(descriptor)
}
