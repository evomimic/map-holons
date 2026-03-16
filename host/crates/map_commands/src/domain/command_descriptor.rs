/// How a command affects transaction state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationClassification {
    ReadOnly,
    Mutating,
    /// Dance mutation detection deferred to Phase 2.3 (version counters).
    RuntimeDetected,
}

/// Static metadata describing a command's lifecycle requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandDescriptor {
    pub mutation: MutationClassification,
    pub requires_open_tx: bool,
    pub requires_commit_guard: bool,
}

impl CommandDescriptor {
    pub const fn read_only() -> Self {
        Self {
            mutation: MutationClassification::ReadOnly,
            requires_open_tx: false,
            requires_commit_guard: false,
        }
    }

    pub const fn mutating() -> Self {
        Self {
            mutation: MutationClassification::Mutating,
            requires_open_tx: true,
            requires_commit_guard: false,
        }
    }

    pub const fn mutating_with_guard() -> Self {
        Self {
            mutation: MutationClassification::Mutating,
            requires_open_tx: true,
            requires_commit_guard: true,
        }
    }
}
