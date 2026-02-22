//!
//! - [`DanceTestStep`], a closed vocabulary of test operations, each
//!   corresponding to one or more MAP dances or assertions.

use crate::harness::fixtures_support::TestReference;
use core_types::ContentSet;
use holons_prelude::prelude::*;

/// Internal step representation used by executors at runtime.
#[derive(Clone, Debug)]
pub enum DanceTestStep {
    AbandonStagedChanges {
        step_token: TestReference,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    },
    AddRelatedHolons {
        step_token: TestReference,
        relationship_name: RelationshipName,
        step_tokens_to_add: Vec<TestReference>,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    },
    Commit {
        saved_tokens: Vec<TestReference>, // Used to match expected
        expected_status: ResponseStatusCode,
        description: Option<String>,
    },
    DeleteHolon {
        step_token: TestReference,
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
        step_token: TestReference,
        properties: PropertyMap,
        key: Option<MapString>,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    },
    PrintDatabase,
    QueryRelationships {
        step_token: TestReference,
        query_expression: QueryExpression,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    },
    RemoveProperties {
        step_token: TestReference,
        properties: PropertyMap,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    },
    RemoveRelatedHolons {
        step_token: TestReference,
        relationship_name: RelationshipName,
        step_tokens_to_remove: Vec<TestReference>,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    },
    StageHolon {
        step_token: TestReference,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    },
    StageNewFromClone {
        step_token: TestReference,
        new_key: MapString,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    },
    StageNewVersion {
        step_token: TestReference,
        expected_status: ResponseStatusCode,
        version_count: MapInteger,
        expected_failure_code: Option<ResponseStatusCode>,
        description: Option<String>,
    },
    WithProperties {
        step_token: TestReference,
        properties: PropertyMap,
        expected_status: ResponseStatusCode,
        description: Option<String>,
    },
}

impl core::fmt::Display for DanceTestStep {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DanceTestStep::AbandonStagedChanges {
                step_token,
                expected_status,
                description: _description,
            } => {
                write!(
                    f,
                    "Marking Holon at ({:?}) as Abandoned, expecting ({:?})",
                    step_token, expected_status
                )
            }
            DanceTestStep::AddRelatedHolons {
                step_token,
                relationship_name,
                step_tokens_to_add,
                expected_status,
                description: _description,
            } => {
                write!(f, "AddRelatedHolons to Holon {:#?} for relationship: {:#?}, added_count: {:#?}, expecting: {:#?}", step_token, relationship_name, step_tokens_to_add.len(), expected_status)
            }
            DanceTestStep::Commit { saved_tokens, expected_status, description: _description } => {
                write!(f, "Committing {:#?}, expecting: {:?}", saved_tokens, expected_status)
            }
            DanceTestStep::DeleteHolon {
                step_token,
                expected_status,
                description: _description,
            } => {
                write!(f, "DeleteHolon({:?}, expecting: {:?},)", step_token, expected_status)
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
                step_token,
                properties: _properties,
                key,
                expected_status,
                description: _description,
            } => {
                write!(
                    f,
                    "NewHolon({:?}, with key: {:?}, expecting: {:?},)",
                    step_token, key, expected_status
                )
            }
            DanceTestStep::PrintDatabase => {
                write!(f, "PrintDatabase")
            }
            DanceTestStep::QueryRelationships {
                step_token,
                query_expression,
                expected_status,
                description: _description,
            } => {
                write!(f, "QueryRelationships for source:{:#?}, with query expression: {:#?}, expecting {:#?}", step_token, query_expression, expected_status)
            }
            DanceTestStep::RemoveProperties {
                step_token,
                properties,
                expected_status,
                description: _description,
            } => {
                write!(
                    f,
                    "RemoveProperties {:#?} for Holon {:#?}, expecting {:#?} ",
                    properties, step_token, expected_status,
                )
            }
            DanceTestStep::RemoveRelatedHolons {
                step_token,
                relationship_name,
                step_tokens_to_remove,
                expected_status,
                description: _description,
            } => {
                write!(f, "RemoveRelatedHolons from Holon {:#?} for relationship: {:#?}, removed_count: {:#?}, expecting: {:#?}", step_token, relationship_name, step_tokens_to_remove.len(), expected_status)
            }
            DanceTestStep::StageHolon {
                step_token,
                expected_status,
                description: _description,
            } => {
                write!(f, "StageHolon({:?}, expecting: {:?},)", step_token, expected_status)
            }
            DanceTestStep::StageNewVersion {
                step_token,
                expected_status,
                version_count: _version_count,
                expected_failure_code,
                description: _description,
            } => {
                write!(
                    f,
                    "NewVersion for source: {:#?}, expecting response: {:#?}, optional failure_code: {:?}",
                    step_token, expected_status, expected_failure_code,
                )
            }
            DanceTestStep::StageNewFromClone {
                step_token,
                new_key,
                expected_status,
                description: _description,
            } => {
                write!(
                    f,
                    "StageNewFromClone for original_holon: {:#?}, with new key: {:?}, expecting response: {:#?}",
                    step_token, new_key, expected_status
                )
            }
            DanceTestStep::WithProperties {
                step_token,
                properties,
                expected_status,
                description: _description,
            } => {
                write!(
                    f,
                    "WithProperties for Holon {:#?} with properties: {:#?}, expecting {:#?} ",
                    step_token, properties, expected_status
                )
            }
        }
    }
}
