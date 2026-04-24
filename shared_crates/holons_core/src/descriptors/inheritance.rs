use std::collections::HashSet;
use std::sync::RwLockReadGuard;

use crate::core_shared_objects::{holon::state::AccessType, HolonCollection};
use crate::reference_layer::{HolonReference, ReadableHolon};
use core_types::HolonError;
use type_names::relationship_names::CoreRelationshipTypeName;

/// Resolves the direct `Extends` parent for a descriptor holon.
///
/// Cardinality is enforced here so all iterator-based callers inherit the same
/// multiple-parent error semantics.
pub(crate) fn extends_parent(holon: &HolonReference) -> Result<Option<HolonReference>, HolonError> {
    holon.is_accessible(AccessType::Read)?;
    let collection_arc = holon.related_holons(CoreRelationshipTypeName::Extends)?;
    let collection = collection_arc.read().map_err(lock_error)?;
    collection.is_accessible(AccessType::Read)?;
    let members = collection.get_members();

    match members.as_slice() {
        [] => Ok(None),
        [single] => Ok(Some(single.clone())),
        many => Err(HolonError::MultipleExtends {
            descriptor: descriptor_label(holon),
            count: many.len(),
        }),
    }
}

/// Returns a lazy iterator over the effective `Extends` chain.
///
/// Iteration always yields `self` first, then successive parents. Structural
/// errors are reported at the point the iterator discovers them so callers can
/// stop early once they have enough context.
pub fn walk_extends_chain(start: &HolonReference) -> ExtendsIter {
    ExtendsIter::new(start)
}

/// Materializes the full `Extends` chain including `self`.
///
/// This is the eager helper for callers that truly need the whole lineage
/// rather than a short-circuiting iterator walk.
pub fn ancestors(start: &HolonReference) -> Result<Vec<HolonReference>, HolonError> {
    walk_extends_chain(start).collect()
}

/// Lazy iterator over a descriptor's `Extends` lineage.
///
/// State model:
/// - `next` is the next descriptor to yield
/// - `pending_error` stores a structural error discovered while preparing the
///   next step
/// - `visited` tracks already-yielded descriptors for cycle detection
/// - `finished` marks terminal exhaustion after either the root or an error
pub struct ExtendsIter {
    next: Option<HolonReference>,
    pending_error: Option<HolonError>,
    visited: HashSet<String>,
    finished: bool,
}

impl ExtendsIter {
    fn new(start: &HolonReference) -> Self {
        Self {
            next: Some(start.clone()),
            pending_error: None,
            visited: HashSet::new(),
            finished: false,
        }
    }
}

impl Iterator for ExtendsIter {
    type Item = Result<HolonReference, HolonError>;

    fn next(&mut self) -> Option<Self::Item> {
        // Terminal state: once the chain is exhausted or an error has been
        // emitted, iteration stays closed.
        if self.finished {
            return None;
        }

        // Deferred error emission preserves the current item for callers that
        // intentionally stop early (for example, first-match inheritance lookups).
        if let Some(error) = self.pending_error.take() {
            self.finished = true;
            return Some(Err(error));
        }

        let current = self.next.take()?;
        self.visited.insert(current.reference_id_string());

        // Resolve the next step after capturing the current item. This keeps
        // the iterator self-first while still surfacing cycles and
        // multiple-parent structures on the following call.
        match extends_parent(&current) {
            Ok(Some(parent)) => {
                if self.visited.contains(&parent.reference_id_string()) {
                    self.pending_error =
                        Some(HolonError::CyclicExtends { descriptor: descriptor_label(&parent) });
                } else {
                    self.next = Some(parent);
                }
            }
            Ok(None) => {
                self.finished = true;
            }
            Err(error) => {
                self.pending_error = Some(error);
            }
        }

        Some(Ok(current))
    }
}

/// Best-effort descriptor label for structural inheritance errors.
///
/// Prefer the human-readable summary when available, but fall back to the
/// stable reference id so error construction never cascades into a second
/// failure path.
fn descriptor_label(holon: &HolonReference) -> String {
    match holon.summarize() {
        Ok(summary) => summary,
        Err(_) => holon.reference_id_string(),
    }
}

