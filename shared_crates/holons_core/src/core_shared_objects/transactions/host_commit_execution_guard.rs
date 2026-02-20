use core_types::HolonError;

use super::transaction_context::TransactionContext;

/// RAII guard for host-ingress commit execution.
///
/// Host-ingress concurrency guard only:
/// Prevents external mutation requests from racing an in-flight commit.
/// Not used by guest commit execution logic.
#[derive(Debug)]
pub struct HostCommitExecutionGuard<'a> {
    context: &'a TransactionContext,
}

impl<'a> HostCommitExecutionGuard<'a> {
    pub(crate) fn acquire(context: &'a TransactionContext) -> Result<Self, HolonError> {
        if !context.try_begin_host_commit_ingress() {
            return Err(HolonError::TransactionCommitInProgress {
                tx_id: context.tx_id().value(),
            });
        }

        Ok(Self { context })
    }
}

impl Drop for HostCommitExecutionGuard<'_> {
    fn drop(&mut self) {
        self.context.end_host_commit_ingress();
    }
}
