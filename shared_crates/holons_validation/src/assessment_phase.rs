//! Optional phase observation without host clocks or transport-specific instrumentation.

/// Read-only observation of the current Commit assessment phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssessmentPhase {
    Construction,
    ReadinessResolution,
    SubjectPreparation,
    SchemaWorksetPreparation,
    PackageDiscovery,
    PreparedProducts,
    PackageValidation,
    InstanceValidation,
}

/// Aggregate preparation demand. Counts describe reference requests, not backend cache misses.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreparationMetrics {
    pub package_roots: usize,
    pub subjects: usize,
    pub affected_schemas: usize,
    pub distinct_content: usize,
    pub identity_role_visits: usize,
    pub contract_consumers: usize,
    pub constraint_consumers: usize,
    pub binding_consumers: usize,
}

/// Clock-free observation; profiling cannot supply inputs or change validation policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssessmentEvent {
    Phase(AssessmentPhase),
    Preparation(PreparationMetrics),
    PreparationVisitStarted,
    PreparationVisitFinished { type_name: String, distinct_content: bool },
    PreparationOperation { operation: &'static str, role: &'static str, starting: bool },
    PreparationEdge { role: &'static str, edge: String, requests: usize, members: usize },
}
