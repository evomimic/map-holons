//! Attempt-scoped observations for rejected new-root creates.

use super::persisted_read_support::fresh_related_holon_ids;
use holons_prelude::prelude::*;
use holons_test::{
    assert_rejection_key_absent, assert_rejection_relationship_unchanged, assert_rejection_workset,
    NamedRelationshipSubject, ResolveBy, TestExecutionState, TestReference,
};
use integrity_core_types::LocalId;
use map_commands_contract::{MapCommand, MapResult, SpaceCommand};

pub(super) struct RejectionObservations {
    keys: Vec<MapString>,
    relationships: Vec<(HolonId, RelationshipName, Vec<LocalId>)>,
}

impl RejectionObservations {
    /// Establishes coverage and baselines before dispatch; no scope comes from Commit's response.
    pub(super) async fn capture(
        state: &TestExecutionState,
        subjects: &[TestReference],
        unchanged_relationships: &[NamedRelationshipSubject],
        candidates: &[&StagedReference],
    ) -> Self {
        let context = state.context();
        let mut keys = Vec::new();
        let mut identities = Vec::new();
        for subject in subjects {
            let reference = state
                .resolve_execution_reference(&context, ResolveBy::Expected, subject)
                .expect("fixture rejection subject must resolve");
            let HolonReference::Staged(staged) = reference else {
                panic!("fixture rejection subject must remain staged");
            };
            assert!(
                staged.versioned_source_id().unwrap().is_none(),
                "bounded rejection requires new roots"
            );
            assert!(!staged.is_committed().unwrap(), "bounded rejection subject already committed");
            let key = staged.key().unwrap().expect("bounded rejection subject key");
            assert_eq!(Some(key.clone()), subject.expected_snapshot().snapshot().key().unwrap());
            keys.push(key);
            identities.push(staged.reference_id_string());
        }
        let actual: Vec<_> =
            candidates.iter().map(|reference| reference.reference_id_string()).collect();
        assert_rejection_workset(&identities, &actual);
        let mut relationships = Vec::new();
        for collection in unchanged_relationships {
            let source = state
                .resolve_execution_reference(&context, ResolveBy::Expected, &collection.source)
                .expect("saved rejection guard must resolve");
            assert!(matches!(source, HolonReference::Smart(_)), "rejection guard must be saved");
            relationships.push((
                source.holon_id().unwrap(),
                collection.relationship.clone(),
                Vec::new(),
            ));
        }
        let mut observations = Self { keys, relationships };
        let current = observations.read_persisted(state).await;
        for (collection, baseline) in observations.relationships.iter_mut().zip(current) {
            collection.2 = baseline;
        }
        observations
    }

    /// Opens an observer without importing fixture transients or changing the active transaction.
    async fn read_persisted(&self, state: &TestExecutionState) -> Vec<Vec<LocalId>> {
        let result = state
            .dispatch_command(
                MapCommand::Space(SpaceCommand::BeginTransaction),
                "rejection persistence observer",
            )
            .await
            .expect("begin rejection observer");
        let MapResult::TransactionCreated { tx_id } = result else {
            panic!("expected rejection observer transaction");
        };
        let context = state.runtime().session().get_transaction(&tx_id).unwrap();
        for key in &self.keys {
            assert_rejection_key_absent(
                key,
                context.lookup().get_saved_holon_by_key(key).map(|_| ()),
            );
        }
        let current = self
            .relationships
            .iter()
            .map(|(id, name, _)| {
                let source = HolonReference::smart_from_id(context.space_read_handle(), id.clone());
                fresh_related_holon_ids(&source, &name.to_string())
            })
            .collect();
        state.runtime().session().archive_transaction(&tx_id).expect("archive rejection observer");
        current
    }

    pub(super) async fn verify(&self, state: &TestExecutionState, subjects: &[TestReference]) {
        for subject in subjects {
            let reference = state
                .resolve_execution_reference(&state.context(), ResolveBy::Expected, subject)
                .unwrap();
            let HolonReference::Staged(staged) = reference else {
                panic!("rejected subject must remain staged");
            };
            assert!(staged.is_live_validation_candidate().unwrap());
            assert!(!staged.is_committed().unwrap(), "rejection committed a subject");
        }
        for ((id, name, before), after) in
            self.relationships.iter().zip(self.read_persisted(state).await)
        {
            assert_rejection_relationship_unchanged(
                &format!("{id:?} --{name}"),
                before.clone(),
                after,
            );
        }
    }
}
