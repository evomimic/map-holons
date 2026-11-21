//! Canonical descriptor keys for DAHN-owned record types.
use base_types::MapString;
#[derive(Debug, Clone, Copy)]
pub enum DahnHolonTypeName {
    VisualizerPreference,
    VisualizerSlotPreference,
    VisualizerUsage,
    VisualizerSlotDefault,
    ResolveCompositionRequest,
    ResolvedComposition,
}
impl DahnHolonTypeName {
    pub fn descriptor_key(self) -> MapString {
        MapString(format!(
            "{self:?}.{}",
            if matches!(self, Self::ResolvedComposition) { "Projection" } else { "HolonType" }
        ))
    }
}
