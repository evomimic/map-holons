use std::collections::VecDeque;
use holons::holon::Holon;
use holons::holon_error::HolonError;
use shared_types_holon::{HolonId, MapInteger, MapString, PropertyValue};
use dances::staging_area::StagingArea;

#[derive(Clone, Debug)]
pub struct DancesTestCase {
    pub name : String,
    pub description: String,
    pub steps : VecDeque<DanceTestStep>,
}

#[derive(Clone, Debug)]
pub enum DanceTestStep {
    EnsureDatabaseCount(MapInteger), // Ensures the expected number of holons exist in the DB
    StageHolon(Holon), // Associated data is expected Holon, it could be empty
    Commit(),
    // Update(Holon), // Associated data is expected Holon after update
    // Delete(HolonId), // Associated data is id of Holon to delete
}

pub struct DanceTestState {
    pub staging_area: StagingArea,
    pub created_holons: Vec<Holon>,
}

impl DanceTestState {
    pub fn new() -> DanceTestState {
        DanceTestState {
            staging_area: StagingArea::new(),
            created_holons: Vec::new(),
        }
    }
}


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

    pub fn add_stage_holon_step(&mut self, holon: Holon) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::StageHolon(holon));
        Ok(())
    }
    pub fn add_commit_step(&mut self) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::Commit());
        Ok(())
    }
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