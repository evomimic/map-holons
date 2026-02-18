<<<<<<< HEAD
//! # Dance Test Language
//!
//! This module defines the **declarative language** used by MAP sweetests to
//! describe integration test behavior in terms of *dance execution*.
//!
//! It does **not** execute tests and does **not** define any concrete test
//! scenarios. Instead, it defines the **grammar, structure, and construction
//! API** used by test fixtures to *author* test cases that are executed later
//! by the sweetests harness.
//!
//! Specifically, this module provides:
//!
//! - [`TestCaseInit`], which initializes the test_case and context, with empty mutable
//!    fixture holons and bindings objects.
//! - [`DancesTestCase`], a container representing a single declarative test
//!   program composed of an ordered sequence of steps.
//! - [`DanceTestStep`], a closed vocabulary of test operations, each
//!   corresponding to one or more MAP dances or assertions.
//! - Builder-style `add_*` methods for constructing test cases in a clear,
//!   sequential, and intention-revealing manner.
//! - [`TestSessionState`], which captures transient holon state produced during
//!   fixture setup and injects it into the test execution context.
//!
//! Test cases constructed using this language are *pure specifications*:
//! they contain no runtime context, no concrete holon identifiers, and no
//! execution logic. Resolution of references, state mutation, and dance
//! invocation are handled entirely by the execution support layer at runtime.
//!
//! ## Architectural Role
//!
//! Within the sweetests harness, this module occupies a middle layer between:
//!
//! - **fixtures_support**, which mints symbolic [`TestReference`] tokens and
//!   assembles test cases using this language, and
//! - **execution_support**, which interprets and executes the resulting test
//!   cases against client- and guest-side contexts.
//!
//! This separation allows test behavior to be described declaratively while
//! remaining independent of runtime identifiers and execution-time handles

=======
>>>>>>> 6cd89d9b (-file restructure)
use crate::{
    harness::fixtures_support::TestReference, DanceTestStep, DancesTestCase, ExpectedSnapshot,
    FixtureHolons, SourceSnapshot, TestHolonState, TestSessionState,
};
use core_types::ContentSet;
use holons_boundary::SerializableHolonPool;
use holons_core::{
    core_shared_objects::{holon, transactions::TransactionContext},
    reference_layer::ReadableHolon,
};
use holons_prelude::prelude::*;
use integrity_core_types::PropertyMap;
use std::sync::Arc;

