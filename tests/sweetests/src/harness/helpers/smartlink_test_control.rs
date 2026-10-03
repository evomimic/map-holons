//! Restricted storage fault injection for isolated relationship-persistence tests.

use super::MockConductorConfig;
use core_types::{DeleteSmartLinkOutcome, SmartLink, SmartLinkId};
use integrity_core_types::{LocalId, RelationshipName};
use std::sync::Arc;

/// Holds a probe-enabled backend without exposing it to ordinary fixture executors.
pub struct SmartLinkTestControl {
    backend: Arc<MockConductorConfig>,
}

impl SmartLinkTestControl {
    pub(super) fn new(backend: Arc<MockConductorConfig>) -> Self {
        Self { backend }
    }

    /// Authors a raw tag through the probe, allowing a stale canonical key.
    pub async fn plant_stale_link(
        &self,
        source: LocalId,
        target: LocalId,
        encoded_tag: Vec<u8>,
    ) -> SmartLinkId {
        let id: LocalId = self
            .backend
            .conductor
            .call(
                &self.backend.cell.zome("holons_test_probes"),
                "smartlink_author_raw_tag_for_test",
                (source, target, encoded_tag),
            )
            .await;
        id.into()
    }

    /// Deletes the injected link through the production coordinator.
    pub async fn delete_link(&self, link_id: SmartLinkId) -> DeleteSmartLinkOutcome {
        self.backend
            .conductor
            .call(&self.backend.cell.zome("holons"), "smartlink_delete", link_id)
            .await
    }

    /// Reads live storage rows, including canonical keys hidden by staged content.
    pub async fn live_links(
        &self,
        source: LocalId,
        relationship: RelationshipName,
    ) -> Vec<SmartLink> {
        self.backend
            .conductor
            .call(&self.backend.cell.zome("holons"), "smartlink_expand", (source, relationship))
            .await
    }
}
