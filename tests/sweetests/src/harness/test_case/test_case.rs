// sweetests/src/harness/test_case/test_case.rs (excerpt)

use crate::{harness::fixtures_support::TestReference, FixtureHolons};
use holons_core::core_shared_objects::holon_pool::SerializableHolonPool;
use holons_prelude::prelude::*;

/// Public test case type that collects steps to be executed later.
#[derive(Default, Clone, Debug)]
pub struct DancesTestCase {
    pub name: String,
    pub description: String,
    pub steps: Vec<DanceTestStep>,
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

/// - The source *token* is a TestReference that is *embedded as input* for the step. Executors will look it up at runtime
///   (Saved ≙ Staged(Committed(LocalId)) enforced at lookup time).
/// - The adders do **not** mint or return tokens; use FixtureHolons
///   to produce any “promise” tokens you want to chain in the fixture.
impl DancesTestCase {
    pub fn new<S: Into<String>>(name: S, description: S) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            steps: Vec::new(),
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

    pub fn add_abandon_staged_changes_step(
        &mut self,
        fixture_holons: &mut FixtureHolons,
        holon_token: TestReference,
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        self.steps.push(DanceTestStep::AbandonStagedChanges {
            holon_token: holon_token.clone(),
            expected_status,
        });
        let abandoned_token = fixture_holons.abandon_staged(&holon_token)?;

        Ok(abandoned_token)
    }

    pub fn add_commit_step(
        &mut self,
        fixture_holons: &mut FixtureHolons,
        expected_status: ResponseStatusCode,
    ) -> Result<Vec<TestReference>, HolonError> {
        let saved_tokens = fixture_holons.commit()?;
        self.steps
            .push(DanceTestStep::Commit { saved_tokens: saved_tokens.clone(), expected_status });

        Ok(saved_tokens)
    }

