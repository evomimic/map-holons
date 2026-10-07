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
struct NoStorage {
    fail_lookup: bool,
    allow_commit: bool,
}

impl HolonServiceApi for NoStorage {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn commit_internal(
        &self,
        context: &Arc<TransactionContext>,
        _: &[StagedReference],
    ) -> Result<TransientReference, HolonError> {
        assert!(self.allow_commit, "unexpected commit");
        let mut response = context.mutation().new_holon(Some("commit-response".into()))?;
        response.with_property_value("CommitRequestStatus", "Complete")?;
        Ok(response)
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
    fn get_saved_holon_by_key_internal(
        &self,
        _: &Arc<TransactionContext>,
        key: &MapString,
    ) -> Result<SmartReference, HolonError> {
        if self.fail_lookup {
            Err(HolonError::NotAccessible("descriptor lookup".into(), "unavailable".into()))
        } else {
            Err(HolonError::HolonNotFound(key.to_string()))
        }
    }
    fn load_holons_internal(
        &self,
        _: &Arc<TransactionContext>,
        _: TransientReference,
    ) -> Result<TransientReference, HolonError> {
        panic!("unexpected load")
    }
}

/// Shared in-memory context for loader tests; every storage operation fails fast.
pub(crate) fn context() -> Arc<TransactionContext> {
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        Arc::new(NoStorage { fail_lookup: false, allow_commit: false }),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    let context = space.get_transaction_manager().open_public_transaction(space.clone()).unwrap();
    context
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
            ))))
            .unwrap(),
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
        assert!(LoadCommitStatus::from_commit_status(value).is_err());
    }
}

#[test]
fn rejected_response_counts_findings_without_creating_loader_errors() -> Result<(), HolonError> {
    let context = context();
    context.enable_bootstrap_provisioning();
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
        crate::response_descriptor::resolve_response_descriptor(
            &context,
            "HolonLoadResponse.DanceResponseType",
        )?,
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
        context.enable_bootstrap_provisioning();
        let response = HolonLoaderController::new().build_response(
            &context,
            crate::response_descriptor::resolve_response_descriptor(
                &context,
                "HolonLoadResponse.DanceResponseType",
            )?,
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

#[test]
fn loader_responses_and_errors_use_canonical_descriptors() -> Result<(), HolonError> {
    let context = context();
    for key in [
        "HolonLoadResponse.DanceResponseType",
        "HolonLoadError.HolonError",
        "LoadDiagnostic.Projection",
        "CommitResponse.Projection",
    ] {
        let descriptor = context.mutation().new_holon(Some(key.into()))?;
        context.mutation().stage_new_holon(descriptor)?;
    }
    let errors = crate::errors::make_load_error_holons(
        &context,
        crate::response_descriptor::resolve_response_descriptor(
            &context,
            "HolonLoadError.HolonError",
        )?,
        &[crate::errors::ErrorWithContext::new(HolonError::InvalidParameter("bad input".into()))],
        None,
    )?;
    let response = HolonLoaderController::new().build_response(
        &context,
        crate::response_descriptor::resolve_response_descriptor(
            &context,
            "HolonLoadResponse.DanceResponseType",
        )?,
        1,
        0,
        0,
        0,
        1,
        0,
        0,
        0,
        LoadCommitStatus::Skipped,
        "skipped".into(),
        errors.clone(),
    )?;
    for (reference, key) in [
        (response, "HolonLoadResponse.DanceResponseType"),
        (errors[0].clone(), "HolonLoadError.HolonError"),
    ] {
        let descriptors =
            reference.related_holons("DescribedBy")?.read().unwrap().get_members().clone();
        assert_eq!(descriptors.len(), 1);
        assert_eq!(descriptors[0].key()?, Some(key.into()));
    }
    Ok(())
}

#[test]
fn bootstrap_descriptor_absence_is_allowed_but_ambiguity_is_not() -> Result<(), HolonError> {
    let context = context();
    context.enable_bootstrap_provisioning();
    let key = "HolonLoadResponse.DanceResponseType";
    assert!(crate::response_descriptor::resolve_response_descriptor(&context, key)?.is_none());
    for _ in 0..2 {
        let descriptor = context.mutation().new_holon(Some(key.into()))?;
        context.mutation().stage_new_holon(descriptor)?;
    }
    assert!(matches!(
        crate::response_descriptor::resolve_response_descriptor(&context, key),
        Err(HolonError::DuplicateError(..))
    ));
    Ok(())
}

#[test]
fn ordinary_descriptor_absence_is_an_error() {
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        Arc::new(NoStorage { fail_lookup: false, allow_commit: false }),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    let context = space.get_transaction_manager().open_public_transaction(space.clone()).unwrap();
    assert!(matches!(
        crate::response_descriptor::resolve_response_descriptor(
            &context,
            "HolonLoadResponse.DanceResponseType"
        ),
        Err(HolonError::HolonNotFound(_))
    ));
}

#[test]
fn bootstrap_does_not_swallow_operational_descriptor_failure() {
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        Arc::new(NoStorage { fail_lookup: true, allow_commit: false }),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    let context = space.get_transaction_manager().open_public_transaction(space.clone()).unwrap();
    context.enable_bootstrap_provisioning();
    context.set_space_holon_id(HolonId::Local(LocalId(vec![1, 2, 3]))).unwrap();
    assert!(matches!(
        crate::response_descriptor::resolve_response_descriptor(
            &context,
            "HolonLoadResponse.DanceResponseType"
        ),
        Err(HolonError::NotAccessible(..))
    ));
}

#[test]
fn retained_descriptor_can_type_response_after_successful_commit() -> Result<(), HolonError> {
    let space = Arc::new(HolonSpaceManager::new_with_managers(
        None,
        Arc::new(NoStorage { fail_lookup: false, allow_commit: true }),
        None,
        ServiceRoutingPolicy::BlockExternal,
    ));
    let context = space.get_transaction_manager().open_public_transaction(space.clone())?;
    let key = "HolonLoadResponse.DanceResponseType";
    let descriptor = context.mutation().new_holon(Some(key.into()))?;
    context.mutation().stage_new_holon(descriptor)?;
    let descriptor = crate::response_descriptor::resolve_response_descriptor(&context, key)?;
    context.commit()?;
    assert!(!context.is_open());
    assert!(crate::response_descriptor::resolve_response_descriptor(&context, key).is_err());
    let response = HolonLoaderController::new().build_response(
        &context,
        descriptor,
        1,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        LoadCommitStatus::Complete,
        "complete".into(),
        vec![],
    )?;
    let descriptors = response.related_holons("DescribedBy")?.read().unwrap().get_members().clone();
    assert_eq!(descriptors[0].key()?, Some(key.into()));
    Ok(())
}
