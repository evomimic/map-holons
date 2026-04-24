use crate::core_shared_objects::space_manager::HolonSpaceManager;
use crate::core_shared_objects::transactions::TransactionContext;
use crate::core_shared_objects::{Holon, HolonCollection, RelationshipMap, ServiceRoutingPolicy};
use crate::reference_layer::{HolonServiceApi, StagedReference, TransientReference};
use base_types::MapString;
use core_types::{HolonError, HolonId, LocalId, RelationshipName};
use std::any::Any;
use std::sync::Arc;

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

pub(crate) fn new_test_holon(
    context: &Arc<TransactionContext>,
    key: &str,
) -> Result<TransientReference, HolonError> {
    context.mutation().new_holon(Some(MapString(key.to_string())))
}
