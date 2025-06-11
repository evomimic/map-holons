

use crate::HolonError;
use super::holon::Holon;
use core_types::TemporaryId;
use std::any::Any;
use std::{cell::RefCell, rc::Rc};

/// Provides access to transients holons by resolving a `TransientReference`
/// to retrieve the corresponding `Holon`.
///
/// This trait is **only responsible** for retrieving a holon **by index**.
/// It does **not** manage staging, committing, or other lifecycle behaviors.
/// TransientManagerAccess is a single-threaded trait for accessing transient manager data.
/// It is not `Sync` or `Send` and must not be used in multi-threaded contexts.
pub trait TransientManagerAccess: Any {
    /// Resolves a `TransientReference` by retrieving the transient holon
    /// at the specified index.
    ///
    /// # Arguments
    /// - `id` - The index (represented by TemporaryId) of the transient holon within the manager.
    ///
    /// # Returns
    /// - `Ok(Rc<RefCell<Holon>>)` if the index is valid.
    /// - `Err(HolonError::IndexOutOfRange)` if the index is invalid.
    fn get_holon_by_id(&self, id: &TemporaryId) -> Result<Rc<RefCell<Holon>>, HolonError>;
}
