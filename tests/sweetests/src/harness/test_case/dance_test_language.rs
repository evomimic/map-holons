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
//! remaining independent of execution order, runtime identifiers, and
//! persistence details.

use crate::{harness::fixtures_support::TestReference, FixtureHolons};
use core_types::ContentSet;
use holons_core::{
    core_shared_objects::holon_pool::SerializableHolonPool, reference_layer::ReadableHolon,
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

    // === Exectution Steps with === //
    // ==== Token Minting ==== //

    pub fn add_abandon_staged_changes_step(
        &mut self,
        context: &dyn HolonsContextBehavior,
        fixture_holons: &mut FixtureHolons,
        source_token: TestReference,
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        let expected_content = source_token.token_id().clone_holon(context)?;

        let abandoned_token = fixture_holons.abandon_staged(&source_token, expected_content)?;

        self.steps.push(DanceTestStep::AbandonStagedChanges {
            source_token,
            expected_token: abandoned_token.clone(),
            expected_status,
        });

        Ok(abandoned_token)
    }

    pub fn add_delete_holon_step(
        &mut self,
        context: &dyn HolonsContextBehavior,
        fixture_holons: &mut FixtureHolons,
        source_token: TestReference,
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        let expected_content = source_token.token_id().clone_holon(context)?;

        let deleted_token = fixture_holons.delete_saved(&source_token, expected_content)?;

        self.steps.push(DanceTestStep::DeleteHolon {
            source_token,
            expected_token: deleted_token.clone(),
            expected_status,
        });

        Ok(deleted_token)
    }

    pub fn add_commit_step(
        &mut self,
        context: &dyn HolonsContextBehavior,
        fixture_holons: &mut FixtureHolons,
        expected_status: ResponseStatusCode,
    ) -> Result<Vec<TestReference>, HolonError> {
        let saved_tokens = fixture_holons.commit(context)?;
        self.steps
            .push(DanceTestStep::Commit { saved_tokens: saved_tokens.clone(), expected_status });

        Ok(saved_tokens)
    }

    pub fn add_new_holon_step(
        &mut self,
        context: &dyn HolonsContextBehavior,
        fixture_holons: &mut FixtureHolons,
        source_reference: TransientReference,
        properties: PropertyMap,
        key: Option<MapString>,
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        let mut expected_content = source_reference.clone_holon(context)?;
        for (name, value) in properties.clone() {
            expected_content.with_property_value(context, name, value)?;
        }
        let source_token = fixture_holons.add_transient(source_reference.clone(), expected_content);

        self.steps.push(DanceTestStep::NewHolon {
            source_token: source_token.clone(),
            properties,
            key,
            expected_status,
        });

        Ok(source_token)
    }

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
    //     source_token.intended_resolved_state(),
    //     expected_content,
    // )?;

    //

    //     Ok(())
    // }

    pub fn add_remove_properties_step(
        &mut self,
        context: &dyn HolonsContextBehavior,
        fixture_holons: &mut FixtureHolons,
        source_token: TestReference,
        properties: PropertyMap,
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        // Cloning source in order to create a new fixture holon
        let mut expected_content = source_token.token_id().clone_holon(context)?;
        // Update expected
        for (property, _value) in properties.clone() {
            expected_content.remove_property_value(context, property)?;
        }
        // Mint next
        let expected_token = fixture_holons.add_token(
            source_token.token_id().clone(),
            source_token.intended_resolved_state(),
            expected_content,
        )?;

        self.steps.push(DanceTestStep::RemoveProperties {
            source_token,
            expected_token: expected_token.clone(),
            properties: properties.clone(),
            expected_status,
        });

        Ok(expected_token)
    }

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
    //     source_token.intended_resolved_state(),
    //     expected_content,
    // )?;

    // Ok(source_token)

    //     Ok(())
    // }

    pub fn add_stage_holon_step(
        &mut self,
        context: &dyn HolonsContextBehavior,
        fixture_holons: &mut FixtureHolons,
        source_token: TestReference,
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        // Cloning source in order to create a new fixture holon
        let expected_content = source_token.token_id().clone_holon(context)?;
        // Mint a staged-intent token
        let staged_token =
            fixture_holons.add_staged(source_token.token_id().clone(), expected_content);

        self.steps.push(DanceTestStep::StageHolon {
            source_token,
            expected_token: staged_token.clone(),
            expected_status,
        });

        Ok(staged_token)
    }

    pub fn add_stage_new_from_clone_step(
        &mut self,
        context: &dyn HolonsContextBehavior,
        fixture_holons: &mut FixtureHolons,
        source_token: TestReference,
        new_key: MapString, // Passing the key is necessary for the dance  // TODO: Future changes will make this an Option
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        // Cloning source in order to create a new fixture holon
        let mut expected_content = source_token.token_id().clone_holon(context)?;
        expected_content.with_property_value(context, "Key", new_key.clone())?;
        // Mint a staged-intent token, with no back pointer
        let staged_token = fixture_holons.add_staged(expected_content.clone(), expected_content);

        self.steps.push(DanceTestStep::StageNewFromClone {
            source_token,
            expected_token: staged_token.clone(),
            new_key: new_key.clone(),
            expected_status: expected_status.clone(),
        });
        // Remove the minted token from FixtureHolons if the dance was meant to fail.
        if expected_status != ResponseStatusCode::OK {
            fixture_holons.remove_last();
        }

        Ok(staged_token)
    }

    pub fn add_stage_new_version_step(
        &mut self,
        context: &dyn HolonsContextBehavior,
        fixture_holons: &mut FixtureHolons,
        source_token: TestReference,
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        // Cloning source in order to create a new fixture holon, and a new root
        let expected_content = source_token.token_id().clone_holon(context)?;
        // Mint a staged-intent token
        let staged_token =
            fixture_holons.add_staged(source_token.token_id().clone(), expected_content);

        self.steps.push(DanceTestStep::StageNewVersion {
            source_token,                         // lookup
            expected_token: staged_token.clone(), // matching expected and recording resolved
            expected_status,
        });

        Ok(staged_token)
    }

    pub fn add_with_properties_step(
        &mut self,
        context: &dyn HolonsContextBehavior,
        fixture_holons: &mut FixtureHolons,
        source_token: TestReference,
        properties: PropertyMap,
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        // Cloning source in order to create a new fixture holon
        let mut expected_content = source_token.token_id().clone_holon(context)?;
        // Update expected
        for (property, value) in properties.clone() {
            expected_content.with_property_value(context, property, value)?;
        }
        // Mint next
        let expected_token = fixture_holons.add_token(
            source_token.token_id().clone(),
            source_token.intended_resolved_state(),
            expected_content,
        )?;

        self.steps.push(DanceTestStep::WithProperties {
            source_token,
            expected_token: expected_token.clone(),
            properties: properties.clone(),
            expected_status,
        });

        Ok(expected_token)
    }
}

