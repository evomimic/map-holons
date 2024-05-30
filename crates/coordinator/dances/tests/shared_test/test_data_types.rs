use dances::dance_request::PortableReference;
use dances::staging_area::StagingArea;
use holons::commit_manager::StagedIndex;
use holons::holon::{Holon, HolonState};
use holons::holon_error::HolonError;
use holons::relationship::RelationshipName;
use shared_types_holon::{HolonId, MapInteger, MapString, PropertyMap, PropertyValue};
use std::collections::VecDeque;
use std::fmt;

#[derive(Clone, Debug)]
pub struct DancesTestCase {
    pub name: String,
    pub description: String,
    pub steps: VecDeque<DanceTestStep>,
    // pub holons: Vec<Holon>,
}

#[derive(Clone, Debug)]
pub enum DanceTestStep {
    AddRelatedHolons(StagedIndex, RelationshipName, Vec<PortableReference>), // Adds relationship between two Holons
    EnsureDatabaseCount(MapInteger), // Ensures the expected number of holons exist in the DB
    StageHolon(Holon), // Associated data is expected Holon, it could be an empty Holon (i.e., with no internal state)
    Commit,            // Attempts to commit
    WithProperties(StagedIndex, PropertyMap), // Update properties for Holon at StagedIndex with PropertyMap
    MatchSavedContent, // Ensures data committed to persistent store (DHT) matches expected
    AbandonStagedChanges(StagedIndex), // Marks a staged Holon as 'abandoned'
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
            DanceTestStep::Commit => {
                write!(f, "Commit")
            }
            DanceTestStep::WithProperties(index, properties) => {
                write!(
                    f,
                    "WithProperties for Holon at ({:#?}) with properties: {:#?} ",
                    index, properties
                )
            }
            DanceTestStep::MatchSavedContent => {
                write!(f, "MatchSavedContent")
            }
            DanceTestStep::AbandonStagedChanges(index) => {
                write!(f, "Marking Holon at ({:#?}) as Abandoned", index)
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
    pub fn new(name: String, description: String) -> Self {
        Self {
            name,
            description,
            steps: VecDeque::new(),
        }
    }

    pub fn add_related_holons_step(
        &mut self,
        source_index: StagedIndex, // "owning" source Holon, which owns the Relationship
        relationship_name: RelationshipName,
        related_holons: Vec<PortableReference>, // "targets" referenced by HolonId for Saved and index for Staged
    ) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::AddRelatedHolons(
            source_index,
            relationship_name,
            related_holons,
        ));
        Ok(())
    }
    pub fn add_ensure_database_count_step(&mut self, count: MapInteger) -> Result<(), HolonError> {
        self.steps
            .push_back(DanceTestStep::EnsureDatabaseCount(count));
        Ok(())
    }
    pub fn add_match_saved_content_step(&mut self) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::MatchSavedContent);
        Ok(())
    }

    pub fn add_stage_holon_step(&mut self, holon: Holon) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::StageHolon(holon));
        Ok(())
    }
    pub fn add_commit_step(&mut self) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::Commit);
        Ok(())
    }
    pub fn add_with_properties_step(
        &mut self,
        index: StagedIndex,
        properties: PropertyMap,
    ) -> Result<(), HolonError> {
        self.steps
            .push_back(DanceTestStep::WithProperties(index, properties));
        Ok(())
    }
    pub fn add_abandon_staged_changes_step(
        &mut self,
        index: StagedIndex,
    ) -> Result<(), HolonError> {
        self.steps
            .push_back(DanceTestStep::AbandonStagedChanges(index));
        Ok(())
    }
}