    pub fn add_database_print_step(&mut self) -> Result<(), HolonError> {
        self.steps.push(DanceTestStep::PrintDatabase);

        Ok(())
    }
    pub fn add_delete_holon_step(
        &mut self,
        fixture_holons: &mut FixtureHolons,
        saved_token: TestReference,
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        self.steps
            .push(DanceTestStep::DeleteHolon { saved_token: saved_token.clone(), expected_status });

        let deleted_holon = fixture_holons.delete_saved(&saved_token)?;

        Ok(deleted_holon)
    }
    pub fn add_ensure_database_count_step(
        &mut self,
        expected_count: MapInteger,
    ) -> Result<(), HolonError> {
        self.steps.push(DanceTestStep::EnsureDatabaseCount { expected_count });

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
        self.steps.push(DanceTestStep::LoadHolons {
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
        self.steps.push(DanceTestStep::MatchSavedContent);

        Ok(())
    }

    pub fn add_query_relationships_step(
        &mut self,
        source_token: TestReference,
        query_expression: QueryExpression,
        expected_status: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push(DanceTestStep::QueryRelationships {
            source_token,
            query_expression,
            expected_status,
        });

        Ok(())
    }

    pub fn add_add_related_holons_step(
        &mut self,
        source_token: TestReference, // "owning" source Holon, which owns the Relationship
        relationship_name: RelationshipName,
        holons_to_add: Vec<TestReference>,
        expected_status: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push(DanceTestStep::AddRelatedHolons {
            source_token: source_token.clone(),
            relationship_name,
            holons_to_add,
            expected_status,
        });

        Ok(())
    }

    pub fn add_remove_properties_step(
        &mut self,
        holon_token: TestReference,
        properties: PropertyMap,
        expected_status: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push(DanceTestStep::RemoveProperties {
            holon_token: holon_token.clone(),
            properties,
            expected_status,
        });

        Ok(())
    }

    pub fn add_remove_related_holons_step(
        &mut self,
        source_token: TestReference, // "owning" source Holon, which owns the Relationship
        relationship_name: RelationshipName,
        holons_to_remove: Vec<TestReference>,
        expected_status: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push(DanceTestStep::RemoveRelatedHolons {
            source_token: source_token.clone(),
            relationship_name,
            holons_to_remove,
            expected_status,
        });

        Ok(())
    }

    pub fn add_stage_holon_step(
        &mut self,
        fixture_holons: &mut FixtureHolons,
        holon_token: TestReference,
        key: Option<MapString>, // Passing key for convenience from fixture, this is not necessary for the dance but helps with minting
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        self.steps
            .push(DanceTestStep::StageHolon { holon_token: holon_token.clone(), expected_status });
        let staged_token = {
            if let Some(key) = key {
                // Mint a staged-intent token indexed by key.
                fixture_holons.add_staged_with_key(
                    holon_token.transient(),
                    key,
                    holon_token.expected_content(),
                )?
            } else {
                // Mint a staged-intent token without a key.
                fixture_holons.add_staged(holon_token.transient(), holon_token.expected_content())
            }
        };

        Ok(staged_token)
    }

    pub fn add_stage_new_from_clone_step(
        &mut self,
        fixture_holons: &mut FixtureHolons,
        source_token: TestReference,
        new_key: MapString, // Passing the key is necessary for the dance  // TODO: Change to Option
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        self.steps.push(DanceTestStep::StageNewFromClone {
            source_token: source_token.clone(),
            new_key: new_key.clone(),
            expected_status,
        });
        // Mint a staged-intent token indexed by key.
        let staged_token = fixture_holons.add_staged_with_key(
            source_token.transient(),
            new_key,
            source_token.expected_content(),
        )?;

        Ok(staged_token)
    }

    pub fn add_stage_new_version_step(
        &mut self,
        fixture_holons: &mut FixtureHolons,
        source_token: TestReference,
        key: Option<MapString>, // Passing key for convenience from fixture, this is not necessary for the dance but helps with minting
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        self.steps.push(DanceTestStep::StageNewVersion {
            source_token: source_token.clone(),
            expected_status,
        });
        let staged_token = {
            if let Some(key) = key {
                // Mint a staged-intent token indexed by key.
                fixture_holons.add_staged_with_key(
                    source_token.transient(),
                    key,
                    source_token.expected_content(),
                )?
            } else {
                // Mint a staged-intent token without a key.
                fixture_holons.add_staged(source_token.transient(), source_token.expected_content())
            }
        };

        Ok(staged_token)
    }

    pub fn add_with_properties_step(
        &mut self,
        holon_token: TestReference,
        properties: PropertyMap,
        expected_status: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push(DanceTestStep::WithProperties {
            holon_token: holon_token.clone(),
            properties,
            expected_status,
        });

        Ok(())
    }
}

/// Internal step representation used by executors at runtime.
#[derive(Clone, Debug)]
pub enum DanceTestStep {
    AbandonStagedChanges {
        holon_token: TestReference,
        expected_status: ResponseStatusCode,
    },
    AddRelatedHolons {
        source_token: TestReference,
        relationship_name: RelationshipName,
        holons_to_add: Vec<TestReference>,
        expected_status: ResponseStatusCode,
    },
    Commit {
        saved_tokens: Vec<TestReference>, // Used to match expected
        expected_status: ResponseStatusCode,
    },
    DeleteHolon {
        saved_token: TestReference,
        expected_status: ResponseStatusCode,
    },
    EnsureDatabaseCount {
        expected_count: MapInteger,
    },
    LoadHolons {
        set: TransientReference,
        expect_staged: MapInteger,
        expect_committed: MapInteger,
        expect_links_created: MapInteger,
        expect_errors: MapInteger,
        expect_total_bundles: MapInteger,
        expect_total_loader_holons: MapInteger,
    },
    MatchSavedContent,
    PrintDatabase,
    QueryRelationships {
        source_token: TestReference,
        query_expression: QueryExpression,
        expected_status: ResponseStatusCode,
    },
    RemoveProperties {
        holon_token: TestReference,
        properties: PropertyMap,
        expected_status: ResponseStatusCode,
    },
    RemoveRelatedHolons {
        source_token: TestReference,
        relationship_name: RelationshipName,
        holons_to_remove: Vec<TestReference>,
        expected_status: ResponseStatusCode,
    },
    StageHolon {
        holon_token: TestReference,
        expected_status: ResponseStatusCode,
    },
    StageNewFromClone {
        source_token: TestReference,
        new_key: MapString,
        expected_status: ResponseStatusCode,
    },
    StageNewVersion {
        source_token: TestReference,
        expected_status: ResponseStatusCode,
    },
    WithProperties {
        holon_token: TestReference,
        properties: PropertyMap,
        expected_status: ResponseStatusCode,
    },
}

impl core::fmt::Display for DanceTestStep {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DanceTestStep::AbandonStagedChanges { holon_token, expected_status } => {
                write!(
                    f,
                    "Marking Holon at ({:?}) as Abandoned, expecting ({:?})",
                    holon_token, expected_status
                )
            }
            DanceTestStep::AddRelatedHolons {
                source_token,
                relationship_name,
                holons_to_add,
                expected_status,
            } => {
                write!(f, "AddRelatedHolons to Holon {:#?} for relationship: {:#?}, added_count: {:#?}, expecting: {:#?}", source_token, relationship_name, holons_to_add.len(), expected_status)
            }
            DanceTestStep::Commit { saved_tokens, expected_status } => {
                write!(f, "Committing {:#?}, expecting: {:?})", saved_tokens, expected_status)
            }
            DanceTestStep::DeleteHolon { saved_token, expected_status } => {
                write!(f, "DeleteHolon({:?}, expecting: {:?},)", saved_token, expected_status)
            }
            DanceTestStep::EnsureDatabaseCount { expected_count } => {
                write!(f, "EnsureDatabaseCount = {}", expected_count.0)
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
            DanceTestStep::PrintDatabase => {
                write!(f, "PrintDatabase")
            }
            DanceTestStep::QueryRelationships {
                source_token,
                query_expression,
                expected_status,
            } => {
                write!(f, "QueryRelationships for source:{:#?}, with query expression: {:#?}, expecting {:#?}", source_token, query_expression, expected_status)
            }
            DanceTestStep::RemoveProperties { holon_token, properties, expected_status } => {
                write!(
                    f,
                    "RemoveProperties {:#?} for Holon {:#?}, expecting {:#?} ",
                    properties, holon_token, expected_status,
                )
            }
            DanceTestStep::RemoveRelatedHolons {
                source_token,
                relationship_name,
                holons_to_remove,
                expected_status,
            } => {
                write!(f, "RemoveRelatedHolons from Holon {:#?} for relationship: {:#?}, added_count: {:#?}, expecting: {:#?}", source_token, relationship_name, holons_to_remove.len(), expected_status)
            }
            DanceTestStep::StageHolon { holon_token, expected_status } => {
                write!(f, "StageHolon({:?}, expecting: {:?},)", holon_token, expected_status)
            }
            DanceTestStep::StageNewVersion { source_token, expected_status } => {
                write!(
                    f,
                    "NewVersion for source: {:#?}, expecting response: {:#?}",
                    source_token, expected_status
                )
            }
            DanceTestStep::StageNewFromClone { source_token, new_key, expected_status } => {
                write!(
                    f,
                    "StageNewFromClone for original_holon: {:#?}, with new key: {:?}, expecting response: {:#?}",
                    source_token, new_key, expected_status
                )
            }
            DanceTestStep::WithProperties { holon_token, properties, expected_status } => {
                write!(
                    f,
                    "WithProperties for Holon {:#?} with properties: {:#?}, expecting {:#?} ",
                    holon_token, properties, expected_status,
                )
            }
        }
    }
}