/// Internal step representation used by executors at runtime.
#[derive(Clone, Debug)]
pub enum DanceTestStep {
    AbandonStagedChanges {
        source_token: TestReference,
        expected_token: TestReference,
        expected_status: ResponseStatusCode,
    },
    AddRelatedHolons {
        source_token: TestReference,
        expected_token: TestReference,
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
        expected_token: TestReference,
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
        expected_token: TestReference,
        properties: PropertyMap,
        expected_status: ResponseStatusCode,
    },
    RemoveRelatedHolons {
        source_token: TestReference,
        expected_token: TestReference,
        relationship_name: RelationshipName,
        holons_to_remove: Vec<TestReference>,
        expected_status: ResponseStatusCode,
    },
    StageHolon {
        source_token: TestReference,
        expected_token: TestReference,
        expected_status: ResponseStatusCode,
    },
    StageNewFromClone {
        source_token: TestReference,
        expected_token: TestReference,
        new_key: MapString,
        expected_status: ResponseStatusCode,
    },
    StageNewVersion {
        source_token: TestReference,
        expected_token: TestReference,
        expected_status: ResponseStatusCode,
    },
    WithProperties {
        source_token: TestReference,
        expected_token: TestReference,
        properties: PropertyMap,
        expected_status: ResponseStatusCode,
    },
}

impl core::fmt::Display for DanceTestStep {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DanceTestStep::AbandonStagedChanges {
                source_token,
                expected_token: _expected_token,
                expected_status,
            } => {
                write!(
                    f,
                    "Marking Holon at ({:?}) as Abandoned, expecting ({:?})",
                    source_token, expected_status
                )
            }
            DanceTestStep::AddRelatedHolons {
                source_token,
                expected_token: _expected_token,
                relationship_name,
                holons_to_add,
                expected_status,
            } => {
                write!(f, "AddRelatedHolons to Holon {:#?} for relationship: {:#?}, added_count: {:#?}, expecting: {:#?}", source_token, relationship_name, holons_to_add.len(), expected_status)
            }
            DanceTestStep::Commit { saved_tokens, expected_status } => {
                write!(f, "Committing {:#?}, expecting: {:?})", saved_tokens, expected_status)
            }
            DanceTestStep::DeleteHolon {
                source_token,
                expected_token: _expected_token,
                expected_status,
            } => {
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
            DanceTestStep::RemoveProperties {
                source_token,
                expected_token: _expected_token,
                properties,
                expected_status,
            } => {
                write!(
                    f,
                    "RemoveProperties {:#?} for Holon {:#?}, expecting {:#?} ",
                    properties, source_token, expected_status,
                )
            }
            DanceTestStep::RemoveRelatedHolons {
                source_token,
                expected_token: _expected_token,
                relationship_name,
                holons_to_remove,
                expected_status,
            } => {
                write!(f, "RemoveRelatedHolons from Holon {:#?} for relationship: {:#?}, added_count: {:#?}, expecting: {:#?}", source_token, relationship_name, holons_to_remove.len(), expected_status)
            }
            DanceTestStep::StageHolon {
                source_token,
                expected_token: _expected_token,
                expected_status,
            } => {
                write!(f, "StageHolon({:?}, expecting: {:?},)", source_token, expected_status)
            }
            DanceTestStep::StageNewVersion {
                source_token,
                expected_token: _expected_token,
                expected_status,
            } => {
                write!(
                    f,
                    "NewVersion for source: {:#?}, expecting response: {:#?}",
                    source_token, expected_status
                )
            }
            DanceTestStep::StageNewFromClone {
                source_token,
                expected_token: _expected_token,
                new_key,
                expected_status,
            } => {
                write!(
                    f,
                    "StageNewFromClone for original_holon: {:#?}, with new key: {:?}, expecting response: {:#?}",
                    source_token, new_key, expected_status
                )
            }
            DanceTestStep::WithProperties {
                source_token,
                expected_token: _expected_token,
                properties,
                expected_status,
            } => {
                write!(
                    f,
                    "WithProperties for Holon {:#?} with properties: {:#?}, expecting {:#?} ",
                    source_token, properties, expected_status,
                )
            }
        }
    }
}
