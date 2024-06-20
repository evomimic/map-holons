use crate::context::HolonsContext;
use crate::holon_collection::HolonCollection;
use crate::holon_error::HolonError;
use crate::smart_link_manager::create_link_tag;
use crate::smart_reference::SmartReference;
use crate::smartlink::get_relationship_name_from_smartlink;
// use crate::smart_reference::SmartReference;
use crate::holon_reference::HolonReference;
use crate::smart_collection::SmartCollection;
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

/// Builds a relationship map by doing a get_relationship_links (which under the hood does a get_links for all relationships) on the (owning) source_holon

pub fn build_relationship_map_from_smartlinks(
    context: &HolonsContext,
    holon_id: ActionHash,
    relationship_name: Option<RelationshipName>,
) -> Result<RelationshipMap, HolonError> {
    let mut reference_map: BTreeMap<RelationshipName, Vec<HolonReference>> = BTreeMap::new();
    let links = get_relationship_links(holon_id.clone(), None)?;

    for link in links {
        let name_string = get_relationship_name_from_smartlink(link.clone())?.0 .0; // TODO: change this to decode the whole smartlink in SmartLinkHolder
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

        if let Some(references) = reference_map.get(&name) {
            let mut vec = references.clone();
            vec.push(reference);
            reference_map.insert(name, vec);
        } else {
            let mut references: Vec<HolonReference> = Vec::new();
            reference_map.insert(name, references);
        }
    }

    let mut relationship_map: BTreeMap<RelationshipName, HolonCollection> = BTreeMap::new();

    for (map_name, holons) in reference_map {
        let mut collection = HolonCollection::new_existing();
        collection.add_references(context, holons)?;
        relationship_map.insert(map_name, collection);
    }

    Ok(RelationshipMap(relationship_map))
}

// pub fn build_relationship_map_from_smartlinks(
//     holon_id: ActionHash,
// ) -> Result<RelationshipMap, HolonError> {
//     let mut map = BTreeMap::new();
//     let links = get_relationship_links(holon_id.clone())?;
//     // a convenience holder to make inserts more efficient
//     let mut holder_vec: Vec<SmartLinkHolder> = Vec::new();

//     let mut names: Vec<String> = Vec::new();

//     for link in links {
//         let name = get_relationship_name_from_smartlink(link.clone())?.0 .0;
//         if !names.iter().any(|n| n == &name) {
//             names.push(name.clone());
//         }

//         let target = link.target.into_action_hash().ok_or_else(|| {
//             HolonError::HashConversion("Link target".to_string(), "ActionHash".to_string())
//         })?;
//         let reference = SmartReference {
//             holon_id: HolonId(target),
//             smart_property_values: None, // defaulting to None until descriptors ready
//         };

//         let holder = SmartLinkHolder { name, reference };
//         holder_vec.push(holder)
//     }

//     for name in names {
//         let mut holons = Vec::new();

//         holder_vec
//             .clone()
//             .into_iter()
//             .filter(|h| h.name == name)
//             .map(|h| holons.push(h.reference));

//         let holon_reference = HolonReference::Smart(SmartReference {
//             holon_id: HolonId(holon_id),
//             smart_property_values: None, // defaulting to None until descriptors ready
//         });

//         let collection = HolonCollection {
//             source_holon: Some(holon_reference),
//             relationship_name: Some(RelationshipName(MapString(name.clone()))),
//             holons,
//         };

//         map.insert(RelationshipName(MapString(name.clone())), collection);
//     }
//     debug!("built relationship_map from smartlinks: {:#?}", map.clone());

//     Ok(RelationshipMap(map))
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
