use std::fmt::Debug;
use std::sync::{Arc, RwLock};

use crate::core_shared_objects::transactions::TransactionContext;
use crate::core_shared_objects::Holon;
use crate::{HolonCollection, RelationshipMap};
use core_types::{HolonError, HolonId, RelationshipName};

pub trait HolonCacheAccess: Debug + Send + Sync {
    /// Returns a resident saved holon without starting a cache-fill transaction.
    fn get_cached_rc_holon(
        &self,
        holon_id: &HolonId,
    ) -> Result<Option<Arc<RwLock<Holon>>>, HolonError>;

    /// This method returns a mutable reference to the Holon identified by holon_id.
    /// If holon_id is `Local`, it retrieves the holon from the local cache. If the holon is not
    /// already resident in the cache, this function first fetches the holon from the persistent
    /// store and inserts it into the cache before returning the reference to that holon.
    /// If the holon_id is `External`, this method currently returns a `NotImplemented` HolonError
    ///
    fn get_rc_holon(
        &self,
        context: &Arc<TransactionContext>,
        holon_id: &HolonId,
    ) -> Result<Arc<RwLock<Holon>>, HolonError>;

    /// Retrieves related holons for the given source holon.
    ///
    /// # Returns
    /// - An `Arc<RwLock<HolonCollection>>` containing the related holons for thread-safe access.
    fn get_related_holons(
        &self,
        context: &Arc<TransactionContext>,
        source_holon_id: &HolonId,
        relationship_name: &RelationshipName,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError>;

    /// Reads membership with a caller freshness requirement.
    fn get_related_holons_with_hint(
        &self,
        context: &Arc<TransactionContext>,
        source: &HolonId,
        name: &RelationshipName,
        hint: crate::RelationshipReadHint,
    ) -> Result<Arc<RwLock<HolonCollection>>, HolonError> {
        if hint == crate::RelationshipReadHint::RequireFresh {
            return Err(HolonError::NotImplemented(
                "fresh relationship reads unsupported by cache adapter".into(),
            ));
        }
        self.get_related_holons(context, source, name)
    }

    /// Retrieves all relationships visible from a saved source.
    fn get_all_related_holons(
        &self,
        context: &Arc<TransactionContext>,
        source_holon_id: &HolonId,
    ) -> Result<RelationshipMap, HolonError>;
}
