use crate::{get_original_holon_node, get_relationship_links};
use hdi::prelude::debug;
use holons_core::{Holon, HolonCollection, HolonError, HolonServiceApi, RelationshipName};
use shared_types_holon::{HolonId, LocalId};

pub struct GuestHolonService;

impl GuestHolonService {
    fn ensure_id_is_local(id: HolonId) -> Result<LocalId, HolonError> {
        match id {
            HolonId::Local(local_id) => Ok(local_id),
            HolonId::External(_) => {
                Err(HolonError::InvalidParameter("Expected LocalId, found ExternalId.".to_string()))
            }
        }
    }
}

impl HolonServiceApi for GuestHolonService {
    /// gets a specific HolonNode from the local persistent store based on the original ActionHash,
    /// then "inflates" the HolonNode into a Holon and returns it
    fn fetch_holon(&self, holon_id: &HolonId) -> Result<Holon, HolonError> {
        let local_id = Self::ensure_id_is_local(*holon_id.clone())?;

        let holon_node_record = get_original_holon_node(local_id.0.clone())?; // Retrieve the holon node
        if let Some(node) = holon_node_record {
            let holon = Holon::try_from_node(node)?;
            Ok(holon)
        } else {
            // No holon_node fetched for the specified holon_id
            Err(HolonError::HolonNotFound(local_id.0.to_string()))
        }
    }

    fn fetch_related_holons(
        &self,
        source_id: HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
        let local_id = Self::ensure_id_is_local(*source_id.clone())?;

        let mut collection = HolonCollection::new_existing();

        // fetch the smartlinks for this relationship (if any)
        let smartlinks = get_relationship_links(local_id.0, relationship_name)?;
        debug!("Got {:?} smartlinks: {:#?}", smartlinks.len(), smartlinks);

        for smartlink in smartlinks {
            let holon_reference = smartlink.to_holon_reference();
            collection.add_reference_with_key(smartlink.get_key().as_ref(), &holon_reference)?;
        }
        Ok(collection)
    }
}
