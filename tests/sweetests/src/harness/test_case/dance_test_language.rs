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


use std::sync::Arc;

use crate::{harness::fixtures_support::TestReference, init_fixture_context, ExpectedSnapshot, FixtureBindings, FixtureHolons, SourceSnapshot, TestHolonState};
use core_types::ContentSet;
use holons_core::{
    core_shared_objects::{
        holon_pool::SerializableHolonPool,
        transactions::TransactionContext,
    },
    reference_layer::ReadableHolon,
};
use holons_prelude::prelude::*;
use integrity_core_types::PropertyMap;

/// Public test case type that collects steps to be executed later.
#[derive(Default, Clone, Debug)]
pub struct DancesTestCase {
    pub name: String,
    pub description: String,
    pub steps: Vec<DanceTestStep>,
    pub test_session_state: TestSessionState,
    pub is_finalized: bool,
}

/// TestCaseInit provides a structured, atomic initialization context for constructing a TestCase together with all required harness-managed fixture-time state.
/// It answers the question: “What must exist, together, in order to author a valid TestCase?”
/// Responsibilities:
/// - Ensure all required harness components are created together
/// - Make initialization explicit and difficult to misuse
/// - Avoid fragile tuple-based or ad-hoc setup
/// - Establish clear ownership boundaries from the outset
/// - Keep the DancesTestCase itself as the primary author-facing artifact
pub struct TestCaseInit {
    pub test_case: DancesTestCase,
    pub fixture_context: Arc<dyn HolonsContextBehavior>,
    pub fixture_holons: FixtureHolons,
    pub fixture_bindings: FixtureBindings,
}

