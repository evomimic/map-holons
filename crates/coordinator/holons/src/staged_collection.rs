use crate::holon_errors::HolonError;
use crate::holon_reference::HolonReference;
use crate::staged_reference::StagedReference;
use hdk::prelude::*;
use shared_types_holon::{HolonId, MapString};
use std::collections::BTreeMap;

///
/// StagedCollections are editable collections of holons representing the target of a relationship
///
/// Assumptions:
/// * Only Relationship Names that have populated target values will exist in source holon's relationship_map
/// * When *_new_* holons are created (from scratch), their relationship_map will be created but empty
/// * When holons are derived or cloned from existing holons
///     * their relationship_map will have entries for any populated in the existing holon
///     * the RelationshipTarget value for those entries will be have a StagedCollection created
///     * the StagedCollection will be populated with SmartReferences cloned from the exist holon
///
#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq)]
pub struct StagedCollection {
    pub source_holon: Option<StagedReference>,
    pub relationship_descriptor: Option<HolonReference>,
    pub holons: Vec<HolonReference>,
    pub keyed_index: BTreeMap<MapString, usize>,
    // TODO: validation_state: ValidationState,
}

// // pub struct StagedCollection {
// //     pub source_holon:  Option<Weak<RefCell<Holon>>>,
// //     pub relationship_descriptor: Option<HolonReference>,
// //     pub holons: Vec<HolonReference>,
// //     pub keyed_index: BTreeMap<MapString, usize>, // Allows lookup by key to staged holons for which keys are defined
// //     // TODO: validation_state: ValidationState,
// //
// // }
// impl PartialEq for StagedCollection {
//     fn eq(&self, other: &Self) -> bool {
//         match (&self.source_holon, &other.source_holon) {
//             (Some(self_weak), Some(other_weak)) => {
//                 let self_rc = self_weak.upgrade().unwrap();
//                 let other_rc = other_weak.upgrade().unwrap();
//                 Rc::ptr_eq(&self_rc, &other_rc)
//             }
//             (None, None) => true,
//             _ => false,
//         }
//     }
// }
//
// impl Eq for StagedCollection {}

// Methods
// add_related_holons(target_holons: Vec<HolonReference>)
// remove_holons(holons: Vec<HolonReference>)
// get_holon_by_key(key: MapString)->HolonReference
// commit(context: HolonsContext)
impl StagedCollection {
    pub fn new() -> Self {
        StagedCollection {
            source_holon: None,
            relationship_descriptor: None,
            holons: Vec::new(),
            keyed_index: BTreeMap::new(),
        }
    }

    // pub fn commit(&mut self, source_id: HolonId) -> Result<(), HolonError> {
    //     for holon_reference in self.holons.clone() {
    //         holon_reference.commit(source_id.clone())?;
    //     }
    //     self.holons = Vec::new(); // is it correct to clear holons from staged collection? otherwise commit doesn't need to take mutable reference to self
    //     Ok(())
    // }
    pub fn commit(&self, source_id: HolonId) -> Result<(), HolonError> {
        for holon_reference in self.holons.clone() {
            holon_reference.commit(source_id.clone())?;
        }
        Ok(())
    }

    // pub fn remove_holons(&mut self, holons: Vec<HolonReference>) {
    //     todo!()
    // }
    // pub fn get_holon_by_key(&mut self, key: MapString)->HolonReference {
    //     todo!()
    // }
}
