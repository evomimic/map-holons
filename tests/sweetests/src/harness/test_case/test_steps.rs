//!
//! - [`DanceTestStep`], a closed vocabulary of test operations, each
//!   corresponding to one or more MAP dances or assertions.

use crate::harness::fixtures_support::TestReference;
use core_types::{CommitValidationViolationKind, TemporaryId};
use holons_core::core_shared_objects::holon::ValidationState;
use holons_prelude::prelude::*;
use integrity_core_types::HolonErrorKind;

/// Expected `LoadCommitStatus` written by the loader controller onto the
/// load response holon. Display values match the on-holon property strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpectedLoadStatus {
    Complete,
    Incomplete,
    Rejected,
    Skipped,
}

impl core::fmt::Display for ExpectedLoadStatus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let value = match self {
            ExpectedLoadStatus::Complete => "Complete",
            ExpectedLoadStatus::Incomplete => "Incomplete",
            ExpectedLoadStatus::Rejected => "Rejected",
            ExpectedLoadStatus::Skipped => "Skipped",
        };
        write!(f, "{value}")
    }
}

/// Expected `CommitRequestStatus` on the commit response holon. An
/// `Incomplete` commit is an `Ok` response with operational persistence errors;
/// partial writes may have occurred. `Rejected` is an `Ok` semantic refusal with no writes.
/// Display values match the on-holon property strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpectedCommitStatus {
    Complete,
    Incomplete,
    Rejected,
}

impl core::fmt::Display for ExpectedCommitStatus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let value = match self {
            ExpectedCommitStatus::Complete => "Complete",
            ExpectedCommitStatus::Incomplete => "Incomplete",
            ExpectedCommitStatus::Rejected => "Rejected",
        };
        write!(f, "{value}")
    }
}

/// Identity-only subject shape; the executor supplies the realized holon's identity.
#[derive(Clone, Debug)]
pub enum ExpectedValidationSubject {
    Holon,
    Property(String),
    Value(String),
}

/// Stable finding expectations deliberately omit diagnostic message text.
#[derive(Clone, Debug)]
pub struct ExpectedValidationFinding {
    pub kind: CommitValidationViolationKind,
    pub rule_key: Option<String>,
    pub subject: ExpectedValidationSubject,
}

/// A rejected fixture token remains staged and exposes findings from the wire round trip.
#[derive(Clone, Debug)]
pub struct ExpectedRejectedHolon {
    pub token: TestReference,
    pub validation_state: ValidationState,
    pub findings: Vec<ExpectedValidationFinding>,
}

