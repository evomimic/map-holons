use super::HolonReference;
use base_types::{MapInteger, MapString};
use core_types::HolonError;
use std::fmt::Debug;

/// Common mutation and access surface for holon reference collections.
///
/// ## Scope and Intent
///
/// This trait defines a **minimal, shared API** used by multiple collection
/// types (`HolonCollection`, `StagedRelationshipMap`, `TransientCollection`, etc.)
/// during Phase 1.x of the transaction-bound execution model.
///
/// It intentionally:
/// - Avoids exposing internal storage or locking strategy
/// - Supports both keyed and non-keyed mutation paths
/// - Allows implementations to enforce phase-specific invariants
///
/// ## Design Notes
///
/// This API currently conflates:
/// - relationship-backed collections
/// - transient, non-relationship collections
///
/// This is a **deliberate short-term compromise** to avoid premature trait
/// fragmentation during the transaction binding rollout.
///
/// ## Future Direction
///
/// As execution semantics stabilize (post Phase 1.2), this trait is expected to:
/// - Either be **split** into more semantically precise traits, or
/// - Be **narrowed** to represent only relationship-backed collections
///
/// New code should prefer concrete collection types where possible
/// rather than introducing additional generic bounds on this trait.
pub trait HolonCollectionApi: Debug + Send + Sync {
    fn add_references(&mut self, holons: Vec<HolonReference>) -> Result<(), HolonError>;

    fn add_reference_with_key(
        &mut self,
        key: Option<&MapString>,
        reference: &HolonReference,
    ) -> Result<(), HolonError>;

    /// Adds references using precomputed keys, avoiding any key lookups during mutation.
    fn add_references_with_keys(
        &mut self,
        entries: Vec<(HolonReference, Option<MapString>)>,
    ) -> Result<(), HolonError>;

    fn get_count(&self) -> MapInteger;

    fn get_by_index(&self, index: usize) -> Result<HolonReference, HolonError>;

    fn get_by_key(&self, key: &MapString) -> Result<Option<HolonReference>, HolonError>;

    fn remove_references(&mut self, holons: Vec<HolonReference>) -> Result<(), HolonError>;

    /// Removes references using precomputed keys, rebuilding the keyed index without holon lookups.
    fn remove_references_with_keys(
        &mut self,
        entries: Vec<(HolonReference, Option<MapString>)>,
    ) -> Result<(), HolonError>;
}
