use super::VisualizerKind;
use holons_core::HolonReference;

/// Actual composition owner; its identity and ownership relationship are validated.
#[derive(Debug)]
pub enum VisualizerOwner {
    Visualizer(HolonReference),
    Dancer(HolonReference),
}

/// Bound selection context. Runtime capability comes from the host, never the caller.
#[derive(Debug)]
pub struct VisualizerSelectionRequest {
    pub subject: HolonReference,
    pub requested_kind: VisualizerKind,
    pub owner: VisualizerOwner,
    pub slot: HolonReference,
    /// Semantic Theme; Rust derives its effective MDS.
    pub theme: HolonReference,
}

/// Machine-readable assessment. Structural/evaluation errors remain Result failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualizerAssessment {
    Viable,
    IncompatibleSlot,
    IncompatibleTheme,
    ImplementationUnavailable,
    NoLongerApplicable,
}

/// One semantic candidate and all permitted local declarations contributing it.
#[derive(Debug)]
pub struct VisualizerCandidate {
    pub visualizer: HolonReference,
    pub declared_on: Vec<HolonReference>,
    pub assessment: VisualizerAssessment,
}

/// Read-only discovery. Current selection is caller-captured, not mounted-state proof.
#[derive(Debug)]
pub struct VisualizerDiscovery {
    pub candidates: Vec<VisualizerCandidate>,
    pub current_selection: Option<VisualizerCandidate>,
    pub ancestry: Vec<HolonReference>,
}
