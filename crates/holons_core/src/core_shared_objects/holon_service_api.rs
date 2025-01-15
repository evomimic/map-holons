use crate::{Holon, HolonCollection, HolonError, RelationshipName};
use shared_types_holon::HolonId;
use std::fmt::Debug;

pub trait HolonServiceApi: Debug {
    fn fetch_holon(&self, id: &HolonId) -> Result<Holon, HolonError>;

    fn fetch_related_holons(
        &self,
        source_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<HolonCollection, HolonError>;
}
