use std::collections::VecDeque;
use std::fmt;
use dances::dance_request::PortableReference;
use holons::holon::{Holon, HolonState};
use holons::holon_error::HolonError;
use shared_types_holon::{HolonId, MapInteger, MapString, PropertyMap, PropertyValue};
use dances::staging_area::StagingArea;
use holons::commit_manager::StagedIndex;
use holons::relationship::RelationshipName;

#[derive(Clone, Debug)]
pub struct DancesTestCase {
    pub name : String,
    pub description: String,
    pub steps : VecDeque<DanceTestStep>,
}

#[derive(Clone, Debug)]
pub enum DanceTestStep {
    AddRelatedHolons(StagedIndex, RelationshipName, Vec<PortableReference>),
    EnsureDatabaseCount(MapInteger), // Ensures the expected number of holons exist in the DB
    StageHolon(Holon), // Associated data is expected Holon, it could be an empty Holon (i.e., with no internal state)
    Commit(),
    WithProperties(StagedIndex,PropertyMap) // Update properties for Holon at StagedIndex with PropertyMap
    // Update(Holon), // Associated data is expected Holon after update
    // Delete(HolonId), // Associated data is id of Holon to delete
}

impl fmt::Display for DanceTestStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DanceTestStep::AddRelatedHolons(index, relationship_name, holons_to_add) => {
                write!(f, "AddRelatedHolons to Holon at ({:#?}) for relationship: {:#?}, added_count: {:#?}", index, relationship_name, holons_to_add.len() )
            }
            DanceTestStep::EnsureDatabaseCount(count) => {
                write!(f, "EnsureDatabaseCount = {}", count.0)
            }
            DanceTestStep::StageHolon(holon) => {
                write!(f, "StageHolon({:#?})", holon)
            }
            DanceTestStep::Commit() => {
                write!(f, "Commit")
            }
            DanceTestStep::WithProperties(index,properties) => {
                write!(f, "WithProperties for Holon at ({:#?}) with properties: {:#?} ", index, properties)
            }
        }
    }
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

    pub fn add_related_holons_step(
        &mut self,
        source_index: StagedIndex,
        relationship_name: RelationshipName,
        related_holons:Vec<PortableReference>
    ) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::AddRelatedHolons(source_index, relationship_name, related_holons));
        Ok(())
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

    pub fn add_with_properties_step(&mut self, index:StagedIndex, properties:PropertyMap) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::WithProperties(index, properties));
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