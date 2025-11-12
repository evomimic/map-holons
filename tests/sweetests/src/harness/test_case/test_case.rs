// sweetests/src/harness/test_case/test_case.rs (excerpt)

use crate::{harness::fixtures_support::TestReference, FixtureHolons};
use holons_prelude::prelude::*;

/// Public test case type that collects steps to be executed later.
#[derive(Default, Clone, Debug)]
pub struct DancesTestCase {
    pub name: String,
    pub description: String,
    pub steps: Vec<DanceTestStep>,
}

/// - The source *token* is a TestReference that is *embedded as input* for the step. Executors will look it up at runtime
///   (Saved ≙ Staged(Committed(LocalId)) enforced at lookup time).
/// - The adders do **not** mint or return tokens; use FixtureHolons
///   to produce any “promise” tokens you want to chain in the fixture.
impl DancesTestCase {
    pub fn new<S: Into<String>>(name: S, description: S) -> Self {
        Self { name: name.into(), description: description.into(), steps: Vec::new() }
    }

    pub fn add_abandon_staged_changes_step(
        &mut self,
        // fixture_holons: &mut FixtureHolons,
        source: TestReference,
        expected_status: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push(DanceTestStep::AbandonStagedChanges { source, expected_status });
        Ok(())
    }

    pub fn add_commit_step(
        &mut self,
        expected_status: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push(DanceTestStep::Commit { expected_status });
        Ok(())
    }

    pub fn add_database_print_step(&mut self) -> Result<(), HolonError> {
        self.steps.push(DanceTestStep::PrintDatabase);
        Ok(())
    }
    pub fn add_delete_holon_step(
        &mut self,
        holon_token: TestReference,
        expected_status: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push(DanceTestStep::DeleteHolon { holon_token, expected_status });
        Ok(())
    }
    pub fn add_ensure_database_count_step(
        &mut self,
        expected_count: MapInteger,
    ) -> Result<(), HolonError> {
        self.steps.push(DanceTestStep::EnsureDatabaseCount { expected_count });
        Ok(())
    }

    pub fn add_match_saved_content_step(
        &mut self,
        expected_status: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push(DanceTestStep::MatchSavedContent { expected_status });
        Ok(())
    }

    pub fn add_query_relationships_step(
        &mut self,
        source: TestReference,
        query_expression: QueryExpression,
        expected_status: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push(DanceTestStep::QueryRelationships {
            source,
            query_expression,
            expected_status,
        });
        Ok(())
    }

    pub fn add_add_related_holons_step(
        &mut self,
        source: TestReference, // "owning" source Holon, which owns the Relationship
        relationship_name: RelationshipName,
        holons_to_add: Vec<TestReference>,
        expected_status: ResponseStatusCode,
        expected_holon: TestReference,
    ) -> Result<(), HolonError> {
        self.steps.push(DanceTestStep::AddRelatedHolons {
            source,
            relationship_name,
            holons_to_add,
            expected_status,
            expected_holon,
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
            holon_token,
            properties,
            expected_status,
        });
        Ok(())
    }

