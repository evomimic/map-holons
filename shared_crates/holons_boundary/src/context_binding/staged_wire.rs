use crate::context_binding::staged_relationship_wire::StagedRelationshipMapWire;
use base_types::MapInteger;
use core_types::{CommitValidationViolation, HolonError, LocalId, PropertyMap, RelationshipName};
use holons_core::core_shared_objects::holon::{
    HolonState, RelationshipCommitScope, StagedState, ValidationState,
};
use holons_core::core_shared_objects::transactions::TransactionContext;
use holons_core::core_shared_objects::StagedHolon;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StagedHolonWire {
    version: MapInteger,
    holon_state: HolonState,
    staged_state: StagedState,
    validation_state: ValidationState,
    #[serde(default)]
    validation_findings: Vec<CommitValidationViolation>,
    property_map: PropertyMap,
    staged_relationships: StagedRelationshipMapWire,
    original_id: Option<LocalId>,
    #[serde(default)]
    versioned_source_id: Option<LocalId>,
    #[serde(default)]
    touched_relationship_names: BTreeSet<RelationshipName>,
    relationship_commit_scope: RelationshipCommitScope,
    errors: Vec<HolonError>,
}

impl StagedHolonWire {
    /// Binds this holon's nested references to a TransactionContext, validating tx_id.
    pub fn bind(self, context: &Arc<TransactionContext>) -> Result<StagedHolon, HolonError> {
        Ok(StagedHolon::from_parts(
            self.version,
            self.holon_state,
            self.staged_state,
            self.validation_state,
            self.validation_findings,
            self.property_map,
            self.staged_relationships.bind(context)?,
            self.original_id,
            self.versioned_source_id,
            self.touched_relationship_names,
            self.relationship_commit_scope,
            self.errors,
        ))
    }

    /// Rebinds this holon's nested references to a different transaction
    /// context, bypassing tx_id validation. See [`StagedRelationshipMapWire::rebind`].
    pub fn rebind(self, context: &Arc<TransactionContext>) -> Result<StagedHolon, HolonError> {
        Ok(StagedHolon::from_parts(
            self.version,
            self.holon_state,
            self.staged_state,
            self.validation_state,
            self.validation_findings,
            self.property_map,
            self.staged_relationships.rebind(context)?,
            self.original_id,
            self.versioned_source_id,
            self.touched_relationship_names,
            self.relationship_commit_scope,
            self.errors,
        ))
    }
}

