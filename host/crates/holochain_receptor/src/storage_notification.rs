use crate::host_signal::HolonsZomeSignal;
use integrity_core_types::LocalId;

/// Classification of the mutation that triggered the notification.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MutationKind {
    HolonCreated,
    HolonUpdated,
    HolonDeleted,
    LinkCreated,
    LinkDeleted,
}

/// MAP-facing adapter event: notifies that a change occurred at the identified holon.
///
/// Carries NO holon state — this is the only signal type that crosses the adapter
/// boundary into the Integration Hub or runtime-consumer layer. Consumers who need
/// the current holon state resolve it through the normal `HolonReference` / cache
/// path using `holon_id`.
///
/// `HostSignal` and `HolonsZomeSignal` are adapter-internal; this type is the
/// public subscription surface.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StorageNotification {
    /// What kind of mutation occurred.
    pub mutation_kind: MutationKind,
    /// The Holochain action hash for this specific commit (39 bytes).
    pub action_id: LocalId,
    /// The permanent identity of the affected holon — the original CREATE action hash.
    /// For new holons this equals `action_id`. For updates/deletes it is the original
    /// holon's CREATE hash, derived from `HolonNode.original_id` inside the adapter.
    /// For link events this equals `action_id`.
    pub holon_id: LocalId,
    /// For link events: the link type string (`format!("{:?}", LinkTypes)` from the guest).
    /// `None` for holon mutation events.
    pub link_type: Option<String>,
}

/// Convert an adapter-internal `HolonsZomeSignal` into the MAP-facing `StorageNotification`.
///
/// `holon_id` is derived from `HolonNodeModel.original_id` (the original CREATE hash).
/// For freshly-created holons `original_id` is `None`; the action hash IS the new
/// holon's permanent identity, so we fall back to `action_id`.
pub(crate) fn to_storage_notification(zs: &HolonsZomeSignal) -> StorageNotification {
    match zs {
        HolonsZomeSignal::HolonCreated { action_id, holon } => StorageNotification {
            mutation_kind: MutationKind::HolonCreated,
            action_id: action_id.clone(),
            holon_id: holon.original_id.clone().unwrap_or_else(|| action_id.clone()),
            link_type: None,
        },
        HolonsZomeSignal::HolonUpdated { action_id, holon, .. } => StorageNotification {
            mutation_kind: MutationKind::HolonUpdated,
            action_id: action_id.clone(),
            holon_id: holon.original_id.clone().unwrap_or_else(|| action_id.clone()),
            link_type: None,
        },
        HolonsZomeSignal::HolonDeleted { action_id, original_holon } => StorageNotification {
            mutation_kind: MutationKind::HolonDeleted,
            action_id: action_id.clone(),
            holon_id: original_holon.original_id.clone().unwrap_or_else(|| action_id.clone()),
            link_type: None,
        },
        HolonsZomeSignal::LinkCreated { action_id, link_type } => StorageNotification {
            mutation_kind: MutationKind::LinkCreated,
            action_id: action_id.clone(),
            holon_id: action_id.clone(),
            link_type: Some(link_type.clone()),
        },
        HolonsZomeSignal::LinkDeleted { action_id, link_type } => StorageNotification {
            mutation_kind: MutationKind::LinkDeleted,
            action_id: action_id.clone(),
            holon_id: action_id.clone(),
            link_type: Some(link_type.clone()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use integrity_core_types::{HolonNodeModel, PropertyMap};

    fn id(seed: u8) -> LocalId {
        LocalId(vec![seed; 39])
    }

    fn holon_with_original(original_seed: u8) -> HolonNodeModel {
        HolonNodeModel::new(Some(id(original_seed)), PropertyMap::new())
    }

    fn holon_without_original() -> HolonNodeModel {
        HolonNodeModel::new(None, PropertyMap::new())
    }

    #[test]
    fn holon_created_uses_original_id_when_present() {
        let zs = HolonsZomeSignal::HolonCreated {
            action_id: id(0xAA),
            holon: holon_with_original(0x01),
        };
        let n = to_storage_notification(&zs);
        assert_eq!(n.mutation_kind, MutationKind::HolonCreated);
        assert_eq!(n.action_id.0, vec![0xAA; 39]);
        assert_eq!(n.holon_id.0, vec![0x01; 39]); // from original_id
        assert!(n.link_type.is_none());
    }

    #[test]
    fn holon_created_falls_back_to_action_id_when_no_original() {
        let zs =
            HolonsZomeSignal::HolonCreated { action_id: id(0xBB), holon: holon_without_original() };
        let n = to_storage_notification(&zs);
        assert_eq!(n.holon_id.0, vec![0xBB; 39]); // falls back to action_id
    }

    #[test]
    fn holon_updated_uses_original_id() {
        let zs = HolonsZomeSignal::HolonUpdated {
            action_id: id(0xCC),
            holon: holon_with_original(0x02),
            original_holon: holon_with_original(0x02),
        };
        let n = to_storage_notification(&zs);
        assert_eq!(n.mutation_kind, MutationKind::HolonUpdated);
        assert_eq!(n.holon_id.0, vec![0x02; 39]);
    }

    #[test]
    fn holon_deleted_uses_original_holons_id() {
        let zs = HolonsZomeSignal::HolonDeleted {
            action_id: id(0xDD),
            original_holon: holon_with_original(0x03),
        };
        let n = to_storage_notification(&zs);
        assert_eq!(n.mutation_kind, MutationKind::HolonDeleted);
        assert_eq!(n.holon_id.0, vec![0x03; 39]);
    }

    #[test]
    fn link_created_carries_link_type() {
        let zs = HolonsZomeSignal::LinkCreated {
            action_id: id(0xEE),
            link_type: "SmartLink".to_string(),
        };
        let n = to_storage_notification(&zs);
        assert_eq!(n.mutation_kind, MutationKind::LinkCreated);
        assert_eq!(n.holon_id, n.action_id);
        assert_eq!(n.link_type.as_deref(), Some("SmartLink"));
    }

    #[test]
    fn link_deleted_carries_link_type() {
        let zs = HolonsZomeSignal::LinkDeleted {
            action_id: id(0xFF),
            link_type: "AllHolonNodes".to_string(),
        };
        let n = to_storage_notification(&zs);
        assert_eq!(n.mutation_kind, MutationKind::LinkDeleted);
        assert_eq!(n.link_type.as_deref(), Some("AllHolonNodes"));
    }
}
