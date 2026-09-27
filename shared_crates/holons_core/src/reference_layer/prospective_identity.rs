use std::sync::Arc;

use core_types::{HolonError, HolonId, TemporaryId};

use crate::core_shared_objects::transactions::TransactionContext;
use crate::reference_layer::assert_reference_transaction_compatible;
use crate::HolonReference;

/// Identity of one definition within a prospective Commit assessment.
///
/// A staged update and its explicitly recorded saved source have the same identity.
/// Two staged updates from one saved source share this identity; the prospective
/// view must resolve that ambiguity before selecting authoritative content.
/// Creates never acquire saved identity by sharing a key. Full typed IDs preserve
/// distinctions hidden by diagnostic display strings; external IDs retain their space.
/// Use only within one assessment's local space and transaction, not as a durable key.
/// This canonicalizes identity only; selecting replacement content belongs to the
/// prospective assessment view and does not change ordinary reference resolution.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProspectiveIdentity {
    /// Persisted version, including the source of a staged update.
    Saved(HolonId),
    /// Staged create without persisted replacement ancestry.
    Staged(TemporaryId),
    /// Transient input, distinct from any staged definition.
    Transient(TemporaryId),
}

impl ProspectiveIdentity {
    /// Resolves identity without fetching saved content or inspecting semantic keys.
    /// Mutable references must belong to the assessment's active transaction.
    pub fn for_reference(
        reference: &HolonReference,
        context: &Arc<TransactionContext>,
    ) -> Result<Self, HolonError> {
        assert_reference_transaction_compatible(reference, context)?;
        match reference {
            HolonReference::Smart(saved) => Ok(Self::Saved(saved.holon_id())),
            HolonReference::Staged(staged) => Ok(match staged.versioned_source_id()? {
                Some(source) => Self::Saved(HolonId::Local(source)),
                None => Self::Staged(staged.temporary_id()),
            }),
            HolonReference::Transient(transient) => Ok(Self::Transient(transient.temporary_id())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_test_holon};
    use core_types::{ExternalId, LocalId, OutboundProxyId};
    use std::collections::HashSet;

    #[test]
    fn saved_identity_uses_full_ids_and_preserves_external_space() -> Result<(), HolonError> {
        let context = build_context();
        let first = LocalId(vec![1; 39]);
        let mut second = first.clone();
        second.0[0] = 2;
        assert_eq!(first.to_string(), second.to_string());
        let ids = [
            HolonId::Local(first.clone()),
            HolonId::Local(second.clone()),
            HolonId::External(ExternalId {
                space_id: OutboundProxyId(first.clone()),
                local_id: first.clone(),
            }),
            HolonId::External(ExternalId { space_id: OutboundProxyId(second), local_id: first }),
        ];
        let identities = ids
            .into_iter()
            .map(|id| {
                let reference = HolonReference::smart_from_id(context.space_read_handle(), id);
                ProspectiveIdentity::for_reference(&reference, &context)
            })
            .collect::<Result<HashSet<_>, HolonError>>()?;
        // No saved content exists in the fixture: identity needs no storage reads.
        assert_eq!(identities.len(), 4);
        Ok(())
    }

    #[test]
    fn mutable_identity_requires_active_transaction() -> Result<(), HolonError> {
        let first = build_context();
        let second = build_context();
        let transient = new_test_holon(&first, "transient")?;
        let staged = first.mutation().stage_new_holon(new_test_holon(&first, "staged")?)?;
        for reference in [HolonReference::from(transient), HolonReference::from(staged)] {
            ProspectiveIdentity::for_reference(&reference, &first)?;
            assert!(matches!(
                ProspectiveIdentity::for_reference(&reference, &second),
                Err(HolonError::CrossTransactionReference { .. })
            ));
        }
        Ok(())
    }
}