/// - The source *token* is a TestReference that is *embedded as input* for the step. Executors will look it up at runtime
///   (Saved ≙ Staged(Committed(LocalId)) enforced at lookup time).
/// - The adders mint and return tokens to be used for subsequent steps.
impl DancesTestCase {
    pub fn new<S: Into<String>>(name: S, description: S) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            steps: Vec::new(),
            test_session_state: TestSessionState::default(),
            is_finalized: false,
        }
    }

    pub fn finalize(
        &mut self,
        fixture_context: &Arc<TransactionContext>,
    ) -> Result<(), HolonError> {
        self.load_test_session_state(fixture_context);
        if self.is_finalized == true {
            panic!("DancesTestCase already finalized!")
        }
        self.is_finalized = true;

        Ok(())
    }

    /// Loads the current test_session_state from the fixture_context the given `TestSessionState` instance.
    ///
    /// This function exports transient holons from the HolonSpaceManager and injects them into
    /// the provided `session_state`, ensuring that the outgoing `TestCase` includes
    /// the latest state from the local context.
    ///
    /// # Arguments
    /// * `fixture_context` - A reference to the `TransactionContext`, which provides access to the space manager.
    /// * `test_session_state` - A mutable reference to the `TestSessionState` that will be updated with transient holons.
    ///
    /// This function is called automatically within `rs_test` and should not be used directly.
    pub fn load_test_session_state(&mut self, fixture_context: &Arc<TransactionContext>) {
        let transient_holons = fixture_context.export_transient_holons().unwrap();
        self.test_session_state
            .set_transient_holons(SerializableHolonPool::from(&transient_holons));
    }

    // === Execution Steps === //

    pub fn add_database_print_step(&mut self) -> Result<(), HolonError> {
        if self.is_finalized == true {
            return Err(HolonError::Misc(
                "DancesTestCase is already finalized, thus closed for any additional steps"
                    .to_string(),
            ));
        }
        self.steps.push(DanceTestStep::PrintDatabase);

        Ok(())
    }

    pub fn add_ensure_database_count_step(
        &mut self,
        expected_count: MapInteger,
        description: Option<String>,
    ) -> Result<(), HolonError> {
        if self.is_finalized == true {
            return Err(HolonError::Misc(
                "DancesTestCase is already finalized, thus closed for any additional steps"
                    .to_string(),
            ));
        }
        self.steps.push(DanceTestStep::EnsureDatabaseCount { expected_count, description });

        Ok(())
    }

    pub fn add_load_holons_client_step(
        &mut self,
        content_set: ContentSet,
        expect_staged: MapInteger,
        expect_committed: MapInteger,
        expect_links_created: MapInteger,
        expect_errors: MapInteger,
        expect_total_bundles: MapInteger,
        expect_total_loader_holons: MapInteger,
    ) -> Result<(), HolonError> {
        if self.is_finalized == true {
            return Err(HolonError::Misc(
                "DancesTestCase is already finalized, thus closed for any additional steps"
                    .to_string(),
            ));
        }
        self.steps.push(DanceTestStep::LoadHolonsClient {
            content_set,
            expect_staged,
            expect_committed,
            expect_links_created,
            expect_errors,
            expect_total_bundles,
            expect_total_loader_holons,
        });

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
        if self.is_finalized == true {
            return Err(HolonError::Misc(
                "DancesTestCase is already finalized, thus closed for any additional steps"
                    .to_string(),
            ));
        }
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
        if self.is_finalized == true {
            return Err(HolonError::Misc(
                "DancesTestCase is already finalized, thus closed for any additional steps"
                    .to_string(),
            ));
        }
        self.steps.push(DanceTestStep::MatchSavedContent);

        Ok(())
    }

    pub fn add_query_relationships_step(
        &mut self,
        step_token: TestReference,
        query_expression: QueryExpression,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    ) -> Result<(), HolonError> {
        if self.is_finalized == true {
            return Err(HolonError::Misc(
                "DancesTestCase is already finalized, thus closed for any additional steps"
                    .to_string(),
            ));
        }
        self.steps.push(DanceTestStep::QueryRelationships {
            step_token,
            query_expression,
            expected_status,
            description,
        });

        Ok(())
    }

    // === Execution Steps with === //
    // ==== Token Minting ==== //

    // Note: adders use the expected snapshot from the step_token passed in as the new source for the execution step.

    // Advance head snapshot (no new logical holon).
    pub fn add_abandon_staged_changes_step(
        &mut self,
        fixture_holons: &mut FixtureHolons,
        step_token: TestReference,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    ) -> Result<TestReference, HolonError> {
        if self.is_finalized == true {
            return Err(HolonError::Misc(
                "DancesTestCase is already finalized, thus closed for any additional steps"
                    .to_string(),
            ));
        }
        // Cloning new source to create the expected snapshot
        let new_source = fixture_holons.derive_next_source(&step_token)?;
        let new_snapshot = new_source.snapshot().clone_holon()?;
        let expected = ExpectedSnapshot::new(new_snapshot, TestHolonState::Abandoned);
        if expected_status == ResponseStatusCode::OK {
            // Advance head snapshot for the FixtureHolon
            fixture_holons.advance_head(&step_token.expected_id(), expected.clone())?;
        }
        // Mint
        let new_step_token = fixture_holons.mint_test_reference(new_source, expected);

        // Add execution step
        self.steps.push(DanceTestStep::AbandonStagedChanges {
            step_token: new_step_token.clone(),
            expected_status,
            description,
        });

        Ok(new_step_token)
    }

    // Advance head snapshot (no new logical holon).
    pub fn add_delete_holon_step(
        &mut self,
        fixture_holons: &mut FixtureHolons,
        step_token: TestReference,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    ) -> Result<(), HolonError> {
        if self.is_finalized == true {
            return Err(HolonError::Misc(
                "DancesTestCase is already finalized, thus closed for any additional steps"
                    .to_string(),
            ));
        }
        // Cloning new source to create the expected snapshot
        let new_source = fixture_holons.derive_next_source(&step_token)?;
        let new_snapshot = new_source.snapshot().clone_holon()?;
        let expected = ExpectedSnapshot::new(new_snapshot, TestHolonState::Deleted);
        if expected_status == ResponseStatusCode::OK {
            // Advance head snapshot for the FixtureHolon
            fixture_holons.advance_head(&step_token.expected_id(), expected.clone())?;
        }
        // Mint
        let new_step_token = fixture_holons.mint_test_reference(new_source, expected);

        // Add execution step
        self.steps.push(DanceTestStep::DeleteHolon {
            step_token: new_step_token.clone(),
            expected_status,
            description,
        });

        Ok(())
    }

    // Commit advances head snapshots to Saved for existing logical holons.
    pub fn add_commit_step(
        &mut self,
        fixture_holons: &mut FixtureHolons,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    ) -> Result<(), HolonError> {
        if self.is_finalized == true {
            return Err(HolonError::Misc(
                "DancesTestCase is already finalized, thus closed for any additional steps"
                    .to_string(),
            ));
        }
<<<<<<< HEAD:tests/sweetests/src/harness/test_case/adders.rs
        let saved_tokens = fixture_holons.commit()?;
        self.steps.push(DanceTestStep::Commit { saved_tokens, expected_status });

        Ok(())
    }

    // Special step that creates a new 'freshly minted' TransientReference,
    // i.e. the first snapshot for a FixtureHolon.
    pub fn add_new_holon_step(
        &mut self,
        fixture_holons: &mut FixtureHolons,
        source_reference: TransientReference,
        properties: PropertyMap,
        key: Option<MapString>,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    ) -> Result<TestReference, HolonError> {
        if self.is_finalized == true {
            return Err(HolonError::Misc(
                "DancesTestCase is already finalized, thus closed for any additional steps"
                    .to_string(),
            ));
        }
        let mut snapshot = source_reference.clone_holon()?;
        for (name, value) in properties.clone() {
            snapshot.with_property_value(name, value)?;
        }
        let source = SourceSnapshot::new(source_reference, TestHolonState::Transient);
        let expected = ExpectedSnapshot::new(snapshot, TestHolonState::Transient);
        fixture_holons.create_fixture_holon(expected.clone())?;
        let new_token = fixture_holons.mint_test_reference(source, expected);

        // Add execution step
        self.steps.push(DanceTestStep::NewHolon {
            step_token: new_token.clone(),
            properties,
            key,
            expected_status,
            description,
        });

        Ok(new_token)
    }

    // Advance head (no new logical holon).
    pub fn add_add_related_holons_step(
        &mut self,
        fixture_holons: &mut FixtureHolons,
        step_token: TestReference,
        relationship_name: RelationshipName,
        holons_to_add: Vec<TestReference>,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    ) -> Result<TestReference, HolonError> {
        // Cloning new source to create the expected snapshot
        let new_source = fixture_holons.derive_next_source(&step_token)?;
        let mut new_snapshot = new_source.snapshot().clone_holon()?;
        let mut references_to_add: Vec<HolonReference> = Vec::new();
        for token in &holons_to_add {
            references_to_add.push(token.expected_reference().into());
        }
        new_snapshot.add_related_holons(&relationship_name, references_to_add)?;

        let expected = ExpectedSnapshot::new(new_snapshot, new_source.state());
        // Advance head snapshot for the FixtureHolon
        fixture_holons.advance_head(&step_token.expected_id(), expected.clone())?;
        // Mint
        let new_token = fixture_holons.mint_test_reference(new_source, expected);
        // Add execution step
        self.steps.push(DanceTestStep::AddRelatedHolons {
            step_token: step_token.clone(),
            relationship_name,
            holons_to_add,
            expected_status,
            description,
        });

        Ok(new_token)
    }

    // Advance head (no new logical holon).
    pub fn add_remove_properties_step(
        &mut self,
        fixture_holons: &mut FixtureHolons,
        step_token: TestReference,
        properties: PropertyMap,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    ) -> Result<TestReference, HolonError> {
        if self.is_finalized == true {
            return Err(HolonError::Misc(
                "DancesTestCase is already finalized, thus closed for any additional steps"
                    .to_string(),
            ));
        }
        // Cloning new source to create the expected snapshot
        let new_source = fixture_holons.derive_next_source(&step_token)?;
        let mut new_snapshot = new_source.snapshot().clone_holon()?;
        for property in properties.keys() {
            new_snapshot.remove_property_value(property)?;
        }
        let expected = ExpectedSnapshot::new(new_snapshot, new_source.state());
        if expected_status == ResponseStatusCode::OK {
            // Advance head snapshot for the FixtureHolon
            fixture_holons.advance_head(&step_token.expected_id(), expected.clone())?;
        }
        // Mint
        let new_token = fixture_holons.mint_test_reference(new_source, expected);
        // Add execution step
        self.steps.push(DanceTestStep::RemoveProperties {
            step_token: new_token.clone(),
            properties: properties.clone(),
            expected_status,
            description,
        });

        Ok(new_token)
    }

    // Advance head (no new logical holon).
    pub fn add_remove_related_holons_step(
        &mut self,
        fixture_holons: &mut FixtureHolons,
        step_token: TestReference,
        relationship_name: RelationshipName,
        holons_to_remove: Vec<TestReference>,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    ) -> Result<TestReference, HolonError> {
        // Cloning new source to create the expected snapshot
        let new_source = fixture_holons.derive_next_source(&step_token)?;
        let mut new_snapshot = new_source.snapshot().clone_holon()?;
        let mut references_to_remove: Vec<HolonReference> = Vec::new();
        for token in &holons_to_remove {
            references_to_remove.push(token.expected_reference().into());
        }
        new_snapshot.remove_related_holons(&relationship_name, references_to_remove)?;

        let expected = ExpectedSnapshot::new(new_snapshot, new_source.state());
        // Advance head snapshot for the FixtureHolon
        fixture_holons.advance_head(&step_token.expected_id(), expected.clone())?;
        // Mint
        let new_token = fixture_holons.mint_test_reference(new_source, expected);
        // Add execution step
        self.steps.push(DanceTestStep::RemoveRelatedHolons {
            step_token: step_token.clone(),
            relationship_name,
            holons_to_remove,
            expected_status,
            description,
        });

        Ok(new_token)
    }

    // Creates new logical holon and therefore a new FixtureHolon.
    pub fn add_stage_holon_step(
        &mut self,
        fixture_holons: &mut FixtureHolons,
        step_token: TestReference,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    ) -> Result<TestReference, HolonError> {
        if self.is_finalized == true {
            return Err(HolonError::Misc(
                "DancesTestCase is already finalized, thus closed for any additional steps"
                    .to_string(),
            ));
        }
        // Cloning new source to create the expected snapshot
        let new_source = fixture_holons.derive_next_source(&step_token)?;
        let snapshot = new_source.snapshot().clone_holon()?;
        let expected = ExpectedSnapshot::new(snapshot, TestHolonState::Staged);
        if expected_status == ResponseStatusCode::OK {
            // Create new FixtureHolon
            fixture_holons.create_fixture_holon(expected.clone())?;
        }
        // Mint
        let new_token = fixture_holons.mint_test_reference(new_source, expected);

        // Add execution step
        self.steps.push(DanceTestStep::StageHolon {
<<<<<<< HEAD
<<<<<<< HEAD:tests/sweetests/src/harness/test_case/adders.rs
            step_token: new_token.clone(),
=======
            source_token: new_token.clone(),
>>>>>>> 253a0ec2 (optional descriptions for steps -- enchanced verbosity):tests/sweetests/src/harness/test_case/dance_test_language.rs
=======
            step_token: new_token.clone(),
>>>>>>> 6cd89d9b (-file restructure)
            expected_status,
            description,
        });

        Ok(new_token)
    }

    // Creates new logical holon and therefore a new FixtureHolon.
    pub fn add_stage_new_from_clone_step(
        &mut self,
        fixture_holons: &mut FixtureHolons,
        step_token: TestReference,
        new_key: MapString, // Passing the key is necessary for the dance  // TODO: Future changes will make this an Option
        expected_status: ResponseStatusCode,
        description: Option<String>,
    ) -> Result<TestReference, HolonError> {
        if self.is_finalized == true {
            return Err(HolonError::Misc(
                "DancesTestCase is already finalized, thus closed for any additional steps"
                    .to_string(),
            ));
        }
        // Cloning new source to create the expected snapshot
        let new_source = fixture_holons.derive_next_source(&step_token)?;
        let mut snapshot = new_source.snapshot().clone_holon()?;
        snapshot.with_property_value("Key", new_key.clone())?;
        let expected = ExpectedSnapshot::new(snapshot, TestHolonState::Staged);
        if expected_status == ResponseStatusCode::OK {
            // Create new FixtureHolon
            fixture_holons.create_fixture_holon(expected.clone())?;
        }
        // Mint
        let new_token = fixture_holons.mint_test_reference(new_source, expected);

        // Add execution step
        self.steps.push(DanceTestStep::StageNewFromClone {
            step_token: new_token.clone(),
            new_key: new_key.clone(),
            expected_status: expected_status.clone(),
            description,
        });

        Ok(new_token)
    }

    // Creates new logical holon and therefore a new FixtureHolon.
    pub fn add_stage_new_version_step(
        &mut self,
        fixture_holons: &mut FixtureHolons,
        step_token: TestReference,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    ) -> Result<TestReference, HolonError> {
        if self.is_finalized == true {
            return Err(HolonError::Misc(
                "DancesTestCase is already finalized, thus closed for any additional steps"
                    .to_string(),
            ));
        }
        // Cloning new source to create the expected snapshot
        let new_source = fixture_holons.derive_next_source(&step_token)?;
        let snapshot = new_source.snapshot().clone_holon()?;
        let expected = ExpectedSnapshot::new(snapshot, TestHolonState::Staged);
        if expected_status == ResponseStatusCode::OK {
            // Create new FixtureHolon
            fixture_holons.create_fixture_holon(expected.clone())?;
        }
        // Mint
        let new_token = fixture_holons.mint_test_reference(new_source, expected);

        // Add execution step
        self.steps.push(DanceTestStep::StageNewVersion {
            step_token: new_token.clone(),
            expected_status: expected_status.clone(),
            description,
        });

        Ok(new_token)
    }

    // Advance head (no new logical holon).
    pub fn add_with_properties_step(
        &mut self,
        fixture_holons: &mut FixtureHolons,
        step_token: TestReference,
        properties: PropertyMap,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    ) -> Result<TestReference, HolonError> {
        if self.is_finalized == true {
            return Err(HolonError::Misc(
                "DancesTestCase is already finalized, thus closed for any additional steps"
                    .to_string(),
            ));
        }
        // Cloning new source to create the expected snapshot
        let new_source = fixture_holons.derive_next_source(&step_token)?;
        let mut new_snapshot = new_source.snapshot().clone_holon()?;
        for (property, value) in properties.clone() {
            new_snapshot.with_property_value(property, value)?;
        }
        let expected = ExpectedSnapshot::new(new_snapshot, new_source.state());
        // Advance head snapshot for the FixtureHolon
        fixture_holons.advance_head(&step_token.expected_id(), expected.clone())?;
        // Mint
        let new_token = fixture_holons.mint_test_reference(new_source, expected);

        // Add execution step
        self.steps.push(DanceTestStep::WithProperties {
            step_token: new_token.clone(),
            properties: properties.clone(),
            expected_status,
            description,
        });

        Ok(new_token)
    }
}
<<<<<<< HEAD
<<<<<<< HEAD:tests/sweetests/src/harness/test_case/adders.rs
=======