/// Normalizes poisoned collection-lock errors into the crate's standard
/// `FailedToAcquireLock` surface.
fn lock_error(error: std::sync::PoisonError<RwLockReadGuard<'_, HolonCollection>>) -> HolonError {
    HolonError::FailedToAcquireLock(format!(
        "Failed to acquire read lock on holon collection: {}",
        error
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_test_holon};
    use crate::reference_layer::WritableHolon;
    use type_names::relationship_names::CoreRelationshipTypeName;

    #[test]
    fn ancestors_returns_self_for_root_descriptor() -> Result<(), HolonError> {
        let context = build_context();
        let root = new_test_holon(&context, "root")?;
        let root_ref = HolonReference::from(&root);

        assert_eq!(ancestors(&root_ref)?, vec![root_ref]);

        Ok(())
    }

    #[test]
    fn ancestors_returns_linear_chain_in_self_first_order() -> Result<(), HolonError> {
        let context = build_context();
        let root = new_test_holon(&context, "a")?;
        let mut middle = new_test_holon(&context, "b")?;
        let mut leaf = new_test_holon(&context, "c")?;

        middle.add_related_holons(CoreRelationshipTypeName::Extends, vec![root.clone().into()])?;
        leaf.add_related_holons(CoreRelationshipTypeName::Extends, vec![middle.clone().into()])?;

        assert_eq!(
            ancestors(&HolonReference::from(&leaf))?,
            vec![
                HolonReference::from(&leaf),
                HolonReference::from(&middle),
                HolonReference::from(&root),
            ]
        );

        Ok(())
    }

    #[test]
    fn ancestors_errors_when_descriptor_has_multiple_extends() -> Result<(), HolonError> {
        let context = build_context();
        let parent_a = new_test_holon(&context, "parent-a")?;
        let parent_b = new_test_holon(&context, "parent-b")?;
        let mut child = new_test_holon(&context, "child")?;

        child.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![parent_a.into(), parent_b.into()],
        )?;

        assert!(matches!(
            ancestors(&HolonReference::from(&child)),
            Err(HolonError::MultipleExtends { count, .. }) if count == 2
        ));

        Ok(())
    }

    #[test]
    fn ancestors_errors_on_extends_cycles_and_self_loops() -> Result<(), HolonError> {
        let context = build_context();
        let mut a = new_test_holon(&context, "cycle-a")?;
        let mut b = new_test_holon(&context, "cycle-b")?;

        a.add_related_holons(CoreRelationshipTypeName::Extends, vec![b.clone().into()])?;
        b.add_related_holons(CoreRelationshipTypeName::Extends, vec![a.clone().into()])?;

        assert!(matches!(
            ancestors(&HolonReference::from(&a)),
            Err(HolonError::CyclicExtends { .. })
        ));

        let mut self_loop = new_test_holon(&context, "self-loop")?;
        self_loop.add_related_holons(
            CoreRelationshipTypeName::Extends,
            vec![self_loop.clone().into()],
        )?;

        assert!(matches!(
            ancestors(&HolonReference::from(&self_loop)),
            Err(HolonError::CyclicExtends { .. })
        ));

        Ok(())
    }

    #[test]
    fn walk_extends_chain_short_circuits_before_errors_further_up_chain() -> Result<(), HolonError>
    {
        let context = build_context();
        let mut ancestor = new_test_holon(&context, "ancestor")?;
        let mut middle = new_test_holon(&context, "middle")?;
        let mut leaf = new_test_holon(&context, "leaf")?;

        ancestor
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![ancestor.clone().into()])?;
        middle
            .add_related_holons(CoreRelationshipTypeName::Extends, vec![ancestor.clone().into()])?;
        leaf.add_related_holons(CoreRelationshipTypeName::Extends, vec![middle.clone().into()])?;

        let first_two = walk_extends_chain(&HolonReference::from(&leaf))
            .take(2)
            .collect::<Result<Vec<_>, _>>()?;

        assert_eq!(first_two, vec![HolonReference::from(&leaf), HolonReference::from(&middle)]);

        Ok(())
    }
}
