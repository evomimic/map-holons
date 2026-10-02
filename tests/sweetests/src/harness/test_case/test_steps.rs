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

/// Persistence disposition declared for one live Pass 1 candidate, for one Commit attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpectedDisposition {
    /// Persist a new root, including an independent clone, without inherited lineage.
    NewRoot,
    /// Save nothing; retain the staged head under Incomplete, or bind the source under Complete.
    NoAction,
    /// Persist graph changes using the saved source's identity.
    GraphOnly,
    /// Persist a distinct version with its saved source as predecessor.
    NewVersion,
}

impl core::fmt::Display for ExpectedDisposition {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let value = match self {
            ExpectedDisposition::NewRoot => "NewRoot",
            ExpectedDisposition::NoAction => "NoAction",
            ExpectedDisposition::GraphOnly => "GraphOnly",
            ExpectedDisposition::NewVersion => "NewVersion",
        };
        write!(f, "{value}")
    }
}

/// A fixture author's declaration for one live staged candidate in a Commit attempt.
#[derive(Clone, Debug)]
pub struct ExpectedCommitCandidate {
    pub token: TestReference,
    pub disposition: ExpectedDisposition,
    /// Newly appended operational error occurrences; an empty list expects none.
    /// Repeated kinds represent separate occurrences.
    pub expected_new_errors: Vec<HolonErrorKind>,
}

impl ExpectedCommitCandidate {
    /// Declares a candidate's disposition with no new operational errors expected.
    pub fn new(token: TestReference, disposition: ExpectedDisposition) -> Self {
        Self { token, disposition, expected_new_errors: Vec::new() }
    }

    /// Sets the expected new operational error occurrences for this attempt.
    pub fn with_expected_new_errors(mut self, expected_new_errors: Vec<HolonErrorKind>) -> Self {
        self.expected_new_errors = expected_new_errors;
        self
    }
}

/// A committed staged entry retained for a Pass 2 relationship retry.
/// It has no live node disposition and produces no new `SavedHolons` entry.
#[derive(Clone, Debug)]
pub struct ExpectedRetryParticipant {
    pub token: TestReference,
    /// Newly appended operational error occurrences; an empty list expects none.
    /// Multiplicity follows [`ExpectedCommitCandidate::expected_new_errors`].
    pub expected_new_errors: Vec<HolonErrorKind>,
}

impl ExpectedRetryParticipant {
    /// Declares a relationship retry participant with no new operational errors expected.
    pub fn new(token: TestReference) -> Self {
        Self { token, expected_new_errors: Vec::new() }
    }

    /// Sets the expected new operational error occurrences for this attempt.
    pub fn with_expected_new_errors(mut self, expected_new_errors: Vec<HolonErrorKind>) -> Self {
        self.expected_new_errors = expected_new_errors;
        self
    }
}

/// Adder-resolved declaration associating author intent with a result token.
#[derive(Clone, Debug)]
pub struct ResolvedCommitCandidate {
    pub staged_token: TestReference,
    pub disposition: ExpectedDisposition,
    /// Advanced fixture-head token, absent only for `NoAction` under `Incomplete`.
    /// That candidate keeps its staged head and remains live for the next attempt.
    /// The Commit executor records the corresponding saved reference against this token;
    /// for `NoAction`, it constructs that reference from the candidate's `versioned_source_id`
    /// without consuming a `SavedHolons` entry.
    pub result_token: Option<TestReference>,
    /// Newly appended operational error occurrences; an empty list expects none.
    /// Multiplicity follows [`ExpectedCommitCandidate::expected_new_errors`].
    pub expected_new_errors: Vec<HolonErrorKind>,
}

/// Identity-only subject shape; the executor supplies the realized holon's identity.
#[derive(Clone, Debug)]
pub enum ExpectedValidationSubject {
    Holon,
    Property(String),
    Value(String),
    Relationship { name: String, target: TestReference },
    Transaction,
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

/// An aggregate finding whose Schema subject was not staged by the caller.
#[derive(Clone, Debug)]
pub struct ExpectedCommitCarrierFinding {
    pub schema_key: String,
    pub rule_code: String,
    pub rule_key: String,
    /// Staged rejections that accompany the carrier, including their findings.
    pub expected_rejected_holons: Vec<ExpectedRejectedHolon>,
}

/// How an `ExecuteQuery` step reaches the direct Query seam.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryRoute {
    /// Peer Rust call: `QueryReference::begin_execution` then `run`.
    Direct,
    /// Descriptor-bound `QueryDance` invocation through `TransactionAction::DanceV2`.
    QueryDance,
}

/// The collection operand an `ExecuteQuery` step supplies to the root expression.
#[derive(Clone, Debug)]
pub enum QueryInputSpec {
    /// No `Input` / `InitialInput` at all (a source root such as `SeedHolons`).
    None,
    /// An explicit `HolonCollection` holon whose members are these fixture references.
    Collection(Vec<TestReference>),
    /// One source holon through the direct single-holon convenience, which
    /// normalizes it into a transient singleton collection inside QueryCore.
    /// Direct route only: the Dance contract stays collection-shaped.
    SingleHolon(TestReference),
}

/// What an `ExecuteQuery` step asserts after the run.
#[derive(Clone, Debug)]
pub enum QueryExpectation {
    /// Success; the result members are exactly these fixture references, in order.
    Members(Vec<TestReference>),
    /// Success; the result members are the focal space's `Owns` targets in storage
    /// order and include every listed fixture reference.
    OwnsOfFocalSpace { must_include: Vec<TestReference> },
    /// The run (or `begin_execution`) fails with this error kind; no result is recorded.
    Error(HolonErrorKind),
}