/// Internal step representation used by executors at runtime.
#[derive(Clone, Debug)]
pub enum DanceTestStep {
    AbandonStagedChanges {
        step_token: TestReference,
        expected_error: Option<HolonErrorKind>,
        description: String,
    },
    AddRelatedHolons {
        step_token: TestReference,
        relationship_name: RelationshipName,
        holons_to_add: Vec<TestReference>,
        expected_error: Option<HolonErrorKind>,
        description: String,
    },
    BeginTransaction {
        expected_error: Option<HolonErrorKind>,
        description: String,
    },
    Commit {
        saved_tokens: Vec<TestReference>, // Used to match expected
        expected_status: ExpectedCommitStatus,
        expected_error: Option<HolonErrorKind>,
        description: String,
    },
    VerifyCommitRejection {
        rejected_holons: Vec<ExpectedRejectedHolon>,
        expected_violation_count: MapInteger,
        description: String,
    },
    DeleteHolon {
        step_token: TestReference,
        expected_error: Option<HolonErrorKind>,
        description: String,
    },
    EnsureDatabaseCount {
        expected_count: MapInteger,
        description: String,
    },
    LoadHolonsInternal {
        set_id: TemporaryId,
        expect_staged: MapInteger,
        expect_committed: MapInteger,
        expect_links_created: MapInteger,
        expect_errors: MapInteger,
        expect_total_bundles: MapInteger,
        expect_total_loader_holons: MapInteger,
        expect_status: ExpectedLoadStatus,
        expect_validation_violation_count: Option<MapInteger>,
    },
    LookupSavedHolonByKey {
        step_token: TestReference,
        key: MapString,
        expected_error: Option<HolonErrorKind>,
        description: String,
    },
    LoadCoreSchema {
        description: String,
    },
    LoadGeneratedCoreSchema {
        description: String,
    },
    LoadGeneratedDanceSchema {
        description: String,
    },
    LoadGeneratedCommandsSchema {
        description: String,
    },
    LoadGeneratedValidationSchema {
        description: String,
    },
    LoadGeneratedQuerySchema {
        description: String,
    },
    LoadGeneratedQueryDanceSchema {
        description: String,
    },
    LoadBookPersonInverseTestSchema {
        description: String,
    },
    LoadInverseOrientedBookPersonInstancesExpectFailure {
        description: String,
    },
    VerifyBookPersonDescriptors {
        description: String,
    },
    VerifyBookPersonInstanceLinks {
        description: String,
    },
    VerifyBookPersonSmartLinkCommitCacheLinks {
        description: String,
    },
    VerifyRelationshipAnchoring {
        description: String,
    },
    VerifyCoreSchemaDescriptorSubtypes {
        description: String,
    },
    VerifyCoreSchemaDescriptors {
        description: String,
    },
    VerifyCoreSchemaCommandAffordances {
        description: String,
    },
    VerifyCoreSchemaValueSemantics {
        description: String,
    },
    VerifyValidationBindingsDescriptorContract {
        description: String,
    },
    VerifySchemaValidationConformance {
        description: String,
    },
    MatchSavedContent,
    NewHolon {
        step_token: TestReference,
        properties: PropertyMap,
        key: Option<MapString>,
        expected_error: Option<HolonErrorKind>,
        description: String,
    },
    PrintDatabase,
    QueryRelationships {
        step_token: TestReference,
        query_expression: QueryExpression,
        expected_error: Option<HolonErrorKind>,
        description: String,
    },
    RemoveProperties {
        step_token: TestReference,
        properties: PropertyMap,
        expected_error: Option<HolonErrorKind>,
        description: String,
    },
    RemoveRelatedHolons {
        step_token: TestReference,
        relationship_name: RelationshipName,
        holons_to_remove: Vec<TestReference>,
        expected_error: Option<HolonErrorKind>,
        description: String,
    },
    StageHolon {
        step_token: TestReference,
        expected_error: Option<HolonErrorKind>,
        description: String,
    },
    StageNewFromClone {
        step_token: TestReference,
        new_key: MapString,
        expected_error: Option<HolonErrorKind>,
        description: String,
    },
    StageNewVersion {
        step_token: TestReference,
        expected_error: Option<HolonErrorKind>,
        version_count: MapInteger,
        expected_staging_error: Option<HolonErrorKind>,
        description: String,
    },
    WithProperties {
        step_token: TestReference,
        properties: PropertyMap,
        expected_error: Option<HolonErrorKind>,
        description: String,
    },
}

