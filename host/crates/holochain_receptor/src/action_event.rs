use crate::host_signal::HolonsZomeSignal;
use integrity_core_types::{LocalId, PersistenceTimestamp};

/// Classification of the mutation that triggered the event.
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
/// boundary into the runtime-consumer layer. Consumers who need the current holon
/// state resolve it through the normal `HolonReference` / cache path using
/// `affected_holon`.
///
/// `HostSignal` and `HolonsZomeSignal` are adapter-internal; this type is the
/// public subscription surface.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActionEvent {
    /// What kind of mutation occurred.
    pub mutation_kind: MutationKind,
    /// The Holochain action hash for this specific commit (39 bytes).
    pub action_id: LocalId,
    /// The permanent identity of the affected holon (the original CREATE hash).
    /// For created holons this equals `action_id`. For updates/deletes it is the
    /// holon's lineage root. For link events it equals `action_id`.
    pub affected_holon: LocalId,
    /// The predecessor record being superseded — `update.original_action_address`
    /// for updates, `delete.deletes_address` for deletes. `None` for creates and
    /// link events.
    pub previous_holon: Option<LocalId>,
    /// The committing action's timestamp.
    pub timestamp: PersistenceTimestamp,
    /// For link events: the link type string (`format!("{:?}", LinkTypes)` from the guest).
    /// `None` for holon mutation events.
    pub link_type: Option<String>,
}

/// Convert an adapter-internal `HolonsZomeSignal` into the MAP-facing `ActionEvent`.
pub(crate) fn to_action_event(zs: &HolonsZomeSignal) -> ActionEvent {
    match zs {
        HolonsZomeSignal::HolonCreated { action_id, affected_holon, timestamp } => ActionEvent {
            mutation_kind: MutationKind::HolonCreated,
            action_id: action_id.clone(),
            affected_holon: affected_holon.clone(),
            previous_holon: None,
            timestamp: timestamp.clone(),
            link_type: None,
        },
        HolonsZomeSignal::HolonUpdated { action_id, affected_holon, previous_holon, timestamp } => {
            ActionEvent {
                mutation_kind: MutationKind::HolonUpdated,
                action_id: action_id.clone(),
                affected_holon: affected_holon.clone(),
                previous_holon: Some(previous_holon.clone()),
                timestamp: timestamp.clone(),
                link_type: None,
            }
        }
        HolonsZomeSignal::HolonDeleted { action_id, affected_holon, previous_holon, timestamp } => {
            ActionEvent {
                mutation_kind: MutationKind::HolonDeleted,
                action_id: action_id.clone(),
                affected_holon: affected_holon.clone(),
                previous_holon: Some(previous_holon.clone()),
                timestamp: timestamp.clone(),
                link_type: None,
            }
        }
        HolonsZomeSignal::LinkCreated { action_id, link_type, timestamp } => ActionEvent {
            mutation_kind: MutationKind::LinkCreated,
            action_id: action_id.clone(),
            affected_holon: action_id.clone(),
            previous_holon: None,
            timestamp: timestamp.clone(),
            link_type: Some(link_type.clone()),
        },
        HolonsZomeSignal::LinkDeleted { action_id, link_type, timestamp } => ActionEvent {
            mutation_kind: MutationKind::LinkDeleted,
            action_id: action_id.clone(),
            affected_holon: action_id.clone(),
            previous_holon: None,
            timestamp: timestamp.clone(),
            link_type: Some(link_type.clone()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(seed: u8) -> LocalId {
        LocalId(vec![seed; 39])
    }

    fn ts() -> PersistenceTimestamp {
        PersistenceTimestamp(1_700_000_000_000_000)
    }

    #[test]
    fn holon_created_maps_fields() {
        let zs = HolonsZomeSignal::HolonCreated {
            action_id: id(0xAA),
            affected_holon: id(0xAA),
            timestamp: ts(),
        };
        let e = to_action_event(&zs);
        assert_eq!(e.mutation_kind, MutationKind::HolonCreated);
        assert_eq!(e.action_id.0, vec![0xAA; 39]);
        assert_eq!(e.affected_holon.0, vec![0xAA; 39]);
        assert!(e.previous_holon.is_none());
        assert_eq!(e.timestamp, ts());
        assert!(e.link_type.is_none());
    }

    #[test]
    fn holon_updated_sets_previous_holon() {
        let zs = HolonsZomeSignal::HolonUpdated {
            action_id: id(0xBB),
            affected_holon: id(0x11),
            previous_holon: id(0x22),
            timestamp: ts(),
        };
        let e = to_action_event(&zs);
        assert_eq!(e.mutation_kind, MutationKind::HolonUpdated);
        assert_eq!(e.affected_holon.0, vec![0x11; 39]);
        assert_eq!(e.previous_holon.as_ref().unwrap().0, vec![0x22; 39]);
    }

    #[test]
    fn holon_deleted_sets_previous_holon() {
        let zs = HolonsZomeSignal::HolonDeleted {
            action_id: id(0xCC),
            affected_holon: id(0x33),
            previous_holon: id(0x44),
            timestamp: ts(),
        };
        let e = to_action_event(&zs);
        assert_eq!(e.mutation_kind, MutationKind::HolonDeleted);
        assert_eq!(e.affected_holon.0, vec![0x33; 39]);
        assert_eq!(e.previous_holon.as_ref().unwrap().0, vec![0x44; 39]);
    }

    #[test]
    fn link_created_sets_link_type_and_no_previous() {
        let zs = HolonsZomeSignal::LinkCreated {
            action_id: id(0xDD),
            link_type: "SmartLink".to_string(),
            timestamp: ts(),
        };
        let e = to_action_event(&zs);
        assert_eq!(e.mutation_kind, MutationKind::LinkCreated);
        assert_eq!(e.affected_holon, e.action_id);
        assert!(e.previous_holon.is_none());
        assert_eq!(e.link_type.as_deref(), Some("SmartLink"));
    }

    #[test]
    fn link_deleted_sets_link_type() {
        let zs = HolonsZomeSignal::LinkDeleted {
            action_id: id(0xEE),
            link_type: "AllHolonNodes".to_string(),
            timestamp: ts(),
        };
        let e = to_action_event(&zs);
        assert_eq!(e.mutation_kind, MutationKind::LinkDeleted);
        assert_eq!(e.link_type.as_deref(), Some("AllHolonNodes"));
    }

    #[test]
    fn timestamp_is_carried_through() {
        let zs = HolonsZomeSignal::HolonCreated {
            action_id: id(1),
            affected_holon: id(1),
            timestamp: PersistenceTimestamp(42),
        };
        assert_eq!(to_action_event(&zs).timestamp, PersistenceTimestamp(42));
    }
}
