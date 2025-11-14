use derive_new::new;

use holons_core::core_shared_objects::holon_pool::SerializableHolonPool;
use holons_core::core_shared_objects::Holon;
use holons_prelude::prelude::*;
use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    fmt::{Debug, Display},
    sync::Arc,
};

use base_types::{MapInteger, MapString};
use core_types::{HolonError, HolonId};
use core_types::{PropertyMap, RelationshipName};

use holons_core::{
    core_shared_objects::{ReadableHolonState, TransientHolon},
    dances::ResponseStatusCode,
    query_layer::QueryExpression,
    reference_layer::{
        HolonReference, HolonsContextBehavior, ReadableHolon, StagedReference, TransientReference,
    },
};

pub const TEST_CLIENT_PREFIX: &str = "TEST CLIENT: ";

// These constants allow consistency between the helper function and its callers
pub const BOOK_KEY: &str =
    "Emerging World: The Evolution of Consciousness and the Future of Humanity";
pub const PERSON_1_KEY: &str = "Roger Briggs";
pub const PERSON_2_KEY: &str = "George Smith";
pub const PUBLISHER_KEY: &str = "Publishing Company";
pub const BOOK_DESCRIPTOR_KEY: &str = "Book.HolonType";
pub const PERSON_DESCRIPTOR_KEY: &str = "Person.HolonType";
pub const BOOK_TO_PERSON_RELATIONSHIP: &str = "AuthoredBy";
pub const BOOK_TO_PERSON_RELATIONSHIP_KEY: &str =
    "(Book.HolonType)-[AuthoredBy]->(Person.HolonType)";
pub const PERSON_TO_BOOK_REL_INVERSE: &str = "Authors";
pub const PERSON_TO_BOOK_RELATIONSHIP_INVERSE_KEY: &str =
    "(Person.HolonType)-[Authors]->(Book.HolonType)";
pub const EDITOR_FOR: &str = "EditorFor";

// #[derive(new, Clone, Debug)]
// pub struct TestHolonData {
//     pub holon: Holon,
//     pub holon_reference: HolonReference,
// }

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
pub struct DanceTestExecutionState {
    pub context: Arc<dyn HolonsContextBehavior>,
    pub created_holons: BTreeMap<MapString, Holon>,
}

#[derive(Clone, Debug)]
pub enum TestReference {
    SavedHolon(MapString),
    StagedHolon(StagedReference),
    TransientHolon(TransientReference),
}

#[derive(Clone, Debug)]
pub struct DancesTestCase {
    pub name: String,
    pub description: String,
    pub steps: VecDeque<DanceTestStep>,
    pub test_session_state: TestSessionState,
}

#[derive(Clone, Debug, Default)]
pub struct TestSessionState {
    transient_holons: SerializableHolonPool,
}

impl TestSessionState {
    pub fn set_transient_holons(&mut self, transient_holons: SerializableHolonPool) {
        self.transient_holons = transient_holons;
    }

    pub fn get_transient_holons(&self) -> &SerializableHolonPool {
        &self.transient_holons
    }
}

