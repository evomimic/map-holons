use crate::context_binding::holon_collection_wire::HolonCollectionWire;
use crate::context_binding::query_wire::NodeCollectionWire;
use crate::context_binding::HolonWire;
use crate::HolonReferenceWire;
use base_types::MapString;
use core_types::{HolonError, RelationshipName};
use holons_core::core_shared_objects::relationship_fetch::{
    FetchedRelationship, RelationshipPolicyBinding,
};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::dances::{DanceResponse, ResponseBody, ResponseStatusCode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// IPC-safe wire-form dance response.
///
/// This is the context-free shape that may be decoded at IPC boundaries.
/// Convert to runtime via `bind(context)`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DanceResponseWire {
    pub status_code: ResponseStatusCode,
    pub description: MapString,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<HolonError>,
    pub body: ResponseBodyWire,
    pub descriptor: Option<HolonReferenceWire>,
}

/// IPC-safe wire-form response body.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ResponseBodyWire {
    SavedHolonHydration(Vec<super::saved_holon_fetch_wire::SavedHolonFetchWire>),
    SavedRelationshipSet(
        Vec<(RelationshipName, HolonCollectionWire, Option<RelationshipPolicyBinding>)>,
    ),
    SavedRelationship {
        collection: HolonCollectionWire,
        descriptor: Option<RelationshipPolicyBinding>,
    },
    None,
    Holon(HolonWire),
    HolonCollection(HolonCollectionWire),
    Holons(Vec<HolonWire>),
    HolonReference(HolonReferenceWire),
    NodeCollection(NodeCollectionWire),
}

impl DanceResponseWire {
    /// Binds a wire response to the supplied transaction, validating `tx_id` for all embedded references.
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<DanceResponse, HolonError> {
        Ok(DanceResponse {
            status_code: self.status_code,
            description: self.description,
            error: self.error,
            body: self.body.bind(context)?,
            descriptor: match self.descriptor {
                None => None,
                Some(reference_wire) => Some(reference_wire.bind(context)?),
            },
        })
    }

    /// Summarizes the IPC-safe wire response for logging purposes.
    ///
    /// Safe to call immediately after IPC decode (before context_binding).
    pub fn summarize(&self) -> String {
        format!(
            "DanceResponseWire {{ \n  status_code: {}, \n  description: {:?}, \n  descriptor: {:?}, \n  body: {} }}",
            self.status_code,
            self.description,
            self.descriptor,
            self.body.summarize(),
        )
    }
}

impl ResponseBodyWire {
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<ResponseBody, HolonError> {
        match self {
            ResponseBodyWire::SavedHolonHydration(entries) => {
                Ok(ResponseBody::SavedHolonHydration(
                    entries.into_iter().map(|entry| entry.bind()).collect(),
                ))
            }
            ResponseBodyWire::SavedRelationship { collection, descriptor } => {
                Ok(ResponseBody::SavedRelationship(FetchedRelationship {
                    collection: collection.bind(context)?,
                    descriptor,
                }))
            }
            ResponseBodyWire::SavedRelationshipSet(entries) => {
                let fetched = entries
                    .into_iter()
                    .map(|(name, collection, descriptor)| {
                        Ok((
                            name,
                            FetchedRelationship {
                                collection: collection.bind(context)?,
                                descriptor,
                            },
                        ))
                    })
                    .collect::<Result<_, HolonError>>()?;
                Ok(ResponseBody::SavedRelationshipSet(fetched))
            }
            ResponseBodyWire::None => Ok(ResponseBody::None),
            ResponseBodyWire::Holon(holon_wire) => {
                Ok(ResponseBody::Holon(holon_wire.bind(context)?))
            }
            ResponseBodyWire::HolonCollection(collection_wire) => {
                Ok(ResponseBody::HolonCollection(collection_wire.bind(context)?))
            }
            ResponseBodyWire::Holons(holons_wire) => {
                let mut holons = Vec::with_capacity(holons_wire.len());
                for holon_wire in holons_wire {
                    holons.push(holon_wire.bind(context)?);
                }
                Ok(ResponseBody::Holons(holons))
            }
            ResponseBodyWire::HolonReference(reference_wire) => {
                Ok(ResponseBody::HolonReference(reference_wire.bind(context)?))
            }
            ResponseBodyWire::NodeCollection(node_collection_wire) => {
                Ok(ResponseBody::NodeCollection(node_collection_wire.bind(context)?))
            }
        }
    }

