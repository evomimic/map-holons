use std::sync::Arc;

use core_types::HolonError;

use crate::core_shared_objects::transactions::TransactionContext;
use crate::reference_layer::HolonReference;

/// Enforces the phase-aware compatibility rule for reference traversal.
///
/// Saved references are routable, immutable identities and do not carry a
/// transaction requirement. Staged and transient references resolve through
/// transaction-local pools, so they must belong to the active context.
pub fn assert_reference_transaction_compatible(
    reference: &HolonReference,
    context: &Arc<TransactionContext>,
) -> Result<(), HolonError> {
    let Some(reference_context) = reference.transaction_context() else {
        return Ok(());
    };
    if !Arc::ptr_eq(&reference_context, context) {
        return Err(HolonError::CrossTransactionReference {
            reference_kind: reference.reference_kind_string(),
            reference_id: reference.reference_id_string(),
            reference_tx: reference_context.tx_id().value(),
            context_tx: context.tx_id().value(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::test_support::{build_context, new_test_holon};
    use core_types::HolonId;

    #[test]
    fn saved_references_are_not_transaction_bound() -> Result<(), HolonError> {
        let first = build_context();
        let second = build_context();
        let saved = HolonReference::smart_from_id(
            first.space_read_handle(),
            HolonId::Local(core_types::LocalId(vec![7; 39])),
        );

        assert_reference_transaction_compatible(&saved, &second)?;
        let transient: HolonReference = new_test_holon(&first, "transient-descriptor")?.into();
        assert_reference_transaction_compatible(&transient, &first)?;
        assert!(matches!(
            assert_reference_transaction_compatible(&transient, &second),
            Err(HolonError::CrossTransactionReference { .. })
        ));

        let staged: HolonReference = first
            .mutation()
            .stage_new_holon(new_test_holon(&first, "transaction-local-descriptor")?)?
            .into();
        assert_reference_transaction_compatible(&staged, &first)?;
        assert!(matches!(
            assert_reference_transaction_compatible(&staged, &second),
            Err(HolonError::CrossTransactionReference { .. })
        ));
        Ok(())
    }
}