#[derive(Clone, Debug)]
pub enum DanceTestStep {
    AbandonStagedChanges(HolonReference, ResponseStatusCode), // Marks a staged Holon as 'abandoned'
    AddRelatedHolons(
        HolonReference,
        RelationshipName,
        Vec<TestReference>,
        ResponseStatusCode,
        HolonReference,
    ), // Adds relationship between two Holons
    Commit,                                                   // Attempts to commit
    DatabasePrint, // Writes log messages for each holon in the persistent store
    DeleteHolon(MapString, ResponseStatusCode), // Deletes the holon whose key is the MapString value
    EnsureDatabaseCount(MapInteger), // Ensures the expected number of holons exist in the DB
    LoadHolons {
        set: TransientReference,
        expect_staged: MapInteger,
        expect_committed: MapInteger,
        expect_links_created: MapInteger,
        expect_errors: MapInteger,
        expect_total_bundles: MapInteger,
        expect_total_loader_holons: MapInteger,
    },
    MatchSavedContent, // Ensures data committed to persistent store (DHT) matches expected
    QueryRelationships(MapString, QueryExpression, ResponseStatusCode),
    RemoveProperties(HolonReference, PropertyMap, ResponseStatusCode),
    RemoveRelatedHolons(HolonReference, RelationshipName, Vec<HolonReference>, ResponseStatusCode),
    StageHolon(TransientReference),
    StageNewFromClone(TestReference, MapString, ResponseStatusCode),
    StageNewVersion(MapString, ResponseStatusCode),
    WithProperties(HolonReference, PropertyMap, ResponseStatusCode), // Update properties for Holon at StagedReference with PropertyMap
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
                holon_reference,
                relationship_name,
                holons_to_add,
                expected_response,
                expected_holon,
            ) => {
                write!(f, "AddRelatedHolons to Holon {:#?} for relationship: {:#?}, added_count: {:#?}, expecting: {:#?}, holon: {:?}", holon_reference, relationship_name, holons_to_add.len(), expected_response, expected_holon)
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
            DanceTestStep::LoadHolons {
                set: _,
                expect_staged,
                expect_committed,
                expect_links_created,
                expect_errors,
                expect_total_bundles,
                expect_total_loader_holons,
            } => {
                write!(
                    f,
                    "LoadHolons(staged={}, committed={}, links_created={}, errors={}, bundles={}, loader_holons={})",
                    expect_staged.0, expect_committed.0, expect_links_created.0, expect_errors.0, expect_total_bundles.0, expect_total_loader_holons.0
                )
            }
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
            DanceTestStep::RemoveProperties(holon_reference, properties, expected_response) => {
                write!(
                    f,
                    "RemoveProperties {:#?} for Holon {:#?}, expecting {:#?} ",
                    properties, holon_reference, expected_response,
                )
            }
            DanceTestStep::RemoveRelatedHolons(
                holon_reference,
                relationship_name,
                holons_to_remove,
                expected_response,
            ) => {
                write!(f, "RemoveRelatedHolons from Holon {:#?} for relationship: {:#?}, added_count: {:#?}, expecting: {:#?}", holon_reference, relationship_name, holons_to_remove.len(), expected_response)
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
            DanceTestStep::StageNewFromClone(original_holon, new_key, expected_response) => {
                write!(
                    f,
                    "StageNewFromClone for original_holon: {:#?}, with new key: {:?}, expecting response: {:#?}",
                    original_holon, new_key, expected_response
                )
            }
            DanceTestStep::WithProperties(holon_reference, properties, expected_response) => {
                write!(
                    f,
                    "WithProperties for Holon {:#?} with properties: {:#?}, expecting {:#?} ",
                    holon_reference, properties, expected_response,
                )
            }
        }
    }
}

impl DanceTestExecutionState {
    /// Creates a new `DanceTestExecutionState`.
    ///
    /// # Arguments
    /// - `test_context`: The test execution context.
    /// - `dance_call_service`: The `DanceCallService` instance for managing dance calls.
    ///
    /// # Returns
    /// A new `DanceTestExecutionState` instance.
    pub fn new(test_context: Arc<dyn HolonsContextBehavior>) -> Self {
        DanceTestExecutionState { context: test_context, created_holons: BTreeMap::new() }
    }
    pub fn context(&self) -> &dyn HolonsContextBehavior {
        &*self.context
    }

