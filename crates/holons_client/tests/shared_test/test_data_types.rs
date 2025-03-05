use derive_new::new;

use holons_core::{HolonReference, HolonsContextBehavior, StagedReference};

use holons_client::dances_client::dance_call_service::DanceCallService;
use holons_client::ConductorDanceCaller;
use holons_core::core_shared_objects::holon_pool::SerializableHolonPool;
use holons_core::core_shared_objects::{Holon, HolonError, HolonPool, RelationshipName};
use holons_core::dances::{ResponseStatusCode, SessionState};
use holons_core::query_layer::QueryExpression;
use shared_types_holon::{BaseValue, HolonId, MapInteger, MapString, PropertyMap, PropertyValue};
use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::fmt::{Debug, Display};
use std::sync::Arc;

pub const TEST_CLIENT_PREFIX: &str = "TEST CLIENT: ";

#[derive(new, Clone, Debug)]
pub struct TestHolonData {
    pub holon: Holon,
    pub holon_reference: HolonReference,
}

/// During the course of executing the steps in a test case:
///
/// - Staged Holons are accumulated in the Nursery (accessible from the context)
/// - Persisted Holons are conveyed from one step to another via the created_holons BTreeMap
///
/// This struct is **generic over any `DanceCaller` implementation**, allowing it to work
/// with different conductor backends (e.g., mock conductor, real conductor, or JS interop).
///
/// # Type Parameters
/// - `C`: A type implementing `DanceCaller`, used to execute dance calls.
#[derive(Debug)]
pub struct DanceTestExecutionState<C: ConductorDanceCaller> {
    context: Arc<dyn HolonsContextBehavior>,
    pub dance_call_service: Arc<DanceCallService<C>>,
    pub created_holons: BTreeMap<MapString, Holon>,
}

#[derive(Clone, Debug)]
pub enum TestReference {
    SavedHolon(MapString),
    StagedHolon(StagedReference),
}

#[derive(Clone, Debug)]
pub struct DancesTestCase {
    pub name: String,
    pub description: String,
    pub steps: VecDeque<DanceTestStep>,
}

#[derive(Clone, Debug)]
pub enum DanceTestStep {
    AbandonStagedChanges(StagedReference, ResponseStatusCode), // Marks a staged Holon as 'abandoned'
    AddRelatedHolons(
        StagedReference,
        RelationshipName,
        Vec<HolonReference>,
        ResponseStatusCode,
        Holon,
    ), // Adds relationship between two Holons
    Commit,                                                    // Attempts to commit
    DatabasePrint, // Writes log messages for each holon in the persistent store
    DeleteHolon(MapString, ResponseStatusCode), // Deletes the holon whose key is the MapString value
    EnsureDatabaseCount(MapInteger), // Ensures the expected number of holons exist in the DB
    // LoadCoreSchema,
    MatchSavedContent, // Ensures data committed to persistent store (DHT) matches expected
    QueryRelationships(MapString, QueryExpression, ResponseStatusCode),
    RemoveRelatedHolons(StagedReference, RelationshipName, Vec<HolonReference>, ResponseStatusCode),
    StageHolon(Holon), // Associated data is expected Holon, it could be an empty Holon (i.e., with no internal state)
    StageNewFromClone(TestReference, ResponseStatusCode),
    StageNewVersion(MapString, ResponseStatusCode),
    WithProperties(StagedReference, PropertyMap, ResponseStatusCode), // Update properties for Holon at StagedReference with PropertyMap
}

impl DanceTestStep {}

impl Display for DanceTestStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DanceTestStep::AbandonStagedChanges(staged_reference, expected_response) => {
                write!(
                    f,
                    "Marking Holon at ({:?}) as Abandoned, expecting ({:?})",
                    staged_reference, expected_response
                )
            }
            DanceTestStep::AddRelatedHolons(
                staged_reference,
                relationship_name,
                holons_to_add,
                expected_response,
                expected_holon,
            ) => {
                write!(f, "AddRelatedHolons to Holon at ({:#?}) for relationship: {:#?}, added_count: {:#?}, expecting: {:#?}, holon: {:?}", staged_reference, relationship_name, holons_to_add.len(), expected_response, expected_holon)
            }
            DanceTestStep::Commit => {
                write!(f, "Commit")
            }
            DanceTestStep::DatabasePrint => {
                write!(f, "DatabasePrint")
            }
            DanceTestStep::DeleteHolon(local_id, expected_response) => {
                write!(f, "DeleteHolon({:?}, expecting: {:?},)", local_id, expected_response)
            }
            DanceTestStep::EnsureDatabaseCount(count) => {
                write!(f, "EnsureDatabaseCount = {}", count.0)
            }
            // DanceTestStep::LoadCoreSchema => {
            //     write!(f, "LoadCoreSchema")
            // }
            DanceTestStep::MatchSavedContent => {
                write!(f, "MatchSavedContent")
            }
            DanceTestStep::QueryRelationships(
                node_collection,
                query_expression,
                expected_response,
            ) => {
                write!(f, "QueryRelationships for node_collection:{:#?}, with query expression: {:#?}, expecting {:#?}", node_collection, query_expression, expected_response)
            }
            DanceTestStep::RemoveRelatedHolons(
                staged_reference,
                relationship_name,
                holons_to_remove,
                expected_response,
            ) => {
                write!(f, "RemoveRelatedHolons to Holon at ({:#?}) for relationship: {:#?}, added_count: {:#?}, expecting: {:#?}", staged_reference, relationship_name, holons_to_remove.len(), expected_response)
            }
            DanceTestStep::StageHolon(holon) => {
                write!(f, "StageHolon({:#?})", holon)
            }
            DanceTestStep::StageNewVersion(original_holon_id, expected_response) => {
                write!(
                    f,
                    "NewVersion for original_holon_id: {:#?}, expecting response: {:#?}",
                    original_holon_id, expected_response
                )
            }
            DanceTestStep::StageNewFromClone(original_holon, expected_response) => {
                write!(
                    f,
                    "StageNewFromClone for original_holon: {:#?}, expecting response: {:#?}",
                    original_holon, expected_response
                )
            }
            DanceTestStep::WithProperties(staged_reference, properties, expected_response) => {
                write!(
                    f,
                    "WithProperties for Holon at ({:#?}) with properties: {:#?}, expecting {:#?} ",
                    staged_reference, properties, expected_response,
                )
            }
        }
    }
}

