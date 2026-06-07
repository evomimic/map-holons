use crate::core_shared_objects::space_manager::HolonSpaceManager;
use crate::core_shared_objects::transactions::TransactionContext;
use crate::core_shared_objects::{Holon, HolonCollection, RelationshipMap, ServiceRoutingPolicy};
use crate::reference_layer::{
    HolonReference, HolonServiceApi, StagedReference, TransientReference, WritableHolon,
};
use base_types::MapString;
use core_types::{BaseTypeKind, HolonError, HolonId, LocalId, RelationshipName, TypeKind};
use std::any::Any;
use std::sync::Arc;
use type_names::{
    CoreHolonTypeName, CorePropertyTypeName, CoreRelationshipTypeName, CoreValueTypeName,
};

// Minimal fail-fast holon service for descriptor unit tests.
//
// Descriptor runtime tests stay entirely in-memory in this phase, so any call
// that would cross into the real holon service is a test bug.
#[derive(Debug)]
struct TestHolonService;

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
        _id: &HolonId,
    ) -> Result<Holon, HolonError> {
        unreachable_in_descriptor_tests()
    }

    fn fetch_related_holons_internal(
        &self,
        _context: &Arc<TransactionContext>,
        _source_id: &HolonId,
        _relationship_name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
        unreachable_in_descriptor_tests()
    }

    fn get_all_holons_internal(
        &self,
        _context: &Arc<TransactionContext>,
    ) -> Result<HolonCollection, HolonError> {
        unreachable_in_descriptor_tests()
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
    let holon_service: Arc<dyn HolonServiceApi> = Arc::new(TestHolonService);
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

/// Returns the canonical schema name for a core value type.
pub(crate) fn core_value_type_name(core_value_type_name: CoreValueTypeName) -> String {
    core_value_type_name.as_value_name().to_string()
}

/// Creates a transient descriptor holon with the shared header properties.
pub(crate) trait TestTypeKindInput {
    fn as_type_kind_schema_key(&self) -> String;
}

impl TestTypeKindInput for TypeKind {
    fn as_type_kind_schema_key(&self) -> String {
        self.as_schema_key()
    }
}

impl TestTypeKindInput for &str {
    fn as_type_kind_schema_key(&self) -> String {
        match *self {
            "Holon" => TypeKind::Holon.as_schema_key(),
            "Property" => TypeKind::Property.as_schema_key(),
            "Relationship" => TypeKind::Relationship.as_schema_key(),
            "EnumVariant" => TypeKind::EnumVariant.as_schema_key(),
            // Existing descriptor tests use this shorthand for generic value descriptors.
            "Value" => TypeKind::Value(BaseTypeKind::String).as_schema_key(),
            other => other.to_string(),
        }
    }
}

pub(crate) fn new_descriptor_holon(
    context: &Arc<TransactionContext>,
    key: &str,
    type_name: &str,
    type_kind: impl TestTypeKindInput,
) -> Result<TransientReference, HolonError> {
    let mut descriptor = new_test_holon(context, key)?;
    descriptor
        .with_property_value(CorePropertyTypeName::TypeName, type_name)?
        .with_property_value(CorePropertyTypeName::IsAbstractType, false)?
        .with_property_value(
            CorePropertyTypeName::InstanceTypeKind,
            type_kind.as_type_kind_schema_key(),
        )?;
    Ok(descriptor)
}

/// Creates a holon-type descriptor with the required Phase B structural flags.
pub(crate) fn new_holon_type_descriptor(
    context: &Arc<TransactionContext>,
    key: &str,
    type_name: &str,
) -> Result<TransientReference, HolonError> {
    let mut descriptor = new_descriptor_holon(context, key, type_name, TypeKind::Holon)?;
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
    property_name: &str,
    is_required: bool,
    value_type: HolonReference,
) -> Result<TransientReference, HolonError> {
    let mut descriptor = new_descriptor_holon(context, key, type_name, TypeKind::Property)?;
    descriptor
        .with_property_value(CorePropertyTypeName::PropertyName, property_name)?
        .with_property_value(CorePropertyTypeName::IsRequired, is_required)?;
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
    let mut descriptor = new_descriptor_holon(context, key, type_name, TypeKind::Relationship)?;
    descriptor
        .with_property_value(CorePropertyTypeName::IsDefinitional, false)?
        .with_property_value(CorePropertyTypeName::IsOrdered, false)?
        .with_property_value(CorePropertyTypeName::AllowsDuplicates, false)?
        .with_property_value(CorePropertyTypeName::MinCardinality, 0_i64)?
        .with_property_value(CorePropertyTypeName::MaxCardinality, 1_i64)?;
    descriptor.add_related_holons(CoreRelationshipTypeName::SourceType, vec![source_type])?;
    descriptor.add_related_holons(CoreRelationshipTypeName::TargetType, vec![target_type])?;
    Ok(descriptor)
}