/// A saved subject addressed by fixture identity, enumeration key, or lineage traversal.
#[derive(Clone, Debug)]
pub enum PersistedSubject {
    /// The adder freezes this token to its current saved head; execution uses its recorded identity.
    Token(TestReference),
    /// Requires exactly one enumerated holon with this key. Use tokens for same-key versions.
    Key(String),
    /// Traverses a unique Successor at each hop; zero denotes the source itself.
    /// Use tokens to identify branches when a hop has multiple successors.
    Successor { of: Box<PersistedSubject>, generation: usize },
}

/// Expected occurrence count of a particular target identity in a persisted relationship.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeExpectation {
    ExactlyOnce,
    Contains,
    Absent,
}

/// One forward edge and, optionally, the corresponding inverse edge.
#[derive(Clone, Debug)]
pub struct ExpectedPersistedEdge {
    pub source: PersistedSubject,
    pub relationship: RelationshipName,
    /// When present, applies the same expectation from target back to source.
    pub inverse: Option<RelationshipName>,
    pub target: PersistedSubject,
    pub expectation: EdgeExpectation,
}

/// Every persisted target of one relationship, as an exact unordered identity set.
///
/// Extra or missing targets fail, and duplicate persisted links fail because
/// multiplicity is preserved rather than normalized away. An empty `targets`
/// asserts an empty persisted collection.
///
/// Declare an inverse direction as its own entry with the target as `source`: an
/// inverse collection may legitimately hold sources from unrelated declarations, so a
/// complete inverse expectation cannot be derived from one forward declaration. Exact
/// inverses on schema descriptors are therefore only sound in an isolated runtime where
/// every source is known; in shared suites use [`ExpectedPersistedEdge`] for the inverse.
#[derive(Clone, Debug)]
pub struct ExpectedPersistedRelationship {
    pub source: PersistedSubject,
    pub relationship: RelationshipName,
    pub targets: Vec<PersistedSubject>,
}

/// Exact identities for the fixed `Predecessor` and `Successor` relationships;
/// empty lists assert absence of lineage.
/// Duplicate actual or declared members are errors, even when the identity sets agree.
///
/// Expands into two [`ExpectedPersistedRelationship`] assertions at execution time, so
/// lineage and ordinary relationships share one exactness comparator.
#[derive(Clone, Debug)]
pub struct ExpectedLineage {
    pub subject: PersistedSubject,
    pub predecessors: Vec<PersistedSubject>,
    pub successors: Vec<PersistedSubject>,
}

/// Persisted graph assertions evaluated with fresh reads rather than staged snapshots.
#[derive(Clone, Debug, Default)]
pub struct ExpectedPersistedGraph {
    /// Each subject must occur exactly once in get-all; unrelated holons are permitted.
    pub enumerated: Vec<PersistedSubject>,
    pub edges: Vec<ExpectedPersistedEdge>,
    /// Exact target sets; the only expectation that rejects an undeclared extra target.
    pub relationships: Vec<ExpectedPersistedRelationship>,
    pub lineage: Vec<ExpectedLineage>,
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
        candidates: Vec<ResolvedCommitCandidate>,
        retry_participants: Vec<ExpectedRetryParticipant>,
        expected_status: ExpectedCommitStatus,
        expected_error: Option<HolonErrorKind>,
        description: String,
    },
    VerifyCommitRejection {
        rejected_holons: Vec<ExpectedRejectedHolon>,
        expected_violation_count: MapInteger,
        description: String,
    },
    VerifyCommitCarrierFinding {
        expected: ExpectedCommitCarrierFinding,
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
    LoadQueryTestSchema {
        description: String,
    },
    ExecuteQuery {
        query: TestReference,
        input: QueryInputSpec,
        route: QueryRoute,
        /// Invocation-level parameter bindings: passed to `begin_execution` on
        /// the direct route and as `RequestParameters` on the QueryDance route.
        bindings: Vec<TestReference>,
        expectation: QueryExpectation,
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
    VerifyPersistedGraph {
        expected: ExpectedPersistedGraph,
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
                candidates,
                retry_participants,
                expected_status,
                expected_error,
                description,
            } => {
                write!(
                    f,
                    "{description} [expected_status: {expected_status}, expected_error: {expected_error:?}, candidates: ["
                )?;
                for (index, candidate) in candidates.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(
                        f,
                        "{{token: {}, disposition: {}, result: {}, expected_new_errors: {:?}}}",
                        candidate.staged_token,
                        candidate.disposition,
                        candidate.result_token.as_ref().map_or_else(
                            || "retained staged head".to_string(),
                            |token| token.to_string(),
                        ),
                        candidate.expected_new_errors
                    )?;
                }
                write!(f, "], retry_participants: [")?;
                for (index, participant) in retry_participants.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write!(
                        f,
                        "{{token: {}, expected_new_errors: {:?}}}",
                        participant.token, participant.expected_new_errors
                    )?;
                }
                write!(f, "]]")
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
            DanceTestStep::VerifyCommitCarrierFinding { expected, description } => {
                write!(
                    f,
                    "{description} [schema: {}, rule: {}]",
                    expected.schema_key, expected.rule_code
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
            DanceTestStep::LoadQueryTestSchema { description } => {
                write!(f, "{description}")
            }
            DanceTestStep::ExecuteQuery { description, .. } => {
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
            DanceTestStep::VerifyPersistedGraph { expected, description } => {
                write!(
                    f,
                    "{description} [enumerated: {}, edges: {}, lineage: {}]",
                    expected.enumerated.len(),
                    expected.edges.len(),
                    expected.lineage.len()
                )
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
//             DanceTestStep::Commit { candidates, expected_error, description, .. } =>
//                 f.debug_struct("Commit")
//                 .field("description", description)
//                 .field("candidates", candidates)
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
