use crate::core_shared_objects::{Holon, HolonError};
use core_types::TemporaryId;
use std::any::Any;
use std::{cell::RefCell, rc::Rc};

/// Provides access to staged holons by resolving a `StagedReference`
/// to retrieve the corresponding `Holon`.
///
/// This trait is **only responsible** for retrieving a holon **by index**.
/// It does **not** manage staging, committing, or other lifecycle behaviors.
/// NurseryAccess is a single-threaded trait for accessing nursery data.
/// It is not `Sync` or `Send` and must not be used in multi-threaded contexts.
pub trait NurseryAccess: Any {
    /// Resolves a `StagedReference` by retrieving the staged holon
    /// at the specified index.
    ///
    /// # Arguments
    /// - `id` - The index (represented by TemporaryId) of the staged holon within the nursery.
    ///
    /// # Returns
    /// - `Ok(Rc<RefCell<Holon>>)` if the index is valid.
    /// - `Err(HolonError::IndexOutOfRange)` if the index is invalid.
    fn get_holon_by_id(&self, id: &TemporaryId) -> Result<Rc<RefCell<Holon>>, HolonError>;
}
