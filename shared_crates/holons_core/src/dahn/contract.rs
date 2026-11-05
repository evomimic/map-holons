use crate::HolonReference;

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

/// Live configuration, ready independently of background persistence; not successful-use evidence.
#[derive(Debug, Clone)]
pub struct VisualizerUsageSelection {
    pub usage: HolonReference,
    pub initialized: bool,
    /// Host-session scope for publication ordering, independent of transaction identity.
    pub report_session: String,
}

/// DAHN-wide classification of the Visualizer required by a request.
///
/// A kind constrains the class of Visualizer that can satisfy a request; it
/// is not a local composition Slot and does not imply one implementation per
/// kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualizerKind {
    TitleBar,
    Canvas,
    Node,
    /// Visualizes an organizing semantic topology.
    Structure,
    RootedNavigation,
    Collection,
    PropertyMap,
    /// Visualizes one descriptor-defined property within a PropertyMap visualizer.
    /// The request subject is the bound `PropertyDescriptor` holon, never a
    /// raw entry from a PropertyMap.
    Property,
    Value,
    /// Composes action slots for an affording holon.
    ActionBar,
    /// Selects an individual action using its Dance descriptor as subject.
    Action,
}

#[derive(Debug)]
pub struct VisualizerSelection {
    pub selected: HolonReference,
    pub requested_kind: VisualizerKind,
    pub alternatives_available: bool,
    /// Live configuration prepared in the same operation during an open Usage round.
    pub usage: Option<VisualizerUsageSelection>,
}