    /// Converts a vector of [`HolonReference`]s into a vector of [`TestReference`]s.
    ///
    /// For `HolonReference::Smart` entries, this method calls `key` on the `SmartReference`.
    /// If `key` fails or returns `None`, an error is returned.
    ///
    /// # Arguments
    ///
    /// * `holon_references` - A vector of `HolonReference`s to convert.
    /// * `context` - A reference to the [`HolonsContextBehavior`] for resolving keys.
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<TestReference>)` on success.
    /// * `Err(HolonError)` if any `SmartReference` fails to resolve a valid key.
    pub fn convert_holon_references_to_test_references(
        &self,
        holon_references: &[HolonReference],
        context: &dyn HolonsContextBehavior,
    ) -> Result<Vec<TestReference>, HolonError> {
        holon_references
            .iter()
            .map(|reference| match reference {
                HolonReference::Transient(transient_ref) => {
                    Ok(TestReference::TransientHolon(transient_ref.clone()))
                }
                HolonReference::Staged(staged_ref) => {
                    Ok(TestReference::StagedHolon(staged_ref.clone()))
                }
                HolonReference::Smart(smart_ref) => match smart_ref.key(context)? {
                    Some(key) => Ok(TestReference::SavedHolon(key)),
                    None => Err(HolonError::InvalidHolonReference(
                        "SmartReference failed to provide a valid key".to_string(),
                    )),
                },
            })
            .collect()
    }
    pub fn get_created_holon_by_key(&self, key: &MapString) -> Option<Holon> {
        self.created_holons.get(key).cloned()
    }

    /// Invokes a full dance roundtrip using the current test context.
    ///
    /// This function retrieves the [`DanceInitiator`] from the active
    /// [`HolonSpaceManager`] and executes `initiate_dance()` with the given request.
    ///
    /// It panics only if the test environment is misconfigured (e.g. no initiator present),
    /// not for any normal dance-level errors — those are encoded into the returned
    /// [`DanceResponse`].
    pub async fn invoke_dance(&self, request: DanceRequest) -> DanceResponse {
        // Get the initiator — this unwrap is safe in test context setup
        let initiator = self
            .context
            .get_space_manager()
            .get_dance_initiator()
            .expect("Dance initiator must be initialized in test context");

        // Call the pipeline — always returns a DanceResponse
        initiator.initiate_dance(&*self.context, request).await
    }

    /// Resolves a [`TestReference`] into a [`HolonReference`].
    ///
    /// This function attempts to resolve a `TestReference` into a valid `HolonReference`.
    /// - If the reference is a `StagedHolon`, it directly converts it into a `HolonReference::Staged`.
    /// - If the reference is a `SavedHolon`, it looks up the corresponding `Holon` by key.
    ///   - If the `Holon` is found, it extracts its local ID and converts it into a `HolonReference`.
    ///   - If the `Holon` is not found or its local ID cannot be retrieved, an error is returned.
    ///
    /// # Arguments
    ///
    /// * `test_ref` - The [`TestReference`] to resolve.
    ///
    /// # Returns
    ///
    /// * `Ok(HolonReference)` if resolution succeeds.
    /// * `Err(HolonError::InvalidHolonReference)` if:
    ///   - The referenced `SavedHolon` does not exist.
    ///   - Retrieving the local ID of the `Holon` fails.
    ///
    /// # Errors
    ///
    /// This function returns a `HolonError::InvalidHolonReference` if:
    /// - The given `SavedHolon` key does not correspond to a known `Holon`.
    /// - Retrieving the `Holon`'s local ID fails.

    pub fn resolve_test_reference(
        &self,
        test_ref: &TestReference,
    ) -> Result<HolonReference, HolonError> {
        match test_ref {
            TestReference::TransientHolon(transient_reference) => {
                Ok(HolonReference::Transient(transient_reference.clone()))
            }
            TestReference::StagedHolon(staged_reference) => {
                Ok(HolonReference::Staged(staged_reference.clone()))
            }
            TestReference::SavedHolon(key) => {
                let holon = self.get_created_holon_by_key(key).ok_or_else(|| {
                    HolonError::InvalidHolonReference(format!(
                        "Couldn't resolve TestReference for SavedHolon({})",
                        key
                    ))
                })?;

                let holon_id = HolonId::from(holon.holon_id().map_err(|e| {
                    HolonError::InvalidHolonReference(format!(
                        "Couldn't resolve TestReference for SavedHolon({}): {}",
                        key, e
                    ))
                })?);

                Ok(HolonReference::from_id(holon_id))
            }
        }
    }
    /// Resolves a vector of [`TestReference`]s into a vector of [`HolonReference`]s.
    ///
    /// # Arguments
    ///
    /// * `test_refs` - A vector of `TestReference`s to resolve.
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<HolonReference>)` if all references are successfully resolved.
    /// * `Err(HolonError)` if any reference fails to resolve.
    ///
    /// # Errors
    ///
    /// If any `TestReference` cannot be resolved, this function returns the first encountered error.
    ///
    pub fn resolve_test_reference_vector(
        &self,
        test_refs: &[TestReference],
    ) -> Result<Vec<HolonReference>, HolonError> {
        test_refs.iter().map(|test_ref| self.resolve_test_reference(test_ref)).collect()
    }
}

