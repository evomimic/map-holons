use super::VisualizerKind;
use holons_core::HolonReference;

/// Actual composition owner; its identity and ownership relationship are validated.
#[derive(Debug, Clone)]
pub enum VisualizerOwner {
    Visualizer(HolonReference),
    Dancer(HolonReference),
}

/// Bound selection context. Runtime capability comes from the host, never the caller.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub struct VisualizerCandidate {
    pub visualizer: HolonReference,
    pub declared_on: Vec<HolonReference>,
    pub assessment: VisualizerAssessment,
}

/// Read-only discovery. Current selection is caller-captured, not mounted-state proof.
#[derive(Debug, Clone)]
pub struct VisualizerDiscovery {
    pub candidates: Vec<VisualizerCandidate>,
    pub current_selection: Option<VisualizerCandidate>,
    pub ancestry: Vec<HolonReference>,
    pub stop_reason: DiscoveryStopReason,
    /// Optional host-retained evidence, projected without repeating discovery.
    pub snapshot: Option<base_types::MapString>,
}

/// Why the authoritative interactive traversal ended, not an automatic winner rationale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryStopReason {
    HolonTypeBoundary,
    LineageExhausted,
}

/// A persisted configuration anchor, prepared but not evidence of successful use.
#[derive(Debug, Clone)]
pub struct VisualizerUsageSelection {
    pub usage: HolonReference,
    pub initialized: bool,
    /// Host-session scope for publication ordering, independent of transaction identity.
    pub report_session: String,
}

/// Why a presentation was selected. Exploration never rewrites explicit preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VisualizerChoiceOrigin {
    Automatic,
    Explicit,
    Exploratory,
}

/// Correlation of one published choice. Sequence is shared by the local presentation client,
/// across occurrences and transactions, and is allocated only at publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualizerUseReport {
    pub session: String,
    pub occurrence_id: String,
    pub sequence: u64,
}
