use std::sync::Arc;

use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::query_layer::QueryRequest;
use map_commands_contract::MapResult;

/// Bridges command ingress into the shared query substrate boundary.
///
/// Query PRO2 stabilizes the contract path down to this seam without
/// implementing the new descriptor-aware substrate yet. Commands remain the
/// ingress adapter; the real query engine should attach below this boundary in
/// later PRS work.
pub async fn handle_query_request(
    _context: &Arc<TransactionContext>,
    _request: QueryRequest,
) -> Result<MapResult, HolonError> {
    Err(HolonError::NotImplemented(
        "query substrate boundary is defined but not implemented".to_string(),
    ))
}