/// Internal step representation used by executors at runtime.
#[derive(Clone, Debug)]
pub enum DanceTestStep {
    AbandonStagedChanges {
        source_token: TestReference,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    },
    AddRelatedHolons {
        source_token: TestReference,
        relationship_name: RelationshipName,
        holons_to_add: Vec<TestReference>,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    },
    Commit {
        saved_tokens: Vec<TestReference>, // Used to match expected
        expected_status: ResponseStatusCode,
        description: Option<String>,
    },
    DeleteHolon {
        source_token: TestReference,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    },
    EnsureDatabaseCount {
        expected_count: MapInteger,
        description: Option<String>,
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
    LoadHolonsClient {
        content_set: ContentSet,
        expect_staged: MapInteger,
        expect_committed: MapInteger,
        expect_links_created: MapInteger,
        expect_errors: MapInteger,
        expect_total_bundles: MapInteger,
        expect_total_loader_holons: MapInteger,
    },
    MatchSavedContent,
    NewHolon {
        source_token: TestReference,
        properties: PropertyMap,
        key: Option<MapString>,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    },
    PrintDatabase,
    QueryRelationships {
        source_token: TestReference,
        query_expression: QueryExpression,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    },
    RemoveProperties {
        source_token: TestReference,
        properties: PropertyMap,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    },
    RemoveRelatedHolons {
        source_token: TestReference,
        relationship_name: RelationshipName,
        holons_to_remove: Vec<TestReference>,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    },
    StageHolon {
        source_token: TestReference,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    },
    StageNewFromClone {
        source_token: TestReference,
        new_key: MapString,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    },
    StageNewVersion {
        source_token: TestReference,
        expected_status: ResponseStatusCode,
        version_count: MapInteger,
        expected_failure_code: Option<ResponseStatusCode>,
        description: Option<String>,
    },
    WithProperties {
        source_token: TestReference,
        properties: PropertyMap,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    },
}

impl core::fmt::Display for DanceTestStep {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DanceTestStep::AbandonStagedChanges {
                source_token,
                expected_status,
                description: _description,
            } => {
                write!(
                    f,
                    "Marking Holon at ({:?}) as Abandoned, expecting ({:?})",
                    source_token, expected_status
                )
            }
            DanceTestStep::AddRelatedHolons {
                source_token,
                relationship_name,
                holons_to_add,
                expected_status,
                description: _description,
            } => {
                write!(f, "AddRelatedHolons to Holon {:#?} for relationship: {:#?}, added_count: {:#?}, expecting: {:#?}", source_token, relationship_name, holons_to_add.len(), expected_status)
            }
            DanceTestStep::Commit { saved_tokens, expected_status, description: _description } => {
                write!(f, "Committing {:#?}, expecting: {:?})", saved_tokens, expected_status)
            }
            DanceTestStep::DeleteHolon {
                source_token,
                expected_status,
                description: _description,
            } => {
                write!(f, "DeleteHolon({:?}, expecting: {:?},)", source_token, expected_status)
            }
            DanceTestStep::EnsureDatabaseCount { expected_count, description: _description } => {
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
            DanceTestStep::LoadHolonsClient {
                expect_staged,
                expect_committed,
                expect_links_created,
                expect_errors,
                expect_total_bundles,
                expect_total_loader_holons,
                ..
            } => {
                write!(
                    f,
                    "LoadHolonsClient(staged={}, committed={}, links_created={}, errors={}, bundles={}, loader_holons={})",
                    expect_staged.0,
                    expect_committed.0,
                    expect_links_created.0,
                    expect_errors.0,
                    expect_total_bundles.0,
                    expect_total_loader_holons.0
                )
            }
            DanceTestStep::MatchSavedContent => {
                write!(f, "MatchSavedContent")
            }
            DanceTestStep::NewHolon {
                source_token,
                properties: _properties,
                key,
                expected_status,
                description: _description,
            } => {
                write!(
                    f,
                    "NewHolon({:?}, with key: {:?}, expecting: {:?},)",
                    source_token, key, expected_status
                )
            }
            DanceTestStep::PrintDatabase => {
                write!(f, "PrintDatabase")
            }
            DanceTestStep::QueryRelationships {
                source_token,
                query_expression,
                expected_status,
                description: _description,
            } => {
                write!(f, "QueryRelationships for source:{:#?}, with query expression: {:#?}, expecting {:#?}", source_token, query_expression, expected_status)
            }
            DanceTestStep::RemoveProperties {
                source_token,
                properties,
                expected_status,
                description: _description,
            } => {
                write!(
                    f,
                    "RemoveProperties {:#?} for Holon {:#?}, expecting {:#?} ",
                    properties, source_token, expected_status,
                )
            }
            DanceTestStep::RemoveRelatedHolons {
                source_token,
                relationship_name,
                holons_to_remove,
                expected_status,
                description: _description,
            } => {
                write!(f, "RemoveRelatedHolons from Holon {:#?} for relationship: {:#?}, added_count: {:#?}, expecting: {:#?}", source_token, relationship_name, holons_to_remove.len(), expected_status)
            }
            DanceTestStep::StageHolon {
                source_token,
                expected_status,
                description: _description,
            } => {
                write!(f, "StageHolon({:?}, expecting: {:?},)", source_token, expected_status)
            }
            DanceTestStep::StageNewVersion {
                source_token,
                expected_status,
                version_count: _version_count,
                expected_failure_code,
                description: _description,
            } => {
                write!(
                    f,
                    "NewVersion for source: {:#?}, expecting response: {:#?}, optional failure_code: {:?}",
                    source_token, expected_status, expected_failure_code,
                )
            }
            DanceTestStep::StageNewFromClone {
                source_token,
                new_key,
                expected_status,
                description: _description,
            } => {
                write!(
                    f,
                    "StageNewFromClone for original_holon: {:#?}, with new key: {:?}, expecting response: {:#?}",
                    source_token, new_key, expected_status
                )
            }
            DanceTestStep::WithProperties {
                source_token,
                properties,
                expected_status,
                description: _description,
            } => {
                write!(
                    f,
                    "WithProperties for Holon {:#?} with properties: {:#?}, expecting {:#?} ",
                    source_token, properties, expected_status
                )
            }
        }
    }
}
>>>>>>> 253a0ec2 (optional descriptions for steps -- enchanced verbosity):tests/sweetests/src/harness/test_case/dance_test_language.rs
=======
>>>>>>> 6cd89d9b (-file restructure)
