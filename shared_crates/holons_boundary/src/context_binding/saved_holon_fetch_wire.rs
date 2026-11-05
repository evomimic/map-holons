use super::HolonWire;
use core_types::{HolonError, HolonId};
use holons_core::core_shared_objects::cache_population::{validate_saved_content, SavedHolonFetch};
use holons_core::core_shared_objects::Holon;
use serde::{Deserialize, Serialize};

/// Exact-ID hydration carrier, used only for saved-cache infrastructure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SavedHolonFetchWire {
    pub id: HolonId,
    pub result: Result<HolonWire, HolonError>,
}
impl SavedHolonFetchWire {
    /// Rejects non-saved state without binding or admitting its nested references.
    pub fn bind(self) -> SavedHolonFetch {
        let result = self.result.and_then(|wire| match wire {
            HolonWire::Saved(saved) => {
                let holon = Holon::Saved(saved);
                validate_saved_content(&self.id, &holon)?;
                Ok(holon)
            }
            _ => Err(HolonError::InvalidParameter("Saved hydration requires saved state".into())),
        });
        SavedHolonFetch { id: self.id, result }
    }
}
impl From<&SavedHolonFetch> for SavedHolonFetchWire {
    fn from(entry: &SavedHolonFetch) -> Self {
        Self {
            id: entry.id.clone(),
            result: entry.result.as_ref().map(HolonWire::from).map_err(Clone::clone),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DanceRequestWire, RequestBodyWire, ResponseBodyWire};
    use base_types::MapInteger;
    use core_types::{LocalId, PropertyMap};
    use holons_core::core_shared_objects::holon::{HolonState, ValidationState};
    use holons_core::core_shared_objects::{SavedHolon, TransientHolon, TransientRelationshipMap};
    use holons_core::dances::{DanceRequest, DanceType, RequestBody};

    fn id(n: u8) -> HolonId {
        HolonId::Local(LocalId(vec![n]))
    }
    fn saved(n: u8) -> Holon {
        Holon::Saved(SavedHolon::new(LocalId(vec![n]), PropertyMap::new(), None, MapInteger(1)))
    }

    #[test]
    fn hydration_projection_and_json_preserve_identity_errors_and_order() {
        let entries = vec![
            SavedHolonFetch { id: id(2), result: Err(HolonError::HolonNotFound("absent".into())) },
            SavedHolonFetch { id: id(1), result: Ok(saved(1)) },
        ];
        let body = holons_core::dances::ResponseBody::SavedHolonHydration(entries.clone());
        let wire = ResponseBodyWire::from(&body);
        let json = serde_json::to_vec(&wire).unwrap();
        let ResponseBodyWire::SavedHolonHydration(wires) = serde_json::from_slice(&json).unwrap()
        else {
            panic!("hydration carrier");
        };
        assert_eq!(wires.into_iter().map(|wire| wire.bind()).collect::<Vec<_>>(), entries);
    }

    #[test]
    fn invalid_saved_shape_is_an_entry_error_without_losing_valid_entries() {
        let transient = Holon::Transient(TransientHolon::from_parts(
            MapInteger(1),
            HolonState::Mutable,
            ValidationState::ValidationRequired,
            PropertyMap::new(),
            TransientRelationshipMap::new_empty(),
            None,
        ));
        let entries = vec![
            SavedHolonFetch { id: id(1), result: Ok(saved(1)) },
            SavedHolonFetch { id: id(2), result: Ok(saved(3)) },
            SavedHolonFetch { id: id(4), result: Ok(transient) },
        ];
        let bound: Vec<_> =
            entries.iter().map(SavedHolonFetchWire::from).map(|wire| wire.bind()).collect();
        assert_eq!(bound[0], entries[0]);
        assert!(bound[1].result.is_err());
        assert!(bound[2].result.is_err());
    }

    #[test]
    fn bulk_request_projection_retains_complete_ids_and_duplicate_slots() {
        let external = HolonId::External(core_types::ExternalId {
            space_id: core_types::OutboundProxyId(LocalId(vec![9])),
            local_id: LocalId(vec![1]),
        });
        let ids = vec![id(1), external, id(1)];
        let request = DanceRequest::new(
            "get_holons_by_ids".into(),
            DanceType::Standalone,
            RequestBody::SavedHolonIds(ids.clone()),
        );
        let wire = DanceRequestWire::from(&request);
        let json = serde_json::to_vec(&wire).unwrap();
        let decoded: DanceRequestWire = serde_json::from_slice(&json).unwrap();
        assert_eq!(decoded.body, RequestBodyWire::SavedHolonIds(ids));
    }
}
