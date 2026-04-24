use crate::core_shared_objects::space_manager::HolonSpaceManager;
use crate::core_shared_objects::transactions::TransactionContext;
use crate::core_shared_objects::{Holon, HolonCollection, RelationshipMap, ServiceRoutingPolicy};
use crate::reference_layer::{HolonServiceApi, StagedReference, TransientReference, WritableHolon};
use base_types::MapString;
use core_types::{HolonError, HolonId, LocalId, RelationshipName};
use std::any::Any;
use std::sync::Arc;

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

/// Creates a transient descriptor holon with the shared header properties.
pub(crate) fn new_descriptor_holon(
    context: &Arc<TransactionContext>,
    key: &str,
    type_name: &str,
    instance_type_kind: &str,
) -> Result<TransientReference, HolonError> {
    let mut descriptor = new_test_holon(context, key)?;
    descriptor
        .with_property_value(type_names::CorePropertyTypeName::TypeName, type_name)?
        .with_property_value(type_names::CorePropertyTypeName::IsAbstractType, false)?
        .with_property_value(
            type_names::CorePropertyTypeName::InstanceTypeKind,
            instance_type_kind,
        )?;
    Ok(descriptor)
}