impl<C: ConductorDanceCaller> DanceTestExecutionState<C> {
    /// Creates a new `DanceTestExecutionState`.
    ///
    /// # Arguments
    /// - `test_context`: The test execution context.
    /// - `dance_call_service`: The `DanceCallService` instance for managing dance calls.
    ///
    /// # Returns
    /// A new `DanceTestExecutionState` instance.
    pub fn new(
        test_context: Arc<dyn HolonsContextBehavior>,
        dance_call_service: Arc<DanceCallService<C>>,
    ) -> Self {
        DanceTestExecutionState {
            context: test_context,
            dance_call_service,
            created_holons: BTreeMap::new(),
        }
    }
    pub fn context(&self) -> &dyn HolonsContextBehavior {
        &*self.context
    }
    pub fn get_created_holon_by_key(&self, key: &MapString) -> Option<Holon> {
        self.created_holons.get(key).cloned()
    }
}

impl DancesTestCase {
    pub fn new(name: String, description: String) -> Self {
        Self { name, description, steps: VecDeque::new() }
    }

    pub fn add_abandon_staged_changes_step(
        &mut self,
        staged_reference: StagedReference,
        expected_response: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps
            .push_back(DanceTestStep::AbandonStagedChanges(staged_reference, expected_response));
        Ok(())
    }

    pub fn add_commit_step(&mut self) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::Commit);
        Ok(())
    }

    // pub fn add_load_core_schema(&mut self) -> Result<(), HolonError> {
    //     self.steps.push_back(DanceTestStep::LoadCoreSchema);
    //     Ok(())
    // }

    pub fn add_database_print_step(&mut self) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::DatabasePrint);
        Ok(())
    }
    pub fn add_delete_holon_step(
        &mut self,
        holon_to_delete: MapString,
        expected_response: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::DeleteHolon(holon_to_delete, expected_response));
        Ok(())
    }
    pub fn add_ensure_database_count_step(&mut self, count: MapInteger) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::EnsureDatabaseCount(count));
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

    pub fn add_stage_new_from_clone_step(
        &mut self,
        original_holon: TestReference,
        expected_response: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::StageNewFromClone(original_holon, expected_response));
        Ok(())
    }

    pub fn add_stage_new_version_step(
        &mut self,
        original_holon_key: MapString,
        expected_response: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::StageNewVersion(original_holon_key, expected_response));
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

    pub fn add_related_holons_step(
        &mut self,
        staged_holon: StagedReference, // "owning" source Holon, which owns the Relationship
        relationship_name: RelationshipName,
        related_holons: Vec<HolonReference>, // "targets" referenced by HolonId for Saved and index for Staged
        expected_response: ResponseStatusCode,
        expected_holon: Holon,
    ) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::AddRelatedHolons(
            staged_holon,
            relationship_name,
            related_holons,
            expected_response,
            expected_holon,
        ));
        Ok(())
    }

    pub fn remove_related_holons_step(
        &mut self,
        staged_holon: StagedReference, // "owning" source Holon, which owns the Relationship
        relationship_name: RelationshipName,
        related_holons: Vec<HolonReference>, // "targets" referenced by HolonId for Saved and index for Staged
        expected_response: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::RemoveRelatedHolons(
            staged_holon,
            relationship_name,
            related_holons,
            expected_response,
        ));
        Ok(())
    }

    pub fn add_with_properties_step(
        &mut self,
        index: StagedReference,
        properties: PropertyMap,
        expected_response: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::WithProperties(index, properties, expected_response));
        Ok(())
    }
}
