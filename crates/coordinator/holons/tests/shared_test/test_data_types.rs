#![allow(dead_code)]

use holons::holon::Holon;
use shared_types_holon::{HolonId, MapString, PropertyValue};

// A HolonsTestCase contains a sequence of test steps. The type of the HolonTestStep determines the test behavior
// EnsureEmpty -- Does a get_all_holons to confirm database is empty
// Create(Holon), Creates the specified holon, gets the created holon to confirm successful created, pushes the created
//    and pushes the created holon into a created_holons stack for subsequent get_all a test step.
// Update(Holon), // Associated data is expected Holon after update
// Delete(HolonId), // Associated data is id of Holon to delete
// `Create` test steps will trigger create and get tests
#[derive(Clone, Debug)]
pub struct HolonsTestCase {
    pub steps: Vec<HolonTestStep>,
}
#[derive(Clone, Debug)]
pub enum HolonTestStep {
    EnsureEmpty(),   // Does a get_all_holons to confirm database is empty
    Create(Holon),   // Associated data is expected Holon
    Update(Holon),   // Associated data is expected Holon after update
    Delete(HolonId), // Associated data is id of Holon to delete
}
#[derive(Clone, Debug)]
pub enum HolonTestCase {
    Creates(HolonCreatesTestCase),
    Updates(HolonUpdatesTestCase),
}
#[derive(Clone, Debug)]
pub struct HolonUpdatesTestCase {
    // this is equivalent to current HolonDescriptorTestCase
    pub original: Holon,
    pub updates: Vec<Holon>,
    // pub message_level: Level,
}
#[derive(Clone, Debug)]
pub struct HolonCreatesTestCase {
    pub creates: Vec<Holon>,
    // pub message_level: Level,
}
