use core_types::{HolonError, HolonId};

use super::Holon;

/// An error affecting one identity, a failed batch, or an unexpected response ID.
/// Successful cache population has no corresponding result entry.
#[derive(Debug, Clone, PartialEq)]
pub struct CachePopulationError {
    pub ids: Vec<HolonId>,
    pub error: HolonError,
}

impl CachePopulationError {
    pub fn new(ids: Vec<HolonId>, error: HolonError) -> Self {
        Self { ids, error }
    }
}

/// Identity-associated infrastructure hydration, internal to the service boundary.
/// This full-state carrier is not an application result or a residency lease.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedHolonFetch {
    pub id: HolonId,
    pub result: Result<Holon, HolonError>,
}

/// Validates exact node identity and saved shape before admitting fetched state.
pub fn validate_saved_content(id: &HolonId, holon: &Holon) -> Result<(), HolonError> {
    match holon {
        Holon::Saved(saved) if saved.get_local_id()? == *id.local_id() => Ok(()),
        _ => Err(HolonError::InvalidParameter(format!(
            "Saved hydration returned non-saved or mismatched content for {id:?}"
        ))),
    }
}
