use crate::reference_layer::HolonReference;

/// Compares references to one definition within the current assessment view.
/// Phase 4 will extend this comparison for staged replacements.
pub(crate) fn same_definition(left: &HolonReference, right: &HolonReference) -> bool {
    left == right
}

pub(crate) fn lineage_contains(lineage: &[HolonReference], anchor: &HolonReference) -> bool {
    lineage.iter().any(|member| same_definition(member, anchor))
}
