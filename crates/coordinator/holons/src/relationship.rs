use crate::context::HolonsContext;
use crate::holon_collection::HolonCollection;
use crate::holon_error::HolonError;
use crate::smart_link_manager::create_link_tag;
use crate::smart_reference::SmartReference;
use crate::smartlink::get_relationship_name_from_smartlink;
// use crate::smart_reference::SmartReference;
use crate::holon_reference::HolonReference;
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

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SmartLinkHolder {
    pub name: RelationshipName,
    pub reference: HolonReference,
}

/// Builds a full or partial RelationshipMap for an existing holon identified by `source_holon_id`
/// by retrieving SmartLinks for that holon.
/// If `relationship_name` is supplied, the RelationshipMap returned will only have (at most) a
/// single entry consisting of the HolonCollection for the supplied `relationship_name`.
/// Otherwise, a full RelationshipMap will be populated for the `source_holon_id`.
///
///
///
pub fn build_relationship_map_from_smartlinks(
    context: &HolonsContext,
    source_holon_id: ActionHash,
    relationship_name: Option<RelationshipName>,
) -> Result<RelationshipMap, HolonError> {
    let mut reference_map: BTreeMap<RelationshipName, Vec<HolonReference>> = BTreeMap::new();
    let links = get_relationship_links(source_holon_id.clone(), relationship_name)?;


    debug!("Retrieved {:?} links from holochain", links.len());

    for link in links {
        // TODO: Consolidated all logic in this loop into a call on a (new) `decode_to_smartlink()`
        // function that creates a SmartLink object from a holochain link.
        let name_string = get_relationship_name_from_smartlink(link.clone())?.0 .0;
        let name = RelationshipName(MapString(name_string));



        let target = link.target.into_action_hash().ok_or_else(|| {
            HolonError::HashConversion("Link target".to_string(), "ActionHash".to_string())
        })?;
        let reference = HolonReference::Smart(SmartReference {
            holon_id: HolonId(target),
            smart_property_values: None, // defaulting to None until descriptors ready
        });

        let holder = SmartLinkHolder {
            name: name.clone(),
            reference: reference.clone(),
        };

        // The following:
        // 1) adds an entry for relationship name if not already present (via `entry` API)
        // 2) adds a value (Vec<HolonReference>) for the entry, if not already present (`.or_insert_with`)
        // 3) pushes the new HolonReference into the vector -- without having to clone the vector

        reference_map.entry(name).or_insert_with(Vec::new).push(reference);
    }

    // Now create the result

    let mut relationship_map: BTreeMap<RelationshipName, HolonCollection> = BTreeMap::new();

    for (map_name, holons) in reference_map {
        let mut collection = HolonCollection::new_existing();
        collection.add_references(context, holons)?;
        relationship_map.insert(map_name, collection);
    }

    Ok(RelationshipMap(relationship_map))
}

// pub fn build_relationship_map_from_smartlinks(
//     context: &HolonsContext,
//     holon_id: ActionHash,
//     relationship_name: Option<RelationshipName>,
// ) -> Result<RelationshipMap, HolonError> {
//     let mut reference_map: BTreeMap<RelationshipName, Vec<HolonReference>> = BTreeMap::new();
//     let links = get_relationship_links(holon_id.clone(), relationship_name)?;
//
//     debug!("Retrieved {:?} links from holochain", links.len());
//
//     for link in links {
//         let name_string = get_relationship_name_from_smartlink(link.clone())?.0 .0; // TODO: change this to decode the whole smartlink in SmartLinkHolder
//         let name = RelationshipName(MapString(name_string));
//
//         let target = link.target.into_action_hash().ok_or_else(|| {
//             HolonError::HashConversion("Link target".to_string(), "ActionHash".to_string())
//         })?;
//         let reference = HolonReference::Smart(SmartReference {
//             holon_id: HolonId(target),
//             smart_property_values: None, // defaulting to None until descriptors ready
//         });
//
//         let holder = SmartLinkHolder {
//             name: name.clone(),
//             reference: reference.clone(),
//         };
//
//         if let Some(references) = reference_map.get(&name) {
//             let mut vec = references.clone();
//             vec.push(reference);
//             reference_map.insert(name, vec);
//         } else {
//             let mut references: Vec<HolonReference> = Vec::new();
//             reference_map.insert(name, references);
//         }
//     }
//
//     let mut relationship_map: BTreeMap<RelationshipName, HolonCollection> = BTreeMap::new();
//
//     for (map_name, holons) in reference_map {
//         let mut collection = HolonCollection::new_existing();
//         collection.add_references(context, holons)?;
//         relationship_map.insert(map_name, collection);
//     }
//
//     Ok(RelationshipMap(relationship_map))
// }



/// Gets relationships optionally filtered by name
pub fn get_relationship_links(
    holon_id: ActionHash,
    relationship_name: Option<RelationshipName>,
) -> Result<Vec<Link>, HolonError> {
    let link_tag: Option<LinkTag> = if let Some(name) = relationship_name {
        Some(create_link_tag(name))
    } else {
        None
    };

    let links =
        get_links(holon_id, LinkTypes::SmartLink, link_tag).map_err(|e| HolonError::from(e))?;

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
