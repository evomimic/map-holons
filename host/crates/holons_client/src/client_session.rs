use std::sync::Arc;
use core_types::HolonError;
use holons_core::core_shared_objects::{space_manager::HolonSpaceManager, transactions::TransactionContext, transactions::TxId};

use crate::Receptor;

pub struct ClientSession {
    _context: Arc<TransactionContext>,
    // Other session-related fields can be added here
}

impl ClientSession {
    pub fn new(space_manager: Arc<HolonSpaceManager>, recovery: Option<Arc<Receptor>>, _destination: Option<Arc<Receptor>>) -> Result<Self, HolonError> {
        if let Some(receptor_arc) = recovery {
            if let Receptor::LocalRecovery(receptor) = receptor_arc.as_ref() {
                if let Some(tx_id) = receptor.recover_last_tx_id_from_crash() {
                    tracing::info!("[CLIENT SESSION] Orphaned transaction found from previous crash: {}", tx_id.clone());
                    let transaction_context = space_manager
                        .get_transaction_manager()
                        .open_transaction_with_id(Arc::clone(&space_manager), TxId::from_str(&tx_id).expect("invalid tx_id"))
                        .or_else(|_| Err(HolonError::FailedToBorrow("[CLIENT SESSION] Failed to open transaction for crash recovery".into())))?;
                        receptor.init_session(transaction_context.clone());
                        return Ok(Self {
                            _context: transaction_context,
                        });
                } else {
                    let transaction_context = space_manager
                        .get_transaction_manager()
                        .open_new_transaction(Arc::clone(&space_manager))
                        .or_else(|_| Err(HolonError::FailedToBorrow("[CLIENT SESSION] Failed to open transaction for crash recovery".into())))?;
                        receptor.init_session(transaction_context.clone());
                        return Ok(Self {
                            _context: transaction_context,
                        });
                }
            } else {
                return Err(HolonError::FailedToBorrow("[CLIENT SESSION] Provided recovery receptor is not a LocalRecoveryReceptor".into()));
            }
        } 
        Err(HolonError::FailedToBorrow("[CLIENT SESSION] No valid recovery receptor provided".into()))
    }

    // todo: add methods for session operations, e.g. add, save, undo, list, etc.
    // commit could be handled by the receptor or by the session itself depending on design decisions around who owns the transaction context and how tightly coupled the session is to the recovery mechanism.

    // Additional methods for session management can be added here
}