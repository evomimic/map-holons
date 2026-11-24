//! Transport for the Visualizer registry's separate, restricted owner transaction.
use super::SessionStateWire;
use core_types::{HolonError, TemporaryId};
use holons_core::{
    core_shared_objects::transactions::TransactionContext,
    dahn::evaluation_registry::VisualizerEvaluationRegistry,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Only this participant's private root and pool cross a composition boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualizerEvaluationsWire {
    pub root: TemporaryId,
    pub owner: SessionStateWire,
}
impl VisualizerEvaluationsWire {
    /// Captures a coherent decision set without serializing another registry's products.
    pub fn capture(registry: &VisualizerEvaluationRegistry) -> Result<Self, HolonError> {
        registry.capture(|owner, root| {
            Ok(Self { root: root.clone(), owner: SessionStateWire::from_context(owner)? })
        })
    }
    /// Binds only a restricted, same-space, unstaged private carrier.
    pub fn bind(
        self,
        agent: &Arc<TransactionContext>,
    ) -> Result<Arc<VisualizerEvaluationRegistry>, HolonError> {
        let owner = super::registry_owner::validate_registry_owner(&self.owner, agent)?;
        VisualizerEvaluationRegistry::restore(agent, owner, self.root, |context| {
            self.owner.get_transient_holons().clone().bind(context)
        })
    }
}
