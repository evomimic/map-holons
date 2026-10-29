use core_types::HolonError;

use crate::reference_layer::{DefinitionIdentity, HolonReference};

/// Compares exact definition identities, including saved definitions represented by
/// no-op or graph-only handles. Keys and version-producing ancestry do not establish
/// equality. Mutable references must belong to the same transaction.
pub fn same_definition(left: &HolonReference, right: &HolonReference) -> Result<bool, HolonError> {
    let context = left.transaction_context().or_else(|| right.transaction_context());
    Ok(DefinitionIdentity::for_optional_context(left, context.as_ref())?
        == DefinitionIdentity::for_optional_context(right, context.as_ref())?)
}

/// Searches bound references for an exact definition, resolving the anchor identity
/// once and stopping at the first match. Empty inputs perform no identity reads.
/// Mutable references examined by the search must belong to the same transaction.
pub fn lineage_contains<'a>(
    lineage: impl IntoIterator<Item = &'a HolonReference>,
    anchor: &HolonReference,
) -> Result<bool, HolonError> {
    let mut members = lineage.into_iter();
    let Some(first) = members.next() else {
        return Ok(false);
    };
    let mut context = anchor.transaction_context().or_else(|| first.transaction_context());
    let anchor_identity = DefinitionIdentity::for_optional_context(anchor, context.as_ref())?;
    for member in std::iter::once(first).chain(members) {
        if context.is_none() {
            context = member.transaction_context();
        }
        if DefinitionIdentity::for_optional_context(member, context.as_ref())? == anchor_identity {
            return Ok(true);
        }
    }
    Ok(false)
}