impl core::fmt::Display for DanceTestStep {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DanceTestStep::BeginTransaction { expected_error, description } => {
                write!(f, "{description} [expected_error: {expected_error:?}]")
            }
            DanceTestStep::AbandonStagedChanges { step_token, expected_error, description } => {
                write!(f, "{description} [token: {step_token}, expected_error: {expected_error:?}]")
            }
            DanceTestStep::AddRelatedHolons {
                step_token,
                relationship_name,
                holons_to_add,
                expected_error,
                description,
            } => {
                write!(
                    f,
                    "{description} [token: {step_token}, relationship: {relationship_name}, targets: {}, expected_error: {expected_error:?}]",
                    holons_to_add.len()
                )
            }
            DanceTestStep::Commit {
                saved_tokens,
                expected_status,
                expected_error,
                description,
            } => {
                write!(
                    f,
                    "{description} [saved_tokens: {}, expected_status: {expected_status}, expected_error: {expected_error:?}]",
                    saved_tokens.len()
                )
            }
            DanceTestStep::DeleteHolon { step_token, expected_error, description } => {
                write!(f, "{description} [token: {step_token}, expected_error: {expected_error:?}]")
            }
            DanceTestStep::VerifyCommitRejection {
                rejected_holons,
                expected_violation_count,
                description,
            } => {
                write!(
                    f,
                    "{description} [rejected_holons: {}, violations: {}]",
                    rejected_holons.len(),
                    expected_violation_count.0
                )
            }
            DanceTestStep::EnsureDatabaseCount { expected_count, description } => {
                write!(f, "{description} [expected_count: {}]", expected_count.0)
            }
            DanceTestStep::LoadHolonsInternal {
                set_id: _,
                expect_staged,
                expect_committed,
                expect_links_created,
                expect_errors,
                expect_total_bundles,
                expect_total_loader_holons,
                expect_status,
                expect_validation_violation_count,
            } => {
                write!(
                    f,
                    "LoadHolonsInternal(staged={}, committed={}, links_created={}, errors={}, bundles={}, loader_holons={}, status={}, violations={expect_validation_violation_count:?})",
                    expect_staged.0, expect_committed.0, expect_links_created.0, expect_errors.0, expect_total_bundles.0, expect_total_loader_holons.0, expect_status
                )
            }
            DanceTestStep::LookupSavedHolonByKey {
                step_token,
                key,
                expected_error,
                description,
            } => {
                write!(
                    f,
                    "{description} [token: {step_token}, key: {key}, expected_error: {expected_error:?}]"
                )
            }
            DanceTestStep::LoadCoreSchema { description } => {
                write!(f, "{description}")
            }
            DanceTestStep::LoadGeneratedCoreSchema { description } => {
                write!(f, "{description}")
            }
            DanceTestStep::LoadGeneratedDanceSchema { description } => {
                write!(f, "{description}")
            }
            DanceTestStep::LoadGeneratedCommandsSchema { description } => {
                write!(f, "{description}")
            }
            DanceTestStep::LoadGeneratedValidationSchema { description } => {
                write!(f, "{description}")
            }
            DanceTestStep::LoadGeneratedQuerySchema { description } => {
                write!(f, "{description}")
            }
            DanceTestStep::LoadGeneratedQueryDanceSchema { description } => {
                write!(f, "{description}")
            }
            DanceTestStep::LoadBookPersonInverseTestSchema { description } => {
                write!(f, "{description}")
            }
            DanceTestStep::LoadInverseOrientedBookPersonInstancesExpectFailure { description } => {
                write!(f, "{description}")
            }
            DanceTestStep::VerifyBookPersonDescriptors { description } => {
                write!(f, "{description}")
            }
            DanceTestStep::VerifyBookPersonInstanceLinks { description } => {
                write!(f, "{description}")
            }
            DanceTestStep::VerifyBookPersonSmartLinkCommitCacheLinks { description } => {
                write!(f, "{description}")
            }
            DanceTestStep::VerifyRelationshipAnchoring { description } => {
                write!(f, "{description}")
            }
            DanceTestStep::VerifyCoreSchemaDescriptorSubtypes { description } => {
                write!(f, "{description}")
            }
            DanceTestStep::VerifyCoreSchemaDescriptors { description } => {
                write!(f, "{description}")
            }
            DanceTestStep::VerifyCoreSchemaCommandAffordances { description } => {
                write!(f, "{description}")
            }
            DanceTestStep::VerifyCoreSchemaValueSemantics { description } => {
                write!(f, "{description}")
            }
            DanceTestStep::VerifyValidationBindingsDescriptorContract { description } => {
                write!(f, "{description}")
            }
            DanceTestStep::VerifySchemaValidationConformance { description } => {
                write!(f, "{description}")
            }
            DanceTestStep::MatchSavedContent => {
                write!(f, "MatchSavedContent")
            }
            DanceTestStep::NewHolon {
                step_token,
                properties,
                key,
                expected_error,
                description,
            } => {
                write!(
                    f,
                    "{description} [token: {step_token}, properties: {}, key: {:?}, expected_error: {expected_error:?}]",
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
                expected_error,
                description,
            } => {
                write!(f, "{description} [token: {step_token}, expected_error: {expected_error:?}]")
            }
            DanceTestStep::RemoveProperties {
                step_token,
                properties,
                expected_error,
                description,
            } => {
                write!(
                    f,
                    "{description} [token: {step_token}, properties: {}, expected_error: {expected_error:?}]",
                    properties.len()
                )
            }
            DanceTestStep::RemoveRelatedHolons {
                step_token,
                relationship_name,
                holons_to_remove,
                expected_error,
                description,
            } => {
                write!(
                    f,
                    "{description} [token: {step_token}, relationship: {relationship_name}, targets: {}, expected_error: {expected_error:?}]",
                    holons_to_remove.len()
                )
            }
            DanceTestStep::StageHolon { step_token, expected_error, description } => {
                write!(f, "{description} [token: {step_token}, expected_error: {expected_error:?}]")
            }
            DanceTestStep::StageNewVersion {
                step_token,
                expected_error,
                version_count,
                expected_staging_error,
                description,
            } => {
                write!(
                    f,
                    "{description} [token: {step_token}, version_count: {}, expected_error: {expected_error:?}, expected_failure: {:?}]",
                    version_count.0,
                    expected_staging_error
                )
            }
            DanceTestStep::StageNewFromClone {
                step_token,
                new_key,
                expected_error,
                description,
            } => {
                write!(
                    f,
                    "{description} [token: {step_token}, new_key: {new_key}, expected_error: {expected_error:?}]"
                )
            }
            DanceTestStep::WithProperties {
                step_token,
                properties,
                expected_error,
                description,
            } => {
                write!(
                    f,
                    "{description} [token: {step_token}, properties: {}, expected_error: {expected_error:?}]",
                    properties.len()
                )
            }
        }
    }
}

