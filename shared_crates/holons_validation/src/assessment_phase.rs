//! Optional phase observation without host clocks or transport-specific instrumentation.

/// Read-only observation of the current Commit assessment phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssessmentPhase {
    Construction,
    PackageValidation,
    InstanceValidation,
}
