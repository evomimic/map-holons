pub use holons_core::dahn::contract::{
    DiscoveryStopReason, VisualizerAssessment, VisualizerCandidate, VisualizerDiscovery,
    VisualizerKind, VisualizerOwner, VisualizerSelection, VisualizerSelectionRequest,
    VisualizerUsageSelection,
};

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

/// Status of a session-local background persistence batch.
#[derive(Debug)]
pub struct VisualizerUsagePersistence {
    pub batch_id: base_types::MapString,
    pub state: String,
    pub error: Option<String>,
}
