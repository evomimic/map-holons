use crate::dances::DanceResponseReference;
use crate::reference_layer::HolonReference;

/// An implementation may supply a body for normal response construction or a complete
/// typed response when it owns the response's properties and evidence relationships.
pub enum DanceImplementationResult {
    Body(Option<HolonReference>),
    Response(DanceResponseReference),
}