// impl fmt::Debug for DanceTestStep {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         match self {
//             DanceTestStep::AbandonStagedChanges { step_token, expected_error, description } => f
//                 .debug_struct("AbandonStagedChanges")
//                 .field("description", description)
//                 .field("step_token", step_token)
//                 .field("expected_status", expected_status)
//                 .finish(),
//             DanceTestStep::AddRelatedHolons {
//                     step_token,
//                     relationship_name,
//                     holons_to_add,
//                     expected_error,
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
//             DanceTestStep::Commit { saved_tokens, expected_error, description } =>
//                 f.debug_struct("Commit")
//                 .field("description", description)
//                 .field("saved_tokens", saved_tokens)
//                 .field("expected_status", expected_status)
//                 .finish(),
//             DanceTestStep::DeleteHolon { step_token, expected_error, description } => f
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
//             DanceTestStep::LoadHolonsInternal {
//                 set: _,
//                 expect_staged,
//                 expect_committed,
//                 expect_links_created,
//                 expect_errors,
//                 expect_total_bundles,
//                 expect_total_loader_holons,
//             } => f
//                 .debug_struct("LoadHolonsInternal")
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
//             DanceTestStep::MatchSavedContent => f
//                 write!(f, "MatchSavedContent")
//             },
//             DanceTestStep::NewHolon {
//                 step_token,
//                 properties: _properties,
//                 key,
//                 expected_error,
//                 description,
//             },
//             DanceTestStep::PrintDatabase => f
//                 write!(f, "PrintDatabase")
//             },
//             DanceTestStep::QueryRelationships {
//                 step_token,
//                 query_expression,
//                 expected_error,
//                 description,
//             },
//             DanceTestStep::RemoveProperties {
//                 step_token,
//                 properties,
//                 expected_error,
//                 description,
//             },
//             DanceTestStep::RemoveRelatedHolons {
//                 step_token,
//                 relationship_name,
//                 holons_to_remove,
//                 expected_error,
//                 description,
//             },
//             DanceTestStep::StageHolon { step_token, expected_error, description },
//             DanceTestStep::StageNewVersion {
//                 step_token,
//                 expected_error,
//                 version_count: _version_count,
//                 expected_staging_error,
//                 description,
//             },
//             DanceTestStep::StageNewFromClone {
//                 step_token,
//                 new_key,
//                 expected_error,
//                 description,
//             },
//             DanceTestStep::WithProperties {
//                 step_token,
//                 properties,
//                 expected_error,
//                 description,
//             },
//         }
//     }
// }