impl From<&StagedHolon> for StagedHolonWire {
    fn from(value: &StagedHolon) -> Self {
        Self {
            version: value.version().clone(),
            holon_state: value.holon_state().clone(),
            staged_state: value.staged_state().clone(),
            validation_state: value.validation_state().clone(),
            validation_findings: value.validation_findings().to_vec(),
            property_map: value.property_map().clone(),
            staged_relationships: StagedRelationshipMapWire::from(value.staged_relationships()),
            original_id: value.original_id_ref().cloned(),
            versioned_source_id: value.versioned_source_id_ref().cloned(),
            touched_relationship_names: value.touched_relationship_names().clone(),
            relationship_commit_scope: value.relationship_commit_scope(),
            errors: value.errors().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base_types::MapString;
    use core_types::RelationshipName;
    use holons_core::core_shared_objects::holon::HolonCloneModel;
    use holons_core::core_shared_objects::WriteableHolonState;
    use holons_core::core_shared_objects::{
        space_manager::HolonSpaceManager, ServiceRoutingPolicy,
    };
    use holons_core::RelationshipMap;

    #[derive(Debug)]
    struct UnusedHolonService;

    impl holons_core::HolonServiceApi for UnusedHolonService {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn commit_internal(
            &self,
            _: &Arc<TransactionContext>,
            _: &[holons_core::StagedReference],
        ) -> Result<holons_core::TransientReference, HolonError> {
            panic!("wire binding must not access persistence")
        }
        fn delete_holon_internal(
            &self,
            _: &Arc<TransactionContext>,
            _: &LocalId,
        ) -> Result<(), HolonError> {
            panic!("wire binding must not access persistence")
        }
        fn fetch_all_related_holons_internal(
            &self,
            _: &Arc<TransactionContext>,
            _: &core_types::HolonId,
        ) -> Result<holons_core::RelationshipMap, HolonError> {
            panic!("wire binding must not access persistence")
        }
        fn fetch_holon_internal(
            &self,
            _: &Arc<TransactionContext>,
            _: &core_types::HolonId,
        ) -> Result<holons_core::core_shared_objects::Holon, HolonError> {
            panic!("wire binding must not access persistence")
        }
        fn fetch_related_holons_internal(
            &self,
            _: &Arc<TransactionContext>,
            _: &core_types::HolonId,
            _: &RelationshipName,
        ) -> Result<holons_core::HolonCollection, HolonError> {
            panic!("wire binding must not access persistence")
        }
        fn get_all_holons_internal(
            &self,
            _: &Arc<TransactionContext>,
        ) -> Result<holons_core::HolonCollection, HolonError> {
            panic!("wire binding must not access persistence")
        }
        fn load_holons_internal(
            &self,
            _: &Arc<TransactionContext>,
            _: holons_core::TransientReference,
        ) -> Result<holons_core::TransientReference, HolonError> {
            panic!("wire binding must not access persistence")
        }
    }

    fn test_space() -> Arc<HolonSpaceManager> {
        Arc::new(HolonSpaceManager::new_with_managers(
            None,
            Arc::new(UnusedHolonService),
            None,
            ServiceRoutingPolicy::BlockExternal,
        ))
    }

    fn graph_only_update_with_touched_collection() -> Result<StagedHolon, HolonError> {
        let model = HolonCloneModel::new(
            MapInteger(1),
            None,
            PropertyMap::new(),
            Some(RelationshipMap::new_empty()),
        );
        let mut staged =
            StagedHolon::new_for_update_from_clone_model(model, LocalId(vec![1, 2, 3]))?;
        staged.add_related_holons(RelationshipName(MapString("Touched".into())), Vec::new())?;
        staged.note_relationship_mutation(false)?;
        assert_eq!(staged.staged_state(), &StagedState::ForUpdateGraphOnly);
        Ok(staged)
    }

    #[test]
    fn validation_findings_round_trip_and_default_when_absent() -> Result<(), HolonError> {
        use core_types::{
            CommitValidationViolationKind, ValidationSeverity, ValidationSubjectPath,
        };
        let space = test_space();
        let context =
            space.get_transaction_manager().open_public_transaction(Arc::clone(&space))?;
        let mut staged = StagedHolon::new_for_create();
        staged.add_error(HolonError::NotImplemented("persistence".into()))?;
        staged.replace_validation_outcome(
            ValidationState::Invalid,
            vec![CommitValidationViolation {
                kind: CommitValidationViolationKind::RuleViolation { code: "DS-PROP-001".into() },
                rule_key: Some("required-property".into()),
                severity: ValidationSeverity::Error,
                subject: ValidationSubjectPath::Property {
                    holon_identity: "subject".into(),
                    name: "Name".into(),
                },
                descriptor_identity: Some("Name.PropertyType".into()),
                message: "Supply Name".into(),
            }],
        )?;
        let mut json = serde_json::to_value(StagedHolonWire::from(&staged)).unwrap();
        assert_eq!(
            json["validation_findings"][0]["kind"],
            serde_json::json!({"RuleViolation": {"code": "DS-PROP-001"}})
        );
        assert_eq!(json["errors"], serde_json::json!([{"NotImplemented": "persistence"}]));
        let wire: StagedHolonWire = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(wire.clone().bind(&context)?, staged);
        assert_eq!(wire.rebind(&context)?, staged);

        json.as_object_mut().unwrap().remove("validation_findings");
        let legacy: StagedHolonWire = serde_json::from_value(json.clone()).unwrap();
        let rebound = legacy.bind(&context)?;
        assert!(rebound.validation_findings().is_empty());
        assert_eq!(rebound.validation_state(), staged.validation_state());
        assert_eq!(rebound.errors(), staged.errors());

        json.as_object_mut().unwrap().remove("relationship_commit_scope");
        assert!(
            serde_json::from_value::<StagedHolonWire>(json).is_err(),
            "relationship_commit_scope is required so an interrupted graph-only retry cannot default to Full"
        );
        Ok(())
    }

    /// Regression guard (issue #515): relationship-mutation intent
    /// (`touched_relationship_names`) is transaction-scoped staged state that must
    /// survive the dance / session-state round-trip. Sweetests export staged holons
    /// to `StagedHolonWire` between the `add` and `commit` dances; if the wire drops
    /// the touched set, a graph-only commit rehydrates with an empty filter and
    /// persists none of the mutated relationships.
    ///
    /// This asserts the wire carries the touched set. It fails until the wire type
    /// and `From<&StagedHolon>` are taught to propagate `touched_relationship_names`.
    #[test]
    fn staged_holon_wire_preserves_touched_relationship_names() {
        let mut staged = StagedHolon::new_for_create();
        let relationship = RelationshipName(MapString("Properties".to_string()));
        staged
            .add_related_holons(relationship, Vec::new())
            .expect("recording a relationship mutation should succeed");

        let wire = StagedHolonWire::from(&staged);
        let json = serde_json::to_value(&wire).expect("StagedHolonWire should serialize");

        assert!(
            json.get("touched_relationship_names").is_some(),
            "StagedHolonWire must carry touched_relationship_names so relationship-mutation \
             intent survives the dance/session-state round-trip; without it, graph-only \
             commits lose their touched set and persist no relationship changes"
        );
        assert!(
            json.get("relationship_commit_scope").is_some(),
            "StagedHolonWire must carry relationship_commit_scope so retry preserves its Pass-2 persistence decision"
        );
    }

    #[test]
    fn relationship_commit_scope_and_touched_names_survive_bind_and_rebind(
    ) -> Result<(), HolonError> {
        let space = test_space();
        let source_context =
            space.get_transaction_manager().open_public_transaction(Arc::clone(&space))?;
        let destination_context =
            space.get_transaction_manager().open_public_transaction(Arc::clone(&space))?;
        let touched = RelationshipName(MapString("Touched".into()));

        for scope in [RelationshipCommitScope::Full, RelationshipCommitScope::TouchedOnly] {
            let mut staged = graph_only_update_with_touched_collection()?;
            match scope {
                RelationshipCommitScope::Full => {
                    staged.note_property_mutation()?;
                    staged.prepare_full_relationship_commit_scope()?;
                    assert_eq!(staged.staged_state(), &StagedState::ForUpdateNewVersion);
                }
                RelationshipCommitScope::TouchedOnly => {
                    staged.prepare_touched_relationship_commit_scope()?
                }
            }

            assert_eq!(staged.touched_relationship_names().len(), 1);
            assert!(staged.touched_relationship_names().contains(&touched));

            let projected = serde_json::to_value(StagedHolonWire::from(&staged)).unwrap();
            let wire: StagedHolonWire = serde_json::from_value(projected).unwrap();
            for restored in
                [wire.clone().bind(&source_context)?, wire.rebind(&destination_context)?]
            {
                assert_eq!(restored.relationship_commit_scope(), scope);
                assert_eq!(
                    restored.touched_relationship_names(),
                    staged.touched_relationship_names()
                );
                assert_eq!(restored, staged);
            }
        }
        Ok(())
    }

    #[test]
    fn committed_staged_holon_retains_relationship_scope_after_wire_restoration(
    ) -> Result<(), HolonError> {
        let space = test_space();
        let source_context =
            space.get_transaction_manager().open_public_transaction(Arc::clone(&space))?;
        let destination_context =
            space.get_transaction_manager().open_public_transaction(Arc::clone(&space))?;
        let mut staged = graph_only_update_with_touched_collection()?;
        staged.prepare_touched_relationship_commit_scope()?;
        let saved_id = LocalId(vec![1, 2, 3]);
        staged.to_committed(saved_id.clone())?;

        let projected = serde_json::to_value(StagedHolonWire::from(&staged)).unwrap();
        let wire: StagedHolonWire = serde_json::from_value(projected).unwrap();
        for restored in [wire.clone().bind(&source_context)?, wire.rebind(&destination_context)?] {
            assert_eq!(restored.staged_state(), &StagedState::Committed(saved_id.clone()));
            assert_eq!(restored.relationship_commit_scope(), RelationshipCommitScope::TouchedOnly);
            assert_eq!(restored.touched_relationship_names(), staged.touched_relationship_names());
            assert_eq!(restored, staged);
        }
        Ok(())
    }

    #[test]
    fn invalid_relationship_commit_scope_values_are_rejected() {
        let valid =
            serde_json::to_value(StagedHolonWire::from(&StagedHolon::new_for_create())).unwrap();
        for invalid in
            [serde_json::json!("Touched"), serde_json::json!("full"), serde_json::Value::Null]
        {
            let mut candidate = valid.clone();
            candidate["relationship_commit_scope"] = invalid.clone();
            assert!(
                serde_json::from_value::<StagedHolonWire>(candidate).is_err(),
                "invalid relationship_commit_scope {invalid} must not decode"
            );
        }
    }
}
