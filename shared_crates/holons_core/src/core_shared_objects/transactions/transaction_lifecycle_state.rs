//! Transaction lifecycle state model.
//!
//! ## Why this is stored atomically
//!
//! `TransactionContext` is shared broadly as `Arc<TransactionContext>` and most call paths
//! only hold `&self`, not `&mut self`. Lifecycle must still transition at runtime
//! (`Open -> Committing -> Committed`), so we need interior mutability.
//!
//! We store lifecycle as an atomic primitive in `TransactionContext` because:
//! - state checks are frequent and should be lock-free on hot paths,
//! - transitions may occur while other threads are reading state,
//! - a plain enum field would require exclusive mutable access that we do not have.
//!
//! A lock-based alternative (for example `Mutex<TransactionLifecycleState>`) would also be
//! correct, but introduces lock overhead and poisoning/error handling complexity for a very
//! small state machine.

/// Lifecycle state for a transaction context.
///
/// Notes:
/// - `Open`: normal execution state.
/// - `Committing`: transitional lock state while commit is in progress.
/// - `Committed`: terminal closed state after a successful commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TransactionLifecycleState {
    Open = 0,
    Committing = 1,
    Committed = 2,
}

impl TransactionLifecycleState {
    pub(crate) fn as_u8(self) -> u8 {
        self as u8
    }

    pub(crate) fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Open,
            1 => Self::Committing,
            2 => Self::Committed,
            _ => {
                debug_assert!(
                    false,
                    "Invalid lifecycle state value {} in TransactionContext",
                    value
                );
                // Restrictive fallback for impossible/corrupt values.
                Self::Committed
            }
        }
    }
}
