use super::*;
use core_types::{
    CommitValidationViolation, CommitValidationViolationKind, HolonId, LocalId, ValidationSeverity,
    ValidationSubjectPath,
};
use holons_core::core_shared_objects::{
    holon::ValidationState, space_manager::HolonSpaceManager, Holon, ServiceRoutingPolicy,
};
use holons_core::reference_layer::HolonServiceApi;
use std::any::Any;

/// Response accounting must operate entirely on transaction-bound memory.
#[derive(Debug)]
struct NoStorage;

impl HolonServiceApi for NoStorage {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn commit_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &[StagedReference],
    ) -> Result<TransientReference, HolonError> {
        panic!("unexpected commit")
    }
    fn delete_holon_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &LocalId,
    ) -> Result<(), HolonError> {
        panic!("unexpected delete")
    }
    fn fetch_all_related_holons_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &HolonId,
    ) -> Result<RelationshipMap, HolonError> {
        panic!("unexpected storage read")
    }
    fn fetch_holon_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &HolonId,
    ) -> Result<Holon, HolonError> {
        panic!("unexpected storage read")
    }
    fn fetch_related_holons_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: &HolonId,
        _: &RelationshipName,
    ) -> Result<HolonCollection, HolonError> {
        panic!("unexpected storage read")
    }
    fn get_all_holons_internal(
        &self,
        _: &Arc<TransactionContext>,
    ) -> Result<HolonCollection, HolonError> {
        panic!("unexpected storage read")
    }
    fn load_holons_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: TransientReference,
    ) -> Result<TransientReference, HolonError> {
        panic!("unexpected load")
    }
}

fn context() -> Arc<TransactionContext> {
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        Arc::new(NoStorage),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    space.get_transaction_manager().open_new_transaction(space.clone()).unwrap()
}

#[test]
fn commit_status_mapping_preserves_rejection() {
    for (token, expected) in [
        ("Complete", LoadCommitStatus::Complete),
        ("Incomplete", LoadCommitStatus::Incomplete),
        ("Rejected", LoadCommitStatus::Rejected),
    ] {
        assert_eq!(
            LoadCommitStatus::from_commit_status(Some(BaseValue::StringValue(MapString::from(
                token
            )))),
            expected
        );
        assert_eq!(expected.to_string(), token);
    }
    assert_eq!(LoadCommitStatus::Skipped.to_string(), "Skipped");
    for value in [
        None,
        Some(BaseValue::IntegerValue(MapInteger(1))),
        Some(BaseValue::StringValue(MapString::from("Unknown"))),
    ] {
        assert_eq!(LoadCommitStatus::from_commit_status(value), LoadCommitStatus::Incomplete);
    }
}

#[test]
fn rejected_response_counts_findings_without_creating_loader_errors() -> Result<(), HolonError> {
    let context = context();
    let transient = context.mutation().new_holon(Some("undescribed".into()))?;
    let staged = context.mutation().stage_new_holon(transient)?;
    let finding = CommitValidationViolation {
        kind: CommitValidationViolationKind::NoDescriptor,
        rule_key: None,
        severity: ValidationSeverity::Error,
        subject: ValidationSubjectPath::Holon { holon_identity: staged.reference_id_string() },
        descriptor_identity: None,
        message: "Missing descriptor".into(),
    };
    staged.replace_validation_outcome(ValidationState::NoDescriptor, vec![finding.clone()])?;
    assert!(HolonLoaderController::collect_commit_errors(&context)?.is_empty());
    let response = HolonLoaderController::new().build_response(
        &context,
        1,
        1,
        0,
        0,
        0,
        1,
        1,
        1,
        LoadCommitStatus::Rejected,
        "Commit rejected".into(),
        Vec::new(),
    )?;
    assert_eq!(
        response.property_value(CorePropertyTypeName::LoadCommitStatus)?,
        Some(BaseValue::StringValue(MapString::from("Rejected")))
    );
    for (property, expected) in [
        (CorePropertyTypeName::HolonsCommitted, 0),
        (CorePropertyTypeName::ErrorCount, 0),
        (CorePropertyTypeName::ValidationViolationCount, 1),
    ] {
        assert_eq!(
            response.property_value(property)?,
            Some(BaseValue::IntegerValue(MapInteger(expected)))
        );
    }
    assert_eq!(
        response
            .related_holons(CoreRelationshipTypeName::HasLoadError)?
            .read()
            .unwrap()
            .get_count()
            .0,
        0
    );
    assert_eq!(staged.validation_findings()?, vec![finding]);
    assert!(context.is_open());
    Ok(())
}

#[test]
fn nonrejected_responses_expose_zero_validation_count() -> Result<(), HolonError> {
    for status in
        [LoadCommitStatus::Complete, LoadCommitStatus::Incomplete, LoadCommitStatus::Skipped]
    {
        let context = context();
        let response = HolonLoaderController::new().build_response(
            &context,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            status,
            String::new(),
            Vec::new(),
        )?;
        assert_eq!(
            response.property_value(CorePropertyTypeName::ValidationViolationCount)?,
            Some(BaseValue::IntegerValue(MapInteger(0)))
        );
    }
    Ok(())
}
