use dances::dance_response::ResponseStatusCode;
use dances::holon_dance_adapter::{NodeCollection, QueryExpression};
use dances::staging_area::StagingArea;
use holons::commit_manager::StagedIndex;
use holons::holon::{Holon, HolonState};
use holons::holon_error::HolonError;
use holons::holon_reference::HolonReference;
use holons::relationship::RelationshipName;
use shared_types_holon::{HolonId, MapInteger, MapString, PropertyMap, PropertyValue};
use std::collections::VecDeque;
use std::fmt;

#[derive(Clone, Debug)]
pub struct DancesTestCase {
    pub name: String,
    pub description: String,
    pub steps: VecDeque<DanceTestStep>,
}

#[derive(Clone, Debug)]
pub enum DanceTestStep {
    AddRelatedHolons(
        StagedIndex,
        RelationshipName,
        Vec<HolonReference>,
        ResponseStatusCode,
        Holon,
    ), // Adds relationship between two Holons
    RemoveRelatedHolons(
         StagedIndex,
         RelationshipName, 
         Vec<HolonReference>, 
         ResponseStatusCode,
         Holon
    ), 
    DatabasePrint, // Writes log messages for each holon in the persistent store
    EnsureDatabaseCount(MapInteger), // Ensures the expected number of holons exist in the DB
    StageHolon(Holon), // Associated data is expected Holon, it could be an empty Holon (i.e., with no internal state)
    Commit,            // Attempts to commit
    WithProperties(StagedIndex, PropertyMap, ResponseStatusCode), // Update properties for Holon at StagedIndex with PropertyMap
    MatchSavedContent, // Ensures data committed to persistent store (DHT) matches expected
    AbandonStagedChanges(StagedIndex, ResponseStatusCode), // Marks a staged Holon as 'abandoned'
    LoadCoreSchema,
    QueryRelationships(MapString, QueryExpression, ResponseStatusCode),
}

impl fmt::Display for DanceTestStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DanceTestStep::AddRelatedHolons(
                index,
                relationship_name,
                holons_to_add,
                expected_response,
                expected_holon,
            ) => {
                write!(f, "AddRelatedHolons to Holon at ({:#?}) for relationship: {:#?}, added_count: {:#?}, expecting: {:#?}, holon: {:?}", index, relationship_name, holons_to_add.len(), expected_response, expected_holon)
            }
            DanceTestStep::RemoveRelatedHolons(
                index,
                relationship_name,
                holons_to_remove,
                expected_response,
                expected_holon,
            ) => {
                write!(f, "RemoveRelatedHolons to Holon at ({:#?}) for relationship: {:#?}, added_count: {:#?}, expecting: {:#?}, holon: {:?}", index, relationship_name, holons_to_remove.len(), expected_response, expected_holon)
            }
            DanceTestStep::DatabasePrint => {
                write!(f, "DatabasePrint")
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
            DanceTestStep::WithProperties(index, properties, expected_response) => {
                write!(
                    f,
                    "WithProperties for Holon at ({:#?}) with properties: {:#?}, expecting {:#?} ",
                    index, properties, expected_response,
                )
            }
            DanceTestStep::MatchSavedContent => {
                write!(f, "MatchSavedContent")
            }

            DanceTestStep::AbandonStagedChanges(index, expected_response) => {
                write!(
                    f,
                    "Marking Holon at ({:#?}) as Abandoned, expecting ({:#?})",
                    index, expected_response
                )
            }
            DanceTestStep::LoadCoreSchema => {
                write!(f, "LoadCoreSchema")
            }
            DanceTestStep::QueryRelationships(
                node_collection,
                query_expression,
                expected_response,
            ) => {
                write!(f, "QueryRelationships for node_collection:{:#?}, with query expression: {:#?}, expecting {:#?}", node_collection, query_expression, expected_response)
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
        related_holons: Vec<HolonReference>, // "targets" referenced by HolonId for Saved and index for Staged
        expected_response: ResponseStatusCode,
        expected_holon: Holon,
    ) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::AddRelatedHolons(
            source_index,
            relationship_name,
            related_holons,
            expected_response,
            expected_holon,
        ));
        Ok(())
    }

    pub fn remove_related_holons_step(
        &mut self,
        source_index: StagedIndex, // "owning" source Holon, which owns the Relationship
        relationship_name: RelationshipName,
        related_holons: Vec<HolonReference>, // "targets" referenced by HolonId for Saved and index for Staged
        expected_response: ResponseStatusCode,
        expected_holon: Holon,
    ) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::RemoveRelatedHolons(
            source_index,
            relationship_name,
            related_holons,
            expected_response,
            expected_holon,
        ));
        Ok(())
    }
    pub fn add_database_print_step(&mut self) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::DatabasePrint);
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
        expected_response: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::WithProperties(
            index,
            properties,
            expected_response,
        ));
        Ok(())
    }
    pub fn add_abandon_staged_changes_step(
        &mut self,
        index: StagedIndex,
        expected_response: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::AbandonStagedChanges(
            index,
            expected_response,
        ));
        Ok(())
    }
    pub fn add_load_core_schema(&mut self) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::LoadCoreSchema);
        Ok(())
    }

    pub fn add_query_relationships_step(
        &mut self,
        source_key: MapString,
        query_expression: QueryExpression,
        expected_response: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::QueryRelationships(
            source_key,
            query_expression,
            expected_response,
        ));
        Ok(())
    }
}
