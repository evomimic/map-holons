use std::collections::VecDeque;
use holons::holon::Holon;
use holons::holon_errors::HolonError;
use shared_types_holon::{HolonId, MapInteger, MapString, PropertyValue};

#[derive(Clone, Debug)]
pub struct DancesTestCase {
    pub name : String,
    pub description: String,
    pub steps : VecDeque<DanceTestStep>,
}

#[derive(Clone, Debug)]
pub enum DanceTestStep {
    EnsureDatabaseCount(MapInteger), // Ensures the expected number of holons exist in the DB
    // Create(Holon), // Associated data is expected Holon
    // Update(Holon), // Associated data is expected Holon after update
    // Delete(HolonId), // Associated data is id of Holon to delete
}

// A HolonsTestCase contains a sequence of test steps. The type of the HolonTestStep determines the test behavior
// EnsureEmpty -- Does a get_all_holons to confirm database is empty
// Create(Holon), Creates the specified holon, gets the created holon to confirm successful created, pushes the created
//    and pushes the created holon into a created_holons stack for subsequent get_all a test step.
// Update(Holon), // Associated data is expected Holon after update
// Delete(HolonId), // Associated data is id of Holon to delete
// `Create` test steps will trigger create and get tests

impl DancesTestCase {
    pub fn new(name: String, description: String)->Self {
        Self {
            name,
            description,
            steps: VecDeque::new(), }
    }

    pub fn add_ensure_database_count_step(&mut self, count: MapInteger) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::EnsureDatabaseCount(count));
        Ok(())
    }

    // pub fn add_create_step(&mut self, holon: Holon) -> Result<(), HolonError> {
    //     self.steps.push_back(DanceTestStep::Create(holon));
    //     Ok(())
    // }
    //
    // pub fn add_update_step(&mut self, holon: Holon) -> Result<(), HolonError> {
    //     self.steps.push_back(DanceTestStep::Update(holon));
    //     Ok(())
    // }
    //
    // pub fn add_delete_step(&mut self, holon_id: HolonId) -> Result<(), HolonError> {
    //     self.steps.push_back(DanceTestStep::Delete(holon_id));
    //     Ok(())
    // }
}

// #[derive(Clone, Debug)]
// pub enum HolonTestCase {
//     Creates(HolonCreatesTestCase),
//     Updates(HolonUpdatesTestCase),
// }
// #[derive(Clone, Debug)]
// pub struct HolonUpdatesTestCase { // this is equivalent to current HolonDescriptorTestCase
//     pub original: Holon,
//     pub updates: Vec<Holon>,
//     // pub message_level: Level,
// }
// #[derive(Clone, Debug)]
// pub struct HolonCreatesTestCase {
//     pub creates: Vec<Holon>,
//     // pub message_level: Level,
// }