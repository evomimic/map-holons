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
        description: String,
    },
    AddRelatedHolons {
        step_token: TestReference,
        relationship_name: RelationshipName,
        holons_to_add: Vec<TestReference>,
        expected_status: ResponseStatusCode,
        description: String,
    },
    Commit {
        saved_tokens: Vec<TestReference>, // Used to match expected
        expected_status: ResponseStatusCode,
        description: String,
    },
    DeleteHolon {
        step_token: TestReference,
        expected_status: ResponseStatusCode,
        description: String,
    },
    EnsureDatabaseCount {
        expected_count: MapInteger,
        description: String,
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
        description: String,
    },
    PrintDatabase,
    QueryRelationships {
        step_token: TestReference,
        query_expression: QueryExpression,
        expected_status: ResponseStatusCode,
        description: String,
    },
    RemoveProperties {
        step_token: TestReference,
        properties: PropertyMap,
        expected_status: ResponseStatusCode,
        description: String,
    },
    RemoveRelatedHolons {
        step_token: TestReference,
        relationship_name: RelationshipName,
        holons_to_remove: Vec<TestReference>,
        expected_status: ResponseStatusCode,
        description: String,
    },
    StageHolon {
        step_token: TestReference,
        expected_status: ResponseStatusCode,
        description: String,
    },
    StageNewFromClone {
        step_token: TestReference,
        new_key: MapString,
        expected_status: ResponseStatusCode,
        description: String,
    },
    StageNewVersion {
        step_token: TestReference,
        expected_status: ResponseStatusCode,
        version_count: MapInteger,
        expected_failure_code: Option<ResponseStatusCode>,
        description: String,
    },
    WithProperties {
        step_token: TestReference,
        properties: PropertyMap,
        expected_status: ResponseStatusCode,
        description: String,
    },
}

impl core::fmt::Display for DanceTestStep {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DanceTestStep::AbandonStagedChanges { step_token, expected_status, description } => {
                write!(
                    f,
                    "{description} [token: {step_token}, expected_status: {expected_status:?}]"
                )
            }
            DanceTestStep::AddRelatedHolons {
                step_token,
                relationship_name,
                holons_to_add,
                expected_status,
                description,
            } => {
                write!(
                    f,
                    "{description} [token: {step_token}, relationship: {relationship_name}, targets: {}, expected_status: {expected_status:?}]",
                    holons_to_add.len()
                )
            }
            DanceTestStep::Commit { saved_tokens, expected_status, description } => {
                write!(
                    f,
                    "{description} [saved_tokens: {}, expected_status: {expected_status:?}]",
                    saved_tokens.len()
                )
            }
            DanceTestStep::DeleteHolon { step_token, expected_status, description } => {
                write!(
                    f,
                    "{description} [token: {step_token}, expected_status: {expected_status:?}]"
                )
            }
            DanceTestStep::EnsureDatabaseCount { expected_count, description } => {
                write!(f, "{description} [expected_count: {}]", expected_count.0)
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
                properties,
                key,
                expected_status,
                description,
            } => {
                write!(
                    f,
                    "{description} [token: {step_token}, properties: {}, key: {:?}, expected_status: {expected_status:?}]",
                    properties.len(),
                    key
                )
            }
            DanceTestStep::PrintDatabase => {
                write!(f, "PrintDatabase")
            }
            DanceTestStep::QueryRelationships {
                step_token,
                query_expression: _query_expression,
                expected_status,
                description,
            } => {
                write!(
                    f,
                    "{description} [token: {step_token}, expected_status: {expected_status:?}]"
                )
            }
            DanceTestStep::RemoveProperties {
                step_token,
                properties,
                expected_status,
                description,
            } => {
                write!(
                    f,
                    "{description} [token: {step_token}, properties: {}, expected_status: {expected_status:?}]",
                    properties.len()
                )
            }
            DanceTestStep::RemoveRelatedHolons {
                step_token,
                relationship_name,
                holons_to_remove,
                expected_status,
                description,
            } => {
                write!(
                    f,
                    "{description} [token: {step_token}, relationship: {relationship_name}, targets: {}, expected_status: {expected_status:?}]",
                    holons_to_remove.len()
                )
            }
            DanceTestStep::StageHolon { step_token, expected_status, description } => {
                write!(
                    f,
                    "{description} [token: {step_token}, expected_status: {expected_status:?}]"
                )
            }
            DanceTestStep::StageNewVersion {
                step_token,
                expected_status,
                version_count,
                expected_failure_code,
                description,
            } => {
                write!(
                    f,
                    "{description} [token: {step_token}, version_count: {}, expected_status: {expected_status:?}, expected_failure: {:?}]",
                    version_count.0,
                    expected_failure_code
                )
            }
            DanceTestStep::StageNewFromClone {
                step_token,
                new_key,
                expected_status,
                description,
            } => {
                write!(
                    f,
                    "{description} [token: {step_token}, new_key: {new_key}, expected_status: {expected_status:?}]"
                )
            }
            DanceTestStep::WithProperties {
                step_token,
                properties,
                expected_status,
                description,
            } => {
                write!(
                    f,
                    "{description} [token: {step_token}, properties: {}, expected_status: {expected_status:?}]",
                    properties.len()
                )
            }
        }
    }
}