impl DancesTestCase {
    pub fn new(name: String, description: String) -> Self {
        Self {
            name,
            description,
            steps: VecDeque::new(),
            test_session_state: TestSessionState::default(),
        }
    }

    /// Loads the current test_session_state from the fixture_context the given `TestSessionState` instance.
    ///
    /// This function exports transient holons from the HolonSpaceManager and injects them into
    /// the provided `session_state`, ensuring that the outgoing `TestCase` includes
    /// the latest state from the local context.
    ///
    /// # Arguments
    /// * `fixture_context` - A reference to the `HolonsContextBehavior`, which provides access to the space manager.
    /// * `test_session_state` - A mutable reference to the `TestSessionState` that will be updated with transient holons.
    ///
    /// This function is called automatically within `rs_test` and should not be used directly.
    pub fn load_test_session_state(&mut self, fixture_context: &dyn HolonsContextBehavior) {
        let space_manager = fixture_context.get_space_manager();
        let transient_holons = space_manager.export_transient_holons().unwrap();
        self.test_session_state.set_transient_holons(transient_holons);
    }

    // == STEPS == //

    pub fn add_abandon_staged_changes_step(
        &mut self,
        staged_reference: HolonReference,
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

    pub fn add_load_holons_step(
        &mut self,
        set: TransientReference,
        expect_staged: MapInteger,
        expect_committed: MapInteger,
        expect_links_created: MapInteger,
        expect_errors: MapInteger,
        expect_total_bundles: MapInteger,
        expect_total_loader_holons: MapInteger,
    ) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::LoadHolons {
            set,
            expect_staged,
            expect_committed,
            expect_links_created,
            expect_errors,
            expect_total_bundles,
            expect_total_loader_holons,
        });
        Ok(())
    }

    pub fn add_match_saved_content_step(&mut self) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::MatchSavedContent);
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

    pub fn add_add_related_holons_step(
        &mut self,
        staged_holon: HolonReference, // "owning" source Holon, which owns the Relationship
        relationship_name: RelationshipName,
        related_holons: Vec<TestReference>,
        expected_response: ResponseStatusCode,
        expected_holon: HolonReference,
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

    pub fn add_remove_properties_step(
        &mut self,
        holon: HolonReference,
        properties: PropertyMap,
        expected_response: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::RemoveProperties(holon, properties, expected_response));
        Ok(())
    }

    pub fn add_remove_related_holons_step(
        &mut self,
        holon: HolonReference, // "owning" source Holon, which owns the Relationship
        relationship_name: RelationshipName,
        related_holons: Vec<HolonReference>,
        expected_response: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::RemoveRelatedHolons(
            holon,
            relationship_name,
            related_holons,
            expected_response,
        ));
        Ok(())
    }

    pub fn add_stage_holon_step(&mut self, holon: TransientReference) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::StageHolon(holon));
        Ok(())
    }

    pub fn add_stage_new_from_clone_step(
        &mut self,
        original_holon: TestReference,
        new_key: MapString,
        expected_response: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::StageNewFromClone(
            original_holon,
            new_key,
            expected_response,
        ));
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

    pub fn add_with_properties_step(
        &mut self,
        holon: HolonReference,
        properties: PropertyMap,
        expected_response: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push_back(DanceTestStep::WithProperties(holon, properties, expected_response));
        Ok(())
    }
}
