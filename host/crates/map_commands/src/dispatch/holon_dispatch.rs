use core_types::HolonError;

use crate::domain::{HolonCommand, MapResult};

/// Dispatches holon-scoped commands.
///
/// Stub: all holon commands return NotImplemented for Phase 2.1.
pub async fn dispatch_holon(_command: HolonCommand) -> Result<MapResult, HolonError> {
    Err(HolonError::NotImplemented(
        "HolonCommand dispatch".to_string(),
    ))
}
