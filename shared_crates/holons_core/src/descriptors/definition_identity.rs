use crate::reference_layer::{HolonReference, ProspectiveIdentity};

/// Compares reference identity in the current view, without reads or replacement ancestry.
/// Prospective callers select both references through their reader before comparison.
pub fn same_definition(left: &HolonReference, right: &HolonReference) -> bool {
    left == right
}

pub(crate) fn lineage_contains(lineage: &[HolonReference], anchor: &HolonReference) -> bool {
    lineage.iter().any(|member| same_definition(member, anchor))
}

/// Full typed identity for traversal cycle detection (diagnostic strings may be lossy).
/// Staged and transient identities omit the transaction ID. `ExtendsIter` does not
/// assert transaction compatibility, so equal temporary IDs across transactions
/// could be mistaken for a cycle; callers must keep lineage traversal in one transaction.
pub(crate) fn definition_identity(reference: &HolonReference) -> ProspectiveIdentity {
    // Preserve phase identity outside assessment. A prospective walk has already
    // selected replacements, so repeated definitions have the same selected handle.
    match reference {
        HolonReference::Smart(saved) => ProspectiveIdentity::Saved(saved.holon_id()),
        HolonReference::Staged(staged) => ProspectiveIdentity::Staged(staged.temporary_id()),
        HolonReference::Transient(transient) => {
            ProspectiveIdentity::Transient(transient.temporary_id())
        }
    }
}
