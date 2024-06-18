use crate::smart_reference::SmartReference;
use crate::smartlink::{get_relationship_name_from_smartlink, SmartLinkHolder};
use crate::{holon_error::HolonError, staged_reference::StagedReference};
// use crate::smart_reference::SmartReference;
use crate::context::HolonsContext;
use crate::holon_reference::HolonReference;
use crate::smart_collection::SmartCollection;
use crate::staged_collection::StagedCollection;
use hdk::prelude::*;
use holons_integrity::LinkTypes;
use shared_types_holon::{HolonId, MapString};
use std::collections::BTreeMap;
use std::fmt;

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct RelationshipName(pub MapString);
impl fmt::Display for RelationshipName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub struct RelationshipMap(pub BTreeMap<RelationshipName, HolonCollection>);
impl RelationshipMap {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }
}

#[hdk_entry_helper]
#[derive(Clone, Eq, PartialEq)]
pub enum HolonCollection {
    Editable(StagedCollection), // Mutable collection -- extended during the staging process
    Existing(SmartCollection),  // constructed from stored links
}
impl HolonCollection {
    /// The method
    pub fn commit_relationship(
        &self,
        context: &HolonsContext,
        source_id: HolonId,
        name: RelationshipName,
    ) -> Result<(), HolonError> {
        match self {
            Self::Editable(collection) => {
                collection.add_smartlinks_for_collection(
                    context,
                    source_id.clone(),
                    name.clone(),
                )?;
            }
            Self::Existing(_) => {}
        }

        Ok(())
    }

    /// Creates an editable_collection within the HolonCollection from the SmartReferences in the existing_collection
    pub fn stage_collection(
        &mut self,
        source_holon: StagedReference,
        existing_collection: SmartCollection,
    ) {
        // convert Vec<SmartReference> to Vec<HolonReference>
        let holons = existing_collection
            .holons
            .into_iter()
            .map(|smart_ref| HolonReference::Smart(smart_ref))
            .collect();

        let staged_collection = StagedCollection {
            source_holon: Some(source_holon),
            relationship_name: existing_collection.relationship_name,
            holons,
            // keyed_index: existing_collection.keyed_index,
        };
        self.editable = Some(staged_collection);
    }
}

/// Builds a relationship map by doing a get_relationship_links (which under the hood does a get_links for all relationships) on the (owning) source_holon
pub fn build_relationship_map_from_smartlinks(
    holon_id: ActionHash,
) -> Result<RelationshipMap, HolonError> {
    let mut map = BTreeMap::new();
    let links = get_relationship_links(holon_id.clone())?;
    // a convenience holder to make inserts more efficient
    let mut holder_vec: Vec<SmartLinkHolder> = Vec::new();

    let mut names: Vec<String> = Vec::new();

    for link in links {
        let name = get_relationship_name_from_smartlink(link.clone())?.0 .0;
        if !names.iter().any(|n| n == &name) {
            names.push(name.clone());
        }

        let target = link.target.into_action_hash().ok_or_else(|| {
            HolonError::HashConversion("Link target".to_string(), "ActionHash".to_string())
        })?;
        let reference = SmartReference {
            holon_id: HolonId(target),
            smart_property_values: None, // defaulting to None until descriptors ready
        };

        let holder = SmartLinkHolder { name, reference };
        holder_vec.push(holder)
    }

    for name in names {
        let mut holons = Vec::new();

        holder_vec
            .clone()
            .into_iter()
            .filter(|h| h.name == name)
            .map(|h| holons.push(h.reference));

        let holon_reference = HolonReference::Smart(SmartReference {
            holon_id: HolonId(holon_id),
            smart_property_values: None, // defaulting to None until descriptors ready
        });

        let collection = SmartCollection {
            source_holon: Some(holon_reference),
            relationship_name: Some(RelationshipName(MapString(name.clone()))),
            holons,
        };

        map.insert(
            RelationshipName(MapString(name.clone())),
            HolonCollection::Existing(collection),
        );
    }
    debug!("built relationship_map from smartlinks: {:#?}", map.clone());

    Ok(RelationshipMap(map))
}

pub fn get_relationship_links(holon_id: ActionHash) -> Result<Vec<Link>, HolonError> {
    let links = get_links(holon_id, LinkTypes::SmartLink, None).map_err(|e| HolonError::from(e))?;

    Ok(links)
}

// impl Clone for HolonCollection {
//     /// Custom clone implementation, does not clone its cursors or editable vector
//     fn clone(&self) -> Self {
//         Self {
//             editable: None,
//             cursors: None,
//         }
//     }
// }

// pub fn query_relationship(
//     source_holon: HolonReference,
//     relationship_name: RelationshipName,
//     // query_spec: QuerySpec
// )
//     ->SmartCollection {
//     todo!()
// }