    pub fn add_remove_related_holons_step(
        &mut self,
        source: TestReference, // "owning" source Holon, which owns the Relationship
        relationship_name: RelationshipName,
        holons_to_remove: Vec<TestReference>,
        expected_status: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push(DanceTestStep::RemoveRelatedHolons {
            source,
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
        key: Option<MapString>,
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        self.steps
            .push(DanceTestStep::StageHolon { holon_token: holon_token.clone(), expected_status });
        let staged_source_token = {
            if let Some(key) = key {
                // Mint a staged-intent token indexed by key.
                fixture_holons.add_staged_with_key(holon_token.transient(), key)?
            } else {
                // Mint a staged-intent token without a key.
                fixture_holons.add_staged(holon_token.transient())
            }
        };

        Ok(staged_source_token)
    }

    pub fn add_stage_new_from_clone_step(
        &mut self,
        fixture_holons: &mut FixtureHolons,
        source: TestReference,
        new_key: MapString, // TODO: Change to Option
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        self.steps.push(DanceTestStep::StageNewFromClone {
            source: source.clone(),
            new_key: new_key.clone(),
            expected_status,
        });
        // Mint a staged-intent token indexed by key.
        let staged_source_token =
            fixture_holons.add_staged_with_key(source.transient(), new_key)?;

        Ok(staged_source_token)
    }

    pub fn add_stage_new_version_step(
        &mut self,
        fixture_holons: &mut FixtureHolons,
        source: TestReference,
        key: Option<MapString>,
        expected_status: ResponseStatusCode,
    ) -> Result<TestReference, HolonError> {
        self.steps.push(DanceTestStep::StageNewVersion { source: source.clone(), expected_status });
        let staged_source_token = {
            if let Some(key) = key {
                // Mint a staged-intent token indexed by key.
                fixture_holons.add_staged_with_key(source.transient(), key)?
            } else {
                // Mint a staged-intent token without a key.
                fixture_holons.add_staged(source.transient())
            }
        };

        Ok(staged_source_token)
    }

    pub fn add_with_properties_step(
        &mut self,
        source: TestReference,
        properties: PropertyMap,
        expected_status: ResponseStatusCode,
    ) -> Result<(), HolonError> {
        self.steps.push(DanceTestStep::WithProperties { source, properties, expected_status });
        Ok(())
    }
}

/// Internal step representation used by executors at runtime.
#[derive(Clone, Debug)]
pub enum DanceTestStep {
    AbandonStagedChanges {
        source: TestReference,
        expected_status: ResponseStatusCode,
    },
    AddRelatedHolons {
        source: TestReference,
        relationship_name: RelationshipName,
        holons_to_add: Vec<TestReference>,
        expected_status: ResponseStatusCode,
        expected_holon: TestReference,
    },
    Commit {
        expected_status: ResponseStatusCode,
    },
    DeleteHolon {
        holon_token: TestReference,
        expected_status: ResponseStatusCode,
    },
    EnsureDatabaseCount {
        expected_count: MapInteger,
    },
    MatchSavedContent {
        expected_status: ResponseStatusCode,
    },
    PrintDatabase,
    QueryRelationships {
        source: TestReference,
        query_expression: QueryExpression,
        expected_status: ResponseStatusCode,
    },
    RemoveProperties {
        holon_token: TestReference,
        properties: PropertyMap,
        expected_status: ResponseStatusCode,
    },
    RemoveRelatedHolons {
        source: TestReference,
        relationship_name: RelationshipName,
        holons_to_remove: Vec<TestReference>,
        expected_status: ResponseStatusCode,
    },
    StageHolon {
        holon_token: TestReference,
        expected_status: ResponseStatusCode,
    },
    StageNewFromClone {
        source: TestReference,
        new_key: MapString,
        expected_status: ResponseStatusCode,
    },
    StageNewVersion {
        source: TestReference,
        expected_status: ResponseStatusCode,
    },
    WithProperties {
        source: TestReference,
        properties: PropertyMap,
        expected_status: ResponseStatusCode,
    },
}

impl core::fmt::Display for DanceTestStep {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DanceTestStep::AbandonStagedChanges { source, expected_status } => {
                write!(
                    f,
                    "Marking Holon at ({:?}) as Abandoned, expecting ({:?})",
                    source, expected_status
                )
            }
            DanceTestStep::AddRelatedHolons {
                source,
                relationship_name,
                holons_to_add,
                expected_status,
                expected_holon,
            } => {
                write!(f, "AddRelatedHolons to Holon {:#?} for relationship: {:#?}, added_count: {:#?}, expecting: {:#?}, holon: {:?}", source, relationship_name, holons_to_add.len(), expected_status, expected_holon)
            }
            DanceTestStep::Commit { expected_status } => {
                write!(f, "Commit, expecting: {:?})", expected_status)
            }
            DanceTestStep::DeleteHolon { holon_token, expected_status } => {
                write!(f, "DeleteHolon({:?}, expecting: {:?},)", holon_token, expected_status)
            }
            DanceTestStep::EnsureDatabaseCount { expected_count } => {
                write!(f, "EnsureDatabaseCount = {}", expected_count.0)
            }

            DanceTestStep::MatchSavedContent { expected_status: _expected_status } => {
                write!(f, "MatchSavedContent")
            }
            DanceTestStep::PrintDatabase => {
                write!(f, "PrintDatabase")
            }
            DanceTestStep::QueryRelationships { source, query_expression, expected_status } => {
                write!(f, "QueryRelationships for source:{:#?}, with query expression: {:#?}, expecting {:#?}", source, query_expression, expected_status)
            }
            DanceTestStep::RemoveProperties { holon_token, properties, expected_status } => {
                write!(
                    f,
                    "RemoveProperties {:#?} for Holon {:#?}, expecting {:#?} ",
                    properties, holon_token, expected_status,
                )
            }
            DanceTestStep::RemoveRelatedHolons {
                source,
                relationship_name,
                holons_to_remove,
                expected_status,
            } => {
                write!(f, "RemoveRelatedHolons from Holon {:#?} for relationship: {:#?}, added_count: {:#?}, expecting: {:#?}", source, relationship_name, holons_to_remove.len(), expected_status)
            }
            DanceTestStep::StageHolon { holon_token, expected_status } => {
                write!(f, "StageHolon({:?}, expecting: {:?},)", holon_token, expected_status)
            }
            DanceTestStep::StageNewVersion { source, expected_status } => {
                write!(
                    f,
                    "NewVersion for source: {:#?}, expecting response: {:#?}",
                    source, expected_status
                )
            }
            DanceTestStep::StageNewFromClone { source, new_key, expected_status } => {
                write!(
                    f,
                    "StageNewFromClone for original_holon: {:#?}, with new key: {:?}, expecting response: {:#?}",
                    source, new_key, expected_status
                )
            }
            DanceTestStep::WithProperties { source, properties, expected_status } => {
                write!(
                    f,
                    "WithProperties for Holon {:#?} with properties: {:#?}, expecting {:#?} ",
                    source, properties, expected_status,
                )
            }
        }
    }
}