impl TestCaseInit {
    pub fn new(name: String, description: String) -> Self {
        let context = init_fixture_context();
        let mut test_case = DancesTestCase::default();
        test_case.name = name;
        test_case.description = description;

        Self {
            test_case,
            fixture_context: context,
            fixture_holons: FixtureHolons::default(),
            fixture_bindings: FixtureBindings::default(),
        }
    }
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
        fixture_context: &dyn HolonsContextBehavior,
    ) -> Result<(), HolonError> {
        self.load_test_session_state(fixture_context);
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
    pub fn load_test_session_state(&mut self, fixture_context: &TransactionContext) {
        let transient_holons = fixture_context.export_transient_holons().unwrap();
        self.test_session_state.set_transient_holons(transient_holons);
    }

    // === Execution Steps === //

    pub fn add_database_print_step(&mut self) -> Result<(), HolonError> {
        self.steps.push(DanceTestStep::PrintDatabase);

        Ok(())
    }

    pub fn add_ensure_database_count_step(
        &mut self,
        expected_count: MapInteger,
    ) -> Result<(), HolonError> {
        self.steps.push(DanceTestStep::EnsureDatabaseCount { expected_count });

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

    // === Execution Steps with === //
    // ==== Token Minting ==== //

    // Note: adders use the expected snapshot from the source_token passed in as the new source for the execution step.

    // Advance head snapshot (no new logical holon).
    pub fn add_abandon_staged_changes_step(
        &mut self,
        context: &dyn HolonsContextBehavior,
        fixture_holons: &mut FixtureHolons,
        source_token: TestReference,
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        // Cloning new source to create the expected snapshot
        let new_source = fixture_holons.derive_next_source(&source_token)?;
        let new_snapshot = new_source.snapshot().clone_holon(context)?;
        let expected = ExpectedSnapshot::new(new_snapshot, TestHolonState::Abandoned);
        if expected_status == ResponseStatusCode::OK {
            // Advance head snapshot for the FixtureHolon
            fixture_holons.advance_head(&source_token.expected_id(), expected.clone())?;
        }
        // Mint
        let new_source_token = fixture_holons.mint_test_reference(new_source, expected);

        // Add execution step
        self.steps.push(DanceTestStep::AbandonStagedChanges {
            source_token: new_source_token.clone(),
            expected_status,
        });

        Ok(new_source_token)
    }

    // Advance head snapshot (no new logical holon).
    pub fn add_delete_holon_step(
        &mut self,
        context: &dyn HolonsContextBehavior,
        fixture_holons: &mut FixtureHolons,
        source_token: TestReference,
        expected_status: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        // Cloning new source to create the expected snapshot
        let new_source = fixture_holons.derive_next_source(&source_token)?;
        let new_snapshot = new_source.snapshot().clone_holon(context)?;
        let expected = ExpectedSnapshot::new(new_snapshot, TestHolonState::Deleted);
        if expected_status == ResponseStatusCode::OK {
            // Advance head snapshot for the FixtureHolon
            fixture_holons.advance_head(&source_token.expected_id(), expected.clone())?;
        }
        // Mint
        let new_source_token = fixture_holons.mint_test_reference(new_source, expected);

        // Add execution step
        self.steps.push(DanceTestStep::DeleteHolon {
            source_token: new_source_token.clone(),
            expected_status,
        });

        Ok(())
    }

    // Commit advances head snapshots to Saved for existing logical holons.
    pub fn add_commit_step(
        &mut self,
        context: &dyn HolonsContextBehavior,
        fixture_holons: &mut FixtureHolons,
        expected_status: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        let saved_tokens = fixture_holons.commit(context)?;
        self.steps.push(DanceTestStep::Commit { saved_tokens, expected_status });

        Ok(())
    }

    // Special step that creates a new 'freshly minted' TransientReference,
    // i.e. the first snapshot for a FixtureHolon.
    pub fn add_new_holon_step(
        &mut self,
        context: &dyn HolonsContextBehavior,
        fixture_holons: &mut FixtureHolons,
        source_reference: TransientReference,
        properties: PropertyMap,
        key: Option<MapString>,
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        let mut snapshot = source_reference.clone_holon(context)?;
        for (name, value) in properties.clone() {
            snapshot.with_property_value(context, name, value)?;
        }
        let source = SourceSnapshot::new(source_reference, TestHolonState::Transient);
        let expected = ExpectedSnapshot::new(snapshot, TestHolonState::Transient);
        fixture_holons.create_fixture_holon(expected.clone())?;
        let new_token = fixture_holons.mint_test_reference(source, expected);

        // Add execution step
        self.steps.push(DanceTestStep::NewHolon {
            source_token: new_token.clone(),
            properties,
            key,
            expected_status,
        });

        Ok(new_token)
    }

    // TODO: Support for relationships to be finished in issue 382

    // Advance head (no new logical holon).
    // pub fn add_add_related_holons_step(
    //     &mut self,
    //     context: &dyn HolonsContextBehavior,
    //     source_token: TestReference, // "owning" source Holon, which owns the Relationship
    //     relationship_name: RelationshipName,
    //     holons_to_add: Vec<TestReference>,
    //     expected_status: ResponseStatusCode,
    // ) -> Result<TestReference, HolonError> {

    // self.steps.push(DanceTestStep::AddRelatedHolons {
    //     source_token: source_token.clone(),
    //     relationship_name,
    //     holons_to_add,
    //     expected_status,
    // });

    // // Cloning source in order to create a new fixture holon
    // let mut expected_content = source_token.token_id().clone_holon(context)?;
    // // Update expected
    // expected_content.add_related_holons(context, relationship_name, holons_to_add)?;
    // // Mint next
    // let source_token = fixture_holons.add_token(
    //     source_token.root(),
    //     source_token.state(),
    //     expected_content,
    // )?;

    //

    //     Ok(())
    // }

    // Advance head (no new logical holon).
    pub fn add_remove_properties_step(
        &mut self,
        context: &dyn HolonsContextBehavior,
        fixture_holons: &mut FixtureHolons,
        source_token: TestReference,
        properties: PropertyMap,
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        // Cloning new source to create the expected snapshot
        let new_source = fixture_holons.derive_next_source(&source_token)?;
        let mut new_snapshot = new_source.snapshot().clone_holon(context)?;
        for property in properties.keys() {
            new_snapshot.remove_property_value(context, property)?;
        }
        let expected = ExpectedSnapshot::new(new_snapshot, new_source.state());
        if expected_status == ResponseStatusCode::OK {
            // Advance head snapshot for the FixtureHolon
            fixture_holons.advance_head(&source_token.expected_id(), expected.clone())?;
        }
        // Mint
        let new_token = fixture_holons.mint_test_reference(new_source, expected);

        // Add execution step
        self.steps.push(DanceTestStep::RemoveProperties {
            source_token: new_token.clone(),
            properties: properties.clone(),
            expected_status,
        });

        Ok(new_token)
    }

    // TODO: Support for relationships to be finished in issue 382

    // Advance head (no new logical holon).
    // pub fn add_remove_related_holons_step(
    //     &mut self,
    //     source_token: TestReference, // "owning" source Holon, which owns the Relationship
    //     relationship_name: RelationshipName,
    //     holons_to_remove: Vec<TestReference>,
    //     expected_status: ResponseStatusCode,
    // ) -> Result<TestReference, HolonError> {
    // self.steps.push(DanceTestStep::RemoveRelatedHolons {
    //     source_token,
    //     expected_token: expected_token.clone(),
    //     relationship_name,
    //     holons_to_remove,
    //     expected_status,
    // });

    // // Cloning source in order to create a new fixture holon
    // let mut expected_content = source_token.token_id().clone_holon(context)?;
    // // Update expected
    // expected_content.remove_related_holons(context, relationship_name, holons_to_remove)?;
    // // Mint next
    // let source_token = fixture_holons.add_token(
    //     source_token.root(),
    //     source_token.state(),
    //     expected_content,
    // )?;

    // Ok(source_token)

    //     Ok(())
    // }

    // Creates new logical holon and therefore a new FixtureHolon.
    pub fn add_stage_holon_step(
        &mut self,
        context: &dyn HolonsContextBehavior,
        fixture_holons: &mut FixtureHolons,
        source_token: TestReference,
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        // Cloning new source to create the expected snapshot
        let new_source = fixture_holons.derive_next_source(&source_token)?;
        let snapshot = new_source.snapshot().clone_holon(context)?;
        let expected = ExpectedSnapshot::new(snapshot, TestHolonState::Staged);
        if expected_status == ResponseStatusCode::OK {
            // Create new FixtureHolon
            fixture_holons.create_fixture_holon(expected.clone())?;
        }
        // Mint
        let new_token = fixture_holons.mint_test_reference(new_source, expected);

        // Add execution step
        self.steps
            .push(DanceTestStep::StageHolon { source_token: new_token.clone(), expected_status });

        Ok(new_token)
    }

    // Creates new logical holon and therefore a new FixtureHolon.
    pub fn add_stage_new_from_clone_step(
        &mut self,
        context: &dyn HolonsContextBehavior,
        fixture_holons: &mut FixtureHolons,
        source_token: TestReference,
        new_key: MapString, // Passing the key is necessary for the dance  // TODO: Future changes will make this an Option
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        // Cloning new source to create the expected snapshot
        let new_source = fixture_holons.derive_next_source(&source_token)?;
        let mut snapshot = new_source.snapshot().clone_holon(context)?;
        snapshot.with_property_value(context, "Key", new_key.clone())?;
        let expected = ExpectedSnapshot::new(snapshot, TestHolonState::Staged);
        if expected_status == ResponseStatusCode::OK {
            // Create new FixtureHolon
            fixture_holons.create_fixture_holon(expected.clone())?;
        }
        // Mint
        let new_token = fixture_holons.mint_test_reference(new_source, expected);

        // Add execution step
        self.steps.push(DanceTestStep::StageNewFromClone {
            source_token: new_token.clone(),
            new_key: new_key.clone(),
            expected_status: expected_status.clone(),
        });

        Ok(new_token)
    }

    // Creates new logical holon and therefore a new FixtureHolon.
    pub fn add_stage_new_version_step(
        &mut self,
        context: &dyn HolonsContextBehavior,
        fixture_holons: &mut FixtureHolons,
        source_token: TestReference,
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        // Cloning new source to create the expected snapshot
        let new_source = fixture_holons.derive_next_source(&source_token)?;
        let snapshot = new_source.snapshot().clone_holon(context)?;
        let expected = ExpectedSnapshot::new(snapshot, TestHolonState::Staged);
        if expected_status == ResponseStatusCode::OK {
            // Create new FixtureHolon
            fixture_holons.create_fixture_holon(expected.clone())?;
        }
        // Mint
        let new_token = fixture_holons.mint_test_reference(new_source, expected);

        // Add execution step
        self.steps.push(DanceTestStep::StageNewVersion {
            source_token: new_token.clone(),
            expected_status: expected_status.clone(),
        });

        Ok(new_token)
    }

    // Advance head (no new logical holon).
    pub fn add_with_properties_step(
        &mut self,
        context: &dyn HolonsContextBehavior,
        fixture_holons: &mut FixtureHolons,
        source_token: TestReference,
        properties: PropertyMap,
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        // Cloning new source to create the expected snapshot
        let new_source = fixture_holons.derive_next_source(&source_token)?;
        let mut new_snapshot = new_source.snapshot().clone_holon(context)?;
        for (property, value) in properties.clone() {
            new_snapshot.with_property_value(context, property, value)?;
        }
        let expected = ExpectedSnapshot::new(new_snapshot, new_source.state());
        // Advance head snapshot for the FixtureHolon
        fixture_holons.advance_head(&source_token.expected_id(), expected.clone())?;
        // Mint
        let new_token = fixture_holons.mint_test_reference(new_source, expected);

        // Add execution step
        self.steps.push(DanceTestStep::WithProperties {
            source_token: new_token.clone(),
            properties: properties.clone(),
            expected_status,
        });

        Ok(new_token)
    }
}

/// Internal step representation used by executors at runtime.
#[derive(Clone, Debug)]
pub enum DanceTestStep {
    AbandonStagedChanges {
        source_token: TestReference,
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
        source_token: TestReference,
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
    },
    PrintDatabase,
    QueryRelationships {
        source_token: TestReference,
        query_expression: QueryExpression,
        expected_status: ResponseStatusCode,
    },
    RemoveProperties {
        source_token: TestReference,
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
        source_token: TestReference,
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
        source_token: TestReference,
        properties: PropertyMap,
        expected_status: ResponseStatusCode,
    },
}

impl core::fmt::Display for DanceTestStep {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DanceTestStep::AbandonStagedChanges { source_token, expected_status } => {
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
            } => {
                write!(f, "AddRelatedHolons to Holon {:#?} for relationship: {:#?}, added_count: {:#?}, expecting: {:#?}", source_token, relationship_name, holons_to_add.len(), expected_status)
            }
            DanceTestStep::Commit { saved_tokens, expected_status } => {
                write!(f, "Committing {:#?}, expecting: {:?})", saved_tokens, expected_status)
            }
            DanceTestStep::DeleteHolon { source_token, expected_status } => {
                write!(f, "DeleteHolon({:?}, expecting: {:?},)", source_token, expected_status)
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
            } => {
                write!(f, "QueryRelationships for source:{:#?}, with query expression: {:#?}, expecting {:#?}", source_token, query_expression, expected_status)
            }
            DanceTestStep::RemoveProperties { source_token, properties, expected_status } => {
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
            } => {
                write!(f, "RemoveRelatedHolons from Holon {:#?} for relationship: {:#?}, added_count: {:#?}, expecting: {:#?}", source_token, relationship_name, holons_to_remove.len(), expected_status)
            }
            DanceTestStep::StageHolon { source_token, expected_status } => {
                write!(f, "StageHolon({:?}, expecting: {:?},)", source_token, expected_status)
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
            DanceTestStep::WithProperties { source_token, properties, expected_status } => {
                write!(
                    f,
                    "WithProperties for Holon {:#?} with properties: {:#?}, expecting {:#?} ",
                    source_token, properties, expected_status,
                )
            }
        }
    }
}