// impl fmt::Debug for DanceTestStep {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         match self {
//             DanceTestStep::AbandonStagedChanges { step_token, expected_status, description } => f
//                 .debug_struct("AbandonStagedChanges")
//                 .field("description", description)
//                 .field("step_token", step_token)
//                 .field("expected_status", expected_status)
//                 .finish(),
//             DanceTestStep::AddRelatedHolons {
//                     step_token,
//                     relationship_name,
//                     holons_to_add,
//                     expected_status,
//                     description,
//                 } => f
//                     .debug_struct("AddRelatedHolons")
//                     .field("description", description)
//                     .field("step_token", step_token)
//                     .field("relationship_name", relationship_name)
//                     .field("holons_to_add", holons_to_add)
//                     .field("expected_status", expected_status)
//                     .finish(),
//             },
//             DanceTestStep::Commit { saved_tokens, expected_status, description } =>
//                 f.debug_struct("Commit")
//                 .field("description", description)
//                 .field("saved_tokens", saved_tokens)
//                 .field("expected_status", expected_status)
//                 .finish(),
//             DanceTestStep::DeleteHolon { step_token, expected_status, description } => f
//                 .debug_struct("DeleteHolon")
//                 .field("description", description)
//                 .field("step_token", step_token)
//                 .field("expected_status", expected_status)
//                 .finish(),
//             },
//             DanceTestStep::EnsureDatabaseCount { expected_count, description } => f
//                 .debug_struct("EnsureDatabaseCount")
//                 .field("description", description)
//                 .field("expected_count", expected_count)
//                 .finish(),
//             },
//             DanceTestStep::LoadHolons {
//                 set: _,
//                 expect_staged,
//                 expect_committed,
//                 expect_links_created,
//                 expect_errors,
//                 expect_total_bundles,
//                 expect_total_loader_holons,
//             } => f
//                 .debug_struct("LoadHolons")
//                 .field("description", description)
//                 .field("set", set),
//                 .field("expect_staged", expect_staged)
//                 .field("expect_committed", expect_committed)
//                 .field("expect_links_created", expect_links_created)
//                 .field("expect_errors", expect_errors)
//                 .field("expect_total_bundles", expect_total_bundles)
//                 .field("expect_total_loader_holons", expect_total_loader_holons)
//                 .finish(),
//             },
//             DanceTestStep::LoadHolonsClient {
//                 expect_staged,
//                 expect_committed,
//                 expect_links_created,
//                 expect_errors,
//                 expect_total_bundles,
//                 expect_total_loader_holons,
//                 ..
//             },
//             DanceTestStep::MatchSavedContent => f
//                 write!(f, "MatchSavedContent")
//             },
//             DanceTestStep::NewHolon {
//                 step_token,
//                 properties: _properties,
//                 key,
//                 expected_status,
//                 description,
//             },
//             DanceTestStep::PrintDatabase => f
//                 write!(f, "PrintDatabase")
//             },
//             DanceTestStep::QueryRelationships {
//                 step_token,
//                 query_expression,
//                 expected_status,
//                 description,
//             },
//             DanceTestStep::RemoveProperties {
//                 step_token,
//                 properties,
//                 expected_status,
//                 description,
//             },
//             DanceTestStep::RemoveRelatedHolons {
//                 step_token,
//                 relationship_name,
//                 holons_to_remove,
//                 expected_status,
//                 description,
//             },
//             DanceTestStep::StageHolon { step_token, expected_status, description },
//             DanceTestStep::StageNewVersion {
//                 step_token,
//                 expected_status,
//                 version_count: _version_count,
//                 expected_failure_code,
//                 description,
//             },
//             DanceTestStep::StageNewFromClone {
//                 step_token,
//                 new_key,
//                 expected_status,
//                 description,
//             },
//             DanceTestStep::WithProperties {
//                 step_token,
//                 properties,
//                 expected_status,
//                 description,
//             },
//         }
//     }
// }
