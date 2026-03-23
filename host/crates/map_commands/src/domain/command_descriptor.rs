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
    /// Read-only descriptor for transaction-scoped commands.
    ///
    /// All transaction commands require an open transaction — even lookups —
    /// because a committed transaction must reject all further operations.
    pub const fn transaction_read_only() -> Self {
        Self {
            mutation: MutationClassification::ReadOnly,
            requires_open_tx: true,
            requires_commit_guard: false,
        }
    }

    /// Read-only descriptor for holon-scoped commands.
    ///
    /// Holon reads do not require an open transaction because references from
    /// committed transactions remain alive and accessible.
    pub const fn holon_read_only() -> Self {
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
