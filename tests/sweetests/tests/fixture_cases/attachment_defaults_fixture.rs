use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, ExpectedCommitStatus, TestCaseInit};

/// Descriptor attachment supplies explicit required defaults before public Commit.
pub fn attachment_defaults_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "Attachment defaults persist",
        "Attach an established meta-type to a staged HolonType and persist its defaults",
    );
    // The runtime already provisions Core; the domain load uses its saved descriptors.
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    let mut descriptors = Vec::new();
    for key in
        ["HolonType.TypeDescriptor", "MetaHolonType.MetaTypeDescriptor", "MAP Core Schema-v0.0.7"]
    {
        let key = MapString(key.into());
        let stub = fixture_context.mutation().new_holon(Some(key.clone()))?;
        descriptors.push(test_case.add_lookup_saved_holon_by_key_step(
            &mut fixture_holons,
            stub,
            key,
            None,
            None,
        )?);
    }
    let key = MapString("AttachmentDefaults.HolonType".into());
    let source = fixture_context.mutation().new_holon(Some(key.clone()))?;
    // MetaTypeDescriptor requires these header fields without defaults. The meta-type's
    // three required Boolean defaults are deliberately omitted from the authored model.
    let properties: PropertyMap = [
        ("TypeName".to_property_name(), "AttachmentDefaults".to_base_value()),
        ("TypeNamePlural".to_property_name(), "AttachmentDefaultsTypes".to_base_value()),
        ("DisplayName".to_property_name(), "Attachment defaults".to_base_value()),
        ("DisplayNamePlural".to_property_name(), "Attachment default types".to_base_value()),
        ("Description".to_property_name(), "Attachment-time population fixture".to_base_value()),
    ]
    .into();
    let subject = test_case.add_new_holon_step(
        &mut fixture_holons,
        source,
        properties,
        Some(key),
        None,
        None,
    )?;
    let subject = test_case.add_stage_holon_step(&mut fixture_holons, subject, None, None)?;
    let subject = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        subject,
        CoreRelationshipTypeName::Extends.as_relationship_name(),
        vec![descriptors[0].clone()],
        None,
        None,
    )?;
    let subject = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        subject,
        CoreRelationshipTypeName::ComponentOf.as_relationship_name(),
        vec![descriptors[2].clone()],
        None,
        None,
    )?;
    let defaults: PropertyMap = [
        ("IsAbstractType".to_property_name(), false.to_base_value()),
        ("DefinesInstanceTypeKind".to_property_name(), false.to_base_value()),
        ("InstanceDeletionAllowed".to_property_name(), true.to_base_value()),
    ]
    .into();
    test_case.add_with_descriptor_step(
        &mut fixture_holons,
        subject,
        descriptors[1].clone(),
        defaults,
        None,
        None,
    )?;
    test_case.add_commit_step(&mut fixture_holons, ExpectedCommitStatus::Complete, None, None)?;
    test_case.add_match_saved_content_step()?;
    test_case.finalize(&fixture_context, &fixture_holons)?;
    Ok(test_case)
}