    /// Summarizes the wire response body for logging purposes.
    pub fn summarize(&self) -> String {
        match self {
            ResponseBodyWire::SavedHolonHydration(entries) => {
                format!("SavedHolonHydration(count={})", entries.len())
            }
            ResponseBodyWire::SavedRelationship { collection, descriptor } => {
                format!("SavedRelationship({}, descriptor={descriptor:?})", collection.summarize())
            }
            ResponseBodyWire::SavedRelationshipSet(entries) => {
                format!("SavedRelationshipSet(count={})", entries.len())
            }
            ResponseBodyWire::None => "None".to_string(),

            ResponseBodyWire::Holon(holon_wire) => {
                format!("HolonWire: {:#?}", holon_wire)
            }

            ResponseBodyWire::HolonCollection(collection_wire) => {
                format!("HolonCollectionWire ({} holons)", collection_wire.summarize())
            }

            ResponseBodyWire::Holons(holons_wire) => {
                format!("HolonsWire ({} holons)", holons_wire.len())
            }

            ResponseBodyWire::HolonReference(reference_wire) => {
                format!("HolonReferenceWire: {:?}", reference_wire)
            }

            ResponseBodyWire::NodeCollection(node_collection_wire) => {
                format!("NodeCollectionWire: {:#?}", node_collection_wire)
            }
        }
    }
}

impl From<&DanceResponse> for DanceResponseWire {
    fn from(response: &DanceResponse) -> Self {
        Self {
            status_code: response.status_code.clone(),
            description: response.description.clone(),
            error: response.error.clone(),
            body: ResponseBodyWire::from(&response.body),
            descriptor: response.descriptor.as_ref().map(HolonReferenceWire::from),
        }
    }
}

impl From<&ResponseBody> for ResponseBodyWire {
    fn from(body: &ResponseBody) -> Self {
        match body {
            ResponseBody::SavedHolonHydration(entries) => ResponseBodyWire::SavedHolonHydration(
                entries
                    .iter()
                    .map(super::saved_holon_fetch_wire::SavedHolonFetchWire::from)
                    .collect(),
            ),
            ResponseBody::SavedRelationship(fetched) => ResponseBodyWire::SavedRelationship {
                collection: HolonCollectionWire::from(&fetched.collection),
                descriptor: fetched.descriptor.clone(),
            },
            ResponseBody::SavedRelationshipSet(entries) => ResponseBodyWire::SavedRelationshipSet(
                entries
                    .iter()
                    .map(|(name, fetched)| {
                        (
                            name.clone(),
                            HolonCollectionWire::from(&fetched.collection),
                            fetched.descriptor.clone(),
                        )
                    })
                    .collect(),
            ),
            ResponseBody::None => ResponseBodyWire::None,
            ResponseBody::Holon(holon) => ResponseBodyWire::Holon(HolonWire::from(holon)),
            ResponseBody::HolonCollection(collection) => {
                ResponseBodyWire::HolonCollection(HolonCollectionWire::from(collection))
            }
            ResponseBody::Holons(holons) => {
                ResponseBodyWire::Holons(holons.iter().map(HolonWire::from).collect())
            }
            ResponseBody::HolonReference(reference) => {
                ResponseBodyWire::HolonReference(HolonReferenceWire::from(reference))
            }
            ResponseBody::NodeCollection(node_collection) => {
                ResponseBodyWire::NodeCollection(NodeCollectionWire::from(node_collection))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_types::{LineageIntegrityReason, LocalId};

    #[test]
    fn serializes_and_restores_structured_lineage_errors() {
        let error = HolonError::LineageIntegrityError {
            key: MapString("book-1".to_string()),
            lineage_root: LocalId(vec![1; 39]),
            reason: LineageIntegrityReason::CycleDetected,
        };
        let wire = DanceResponseWire {
            status_code: ResponseStatusCode::Conflict,
            description: MapString(error.to_string()),
            error: Some(error.clone()),
            body: ResponseBodyWire::None,
            descriptor: None,
        };

        let encoded = serde_json::to_string(&wire).expect("response wire must serialize");
        let decoded: DanceResponseWire =
            serde_json::from_str(&encoded).expect("response wire must deserialize");

        assert_eq!(decoded.error, Some(error));
    }
    #[test]
    fn relationship_set_wire_preserves_empty_sets_and_policy_bindings() {
        use core_types::{HolonId, RelationshipName};
        use holons_core::core_shared_objects::relationship_fetch::{
            FetchedRelationship, RelationshipPolicyBinding,
        };
        use holons_core::{HolonCollection, RelationshipCachePolicy};
        let binding = RelationshipPolicyBinding {
            descriptor_id: HolonId::Local(LocalId(vec![2; 39])),
            policy: RelationshipCachePolicy::MaxAgeMillis(100),
        };
        let runtime = ResponseBody::SavedRelationshipSet(vec![
            (
                RelationshipName("Empty".into()),
                FetchedRelationship {
                    collection: HolonCollection::new_existing(),
                    descriptor: Some(binding.clone()),
                },
            ),
            (
                RelationshipName("Foundational".into()),
                FetchedRelationship {
                    collection: HolonCollection::new_existing(),
                    descriptor: None,
                },
            ),
        ]);
        let projected = ResponseBodyWire::from(&runtime);
        let encoded = serde_json::to_string(&projected).unwrap();
        let decoded: ResponseBodyWire = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, projected);
        let ResponseBodyWire::SavedRelationshipSet(entries) = decoded else {
            panic!("expected membership set");
        };
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].2, Some(binding));
        assert!(entries[0].1.members.is_empty());
        assert!(entries[1].2.is_none());
    }
}
