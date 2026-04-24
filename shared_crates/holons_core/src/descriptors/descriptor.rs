use crate::reference_layer::HolonReference;

/// Common runtime contract for thin descriptor wrappers.
///
/// Descriptor wrappers stay intentionally small: they expose their backing
/// holon reference and layer any descriptor-specific behavior on top.
pub trait Descriptor {
    /// Returns the backing holon reference for this descriptor wrapper.
    fn holon(&self) -> &HolonReference;
}
