use crate::HolonReferenceWire;
use base_types::MapString;
use core_types::HolonError;
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::{CollectionState, HolonCollection};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct HolonCollectionWire {
    pub state: CollectionState,
    pub members: Vec<HolonReferenceWire>,
    pub keyed_index: BTreeMap<MapString, usize>,
}

impl HolonCollectionWire {
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<HolonCollection, HolonError> {
        // Validate that keyed_index does not contain out-of-bounds indices.
        // This protects runtime code from panics and makes corrupted or malicious
        // wire data fail deterministically.
        //
        // NOTE: We intentionally do *not* validate that keyed_index keys match
        // the member holons' derived keys here. That stronger invariant may be
        // added later once key derivation is guaranteed to be cheap and
        // transaction-safe during bind.
        for (key, index) in &self.keyed_index {
            if *index >= self.members.len() {
                return Err(HolonError::InvalidWireFormat {
                    wire_type: "HolonCollectionWire".to_string(),
                    reason: format!(
                        "keyed_index out of bounds: key={:?} index={} members_len={}",
                        key,
                        index,
                        self.members.len()
                    ),
                });
            }
        }

        // Bind members (tx_id validation happens inside HolonReferenceWire::bind).
        let mut members = Vec::with_capacity(self.members.len());
        for member_wire in self.members {
            members.push(member_wire.bind(context)?);
        }

        Ok(HolonCollection::from_parts(self.state, members, self.keyed_index))
    }

    /// Summarizes this wire collection without context_binding.
    pub fn summarize(&self) -> String {
        format!(
            "HolonCollectionWire {{ state: {}, members: {}, keyed_index: {} }}",
            self.state,
            self.members.len(),
            self.keyed_index.len(),
        )
    }
}

impl From<&HolonCollection> for HolonCollectionWire {
    fn from(collection: &HolonCollection) -> Self {
        let members = collection.get_members().iter().map(HolonReferenceWire::from).collect();

        Self { state: collection.get_state(), members, keyed_index: collection.keyed_index() }
    }
}

impl From<HolonCollection> for HolonCollectionWire {
    fn from(collection: HolonCollection) -> Self {
        HolonCollectionWire::from(&collection)
    }
}
