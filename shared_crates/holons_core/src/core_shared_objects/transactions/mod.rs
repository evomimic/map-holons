//! Transaction primitives and context types.
//!
//! Phase 1.3 lifecycle/concurrency intent in current implementation:
//! - Lifecycle: `Open -> Committed` (monotonic).
//! - Host external mutations/commit-like ingress require `Open`.
//! - Host commit ingress blocks overlapping external mutations.
//! - Read/query ingress may remain available during commit ingress and after `Committed`.

mod host_commit_execution_guard;
mod lookup_facade;
mod mutation_facade;
mod transaction_behavior;
mod transaction_context;
mod transaction_context_handle;
mod transaction_lifecycle_state;
mod transaction_manager;
mod tx_id;

pub use host_commit_execution_guard::HostCommitExecutionGuard;
pub use lookup_facade::LookupFacade;
pub use mutation_facade::MutationFacade;
pub use transaction_behavior::TransactionBehavior;
pub use transaction_context::TransactionContext;
pub use transaction_context_handle::TransactionContextHandle;
pub use transaction_lifecycle_state::TransactionLifecycleState;
pub use transaction_manager::TransactionManager;
pub use tx_id::TxId;
