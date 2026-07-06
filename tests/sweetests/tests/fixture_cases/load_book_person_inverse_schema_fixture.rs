use holons_prelude::prelude::*;
use holons_test::harness::helpers::{
    BOOK_DESCRIPTOR_KEY, BOOK_KEY, BOOK_PERSON_INVERSE_METRICS, BOOK_TO_PERSON_RELATIONSHIP,
    CORE_SCHEMA_METRICS, PERSON_1_KEY, PERSON_DESCRIPTOR_KEY,
};
use holons_test::{DancesTestCase, ExpectedCommitStatus, TestCaseInit};

/// Fixture for the `LoadBookPersonInverseTestSchema` preset step.
///
/// Loads MAP core schema first, then starts a fresh transaction and imports the
/// Book/Person inverse test schema through public MAP Commands `LoadHolons`
/// ingress. This exercises loader resolution against already-saved core-schema
/// holons rather than restaging core and domain together in one import.
///
/// After verifying the loaded descriptors, the fixture exercises schema-backed
/// **instance** persistence (issue #442): it stages a Book and a Person
/// described by their loaded `HolonType` descriptors, relates them through the
/// declared `AuthoredBy` relationship, commits with an expected `Complete`
/// status, and asserts bidirectional SmartLink traversal — the forward
/// declared edges plus the commit-materialized inverse edges (`Authors`,
/// `Instances`).
pub fn load_book_person_inverse_schema_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "load_book_person_inverse_schema",
        "Load Book/Person inverse test schema after committed MAP core schema, \
             then commit described Book/Person instances and verify bidirectional traversal",
    );

    test_case.add_load_core_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    test_case.add_load_book_person_inverse_test_schema_step(None)?;
    test_case.add_verify_book_person_descriptors_step(None)?;

    // --- Schema-backed instance persistence (issue #442) ---
    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for described Book/Person instances".to_string()),
    )?;

    // Resolve the loader-saved type descriptors so they can be DescribedBy targets.
    let book_type_stub =
        fixture_context.mutation().new_holon(Some(MapString(BOOK_DESCRIPTOR_KEY.to_string())))?;
    let book_type_token = test_case.add_lookup_saved_holon_by_key_step(
        &mut fixture_holons,
        book_type_stub,
        MapString(BOOK_DESCRIPTOR_KEY.to_string()),
        None,
        None,
    )?;
    let person_type_stub =
        fixture_context.mutation().new_holon(Some(MapString(PERSON_DESCRIPTOR_KEY.to_string())))?;
    let person_type_token = test_case.add_lookup_saved_holon_by_key_step(
        &mut fixture_holons,
        person_type_stub,
        MapString(PERSON_DESCRIPTOR_KEY.to_string()),
        None,
        None,
    )?;

    // Book instance with the schema-declared Title property.
    let book_source =
        fixture_context.mutation().new_holon(Some(MapString(BOOK_KEY.to_string())))?;
    let mut book_properties = PropertyMap::new();
    book_properties
        .insert("Title".to_property_name(), MapString(BOOK_KEY.to_string()).to_base_value());
    let book_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        book_source,
        book_properties,
        Some(MapString(BOOK_KEY.to_string())),
        None,
        None,
    )?;
    let book_token = test_case.add_stage_holon_step(&mut fixture_holons, book_token, None, None)?;

    // Person instance with the schema-declared Name property.
    let person_source =
        fixture_context.mutation().new_holon(Some(MapString(PERSON_1_KEY.to_string())))?;
    let mut person_properties = PropertyMap::new();
    person_properties
        .insert("Name".to_property_name(), MapString(PERSON_1_KEY.to_string()).to_base_value());
    let person_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        person_source,
        person_properties,
        Some(MapString(PERSON_1_KEY.to_string())),
        None,
        None,
    )?;
    let person_token =
        test_case.add_stage_holon_step(&mut fixture_holons, person_token, None, None)?;

    // Describe both instances, then author the Book by the Person.
    let book_token = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        book_token,
        CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
        vec![book_type_token],
        None,
        Some("Describe Book by Book.HolonType".to_string()),
    )?;
    let person_token = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        person_token.clone(),
        CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
        vec![person_type_token],
        None,
        Some("Describe Person by Person.HolonType".to_string()),
    )?;
    test_case.add_add_related_holons_step(
        &mut fixture_holons,
        book_token,
        RelationshipName(MapString(BOOK_TO_PERSON_RELATIONSHIP.to_string())),
        vec![person_token],
        None,
        Some("Relate Book --AuthoredBy--> Person".to_string()),
    )?;

    test_case.add_commit_step(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        Some("Commit described Book/Person instances".to_string()),
    )?;

    // DB = fixture-saved holons (incl. space baseline) + loader-committed schema holons.
    let expected_db_count = MapInteger(
        fixture_holons.count_saved().0
            + CORE_SCHEMA_METRICS.committed
            + BOOK_PERSON_INVERSE_METRICS.committed,
    );
    test_case.add_ensure_database_count_step(expected_db_count, None)?;
    test_case.add_match_saved_content_step()?;
    test_case.add_verify_book_person_instance_links_step(None)?;

    test_case.finalize(&fixture_context, &fixture_holons)?;

    Ok(test_case)
}

/// Regression fixture for saved-content comparison of frozen relationship members
/// (issue #555 head-redirect).
///
/// Book and Person are staged together and `Book --AuthoredBy--> Person` is
/// authored while Person is still staged, so Book's expected `AuthoredBy` member
/// is frozen at Person's *staged* snapshot. Committing the transaction advances
/// Person's head to its Saved snapshot under a new id, leaving that frozen member
/// id stale. The saved-content assertion must redirect the frozen member through
/// the fixture head index to the committed Person before comparing it to the DB
/// member. Everything stays in a single transaction so the cross-transaction
/// execution-step binding path (issue #556) is not exercised here.
pub fn frozen_member_head_redirect_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "frozen_member_head_redirect",
        "Saved-content assertion redirects a frozen staged relationship member to its committed head",
    );

    test_case.add_load_core_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    test_case.add_load_book_person_inverse_test_schema_step(None)?;

    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for staged Book and Person instances".to_string()),
    )?;

    // Resolve the loader-saved type descriptors so they can be DescribedBy targets.
    let book_type_stub =
        fixture_context.mutation().new_holon(Some(MapString(BOOK_DESCRIPTOR_KEY.to_string())))?;
    let book_type_token = test_case.add_lookup_saved_holon_by_key_step(
        &mut fixture_holons,
        book_type_stub,
        MapString(BOOK_DESCRIPTOR_KEY.to_string()),
        None,
        None,
    )?;
    let person_type_stub =
        fixture_context.mutation().new_holon(Some(MapString(PERSON_DESCRIPTOR_KEY.to_string())))?;
    let person_type_token = test_case.add_lookup_saved_holon_by_key_step(
        &mut fixture_holons,
        person_type_stub,
        MapString(PERSON_DESCRIPTOR_KEY.to_string()),
        None,
        None,
    )?;

    // Stage Person, then Book, in the same transaction.
    let person_source =
        fixture_context.mutation().new_holon(Some(MapString(PERSON_1_KEY.to_string())))?;
    let mut person_properties = PropertyMap::new();
    person_properties
        .insert("Name".to_property_name(), MapString(PERSON_1_KEY.to_string()).to_base_value());
    let person_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        person_source,
        person_properties,
        Some(MapString(PERSON_1_KEY.to_string())),
        None,
        None,
    )?;
    let person_token =
        test_case.add_stage_holon_step(&mut fixture_holons, person_token, None, None)?;

    let book_source =
        fixture_context.mutation().new_holon(Some(MapString(BOOK_KEY.to_string())))?;
    let mut book_properties = PropertyMap::new();
    book_properties
        .insert("Title".to_property_name(), MapString(BOOK_KEY.to_string()).to_base_value());
    let book_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        book_source,
        book_properties,
        Some(MapString(BOOK_KEY.to_string())),
        None,
        None,
    )?;
    let book_token = test_case.add_stage_holon_step(&mut fixture_holons, book_token, None, None)?;

    // Describe both instances, then freeze Book --AuthoredBy--> the staged Person.
    let book_token = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        book_token,
        CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
        vec![book_type_token],
        None,
        Some("Describe Book by Book.HolonType".to_string()),
    )?;
    let person_token = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        person_token,
        CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
        vec![person_type_token],
        None,
        Some("Describe Person by Person.HolonType".to_string()),
    )?;
    test_case.add_add_related_holons_step(
        &mut fixture_holons,
        book_token,
        RelationshipName(MapString(BOOK_TO_PERSON_RELATIONSHIP.to_string())),
        vec![person_token],
        None,
        Some("Freeze Book --AuthoredBy--> staged Person before commit".to_string()),
    )?;

    test_case.add_commit_step(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        Some("Commit Book and Person together; heads advance to Saved".to_string()),
    )?;

    let expected_db_count = MapInteger(
        fixture_holons.count_saved().0
            + CORE_SCHEMA_METRICS.committed
            + BOOK_PERSON_INVERSE_METRICS.committed,
    );
    test_case.add_ensure_database_count_step(expected_db_count, None)?;
    test_case.add_match_saved_content_step()?;

    test_case.finalize(&fixture_context, &fixture_holons)?;

    Ok(test_case)
}

/// Cross-transaction variant of the frozen-member regression, pending issue #556.
///
/// Person is staged and committed in one transaction; Book — authored with a
/// frozen reference to Person's *staged* snapshot — is staged and committed in a
/// *later* transaction. The frozen staged Person reference is carried into the
/// later commit, where the production session-import bind correctly rejects it as
/// a cross-transaction reference. This cannot pass until the sweettest
/// relationship adders head-resolve execution-step target tokens (issue #556); it
/// is driven by the `#[ignore]`d `frozen_member_head_redirect_cross_tx_test`.
pub fn frozen_member_head_redirect_cross_tx_fixture() -> Result<DancesTestCase, HolonError> {
    let TestCaseInit { mut test_case, fixture_context, mut fixture_holons, .. } = TestCaseInit::new(
        "frozen_member_head_redirect_cross_tx",
        "Cross-transaction frozen relationship member (pending issue #556)",
    );

    test_case.add_load_core_schema_step(None)?;
    test_case.add_begin_transaction_step(None, None)?;
    test_case.add_load_book_person_inverse_test_schema_step(None)?;

    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction for staged Person and transient Book".to_string()),
    )?;

    let book_type_stub =
        fixture_context.mutation().new_holon(Some(MapString(BOOK_DESCRIPTOR_KEY.to_string())))?;
    let book_type_token = test_case.add_lookup_saved_holon_by_key_step(
        &mut fixture_holons,
        book_type_stub,
        MapString(BOOK_DESCRIPTOR_KEY.to_string()),
        None,
        None,
    )?;
    let person_type_stub =
        fixture_context.mutation().new_holon(Some(MapString(PERSON_DESCRIPTOR_KEY.to_string())))?;
    let person_type_token = test_case.add_lookup_saved_holon_by_key_step(
        &mut fixture_holons,
        person_type_stub,
        MapString(PERSON_DESCRIPTOR_KEY.to_string()),
        None,
        None,
    )?;

    let person_source =
        fixture_context.mutation().new_holon(Some(MapString(PERSON_1_KEY.to_string())))?;
    let mut person_properties = PropertyMap::new();
    person_properties
        .insert("Name".to_property_name(), MapString(PERSON_1_KEY.to_string()).to_base_value());
    let person_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        person_source,
        person_properties,
        Some(MapString(PERSON_1_KEY.to_string())),
        None,
        None,
    )?;
    let person_token =
        test_case.add_stage_holon_step(&mut fixture_holons, person_token, None, None)?;
    let person_token = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        person_token,
        CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
        vec![person_type_token],
        None,
        Some("Describe Person before first commit".to_string()),
    )?;

    let book_source =
        fixture_context.mutation().new_holon(Some(MapString(BOOK_KEY.to_string())))?;
    let mut book_properties = PropertyMap::new();
    book_properties
        .insert("Title".to_property_name(), MapString(BOOK_KEY.to_string()).to_base_value());
    let book_token = test_case.add_new_holon_step(
        &mut fixture_holons,
        book_source,
        book_properties,
        Some(MapString(BOOK_KEY.to_string())),
        None,
        None,
    )?;
    let book_token = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        book_token,
        CoreRelationshipTypeName::DescribedBy.as_relationship_name(),
        vec![book_type_token],
        None,
        Some("Describe transient Book before first commit".to_string()),
    )?;
    let book_token = test_case.add_add_related_holons_step(
        &mut fixture_holons,
        book_token,
        RelationshipName(MapString(BOOK_TO_PERSON_RELATIONSHIP.to_string())),
        vec![person_token],
        None,
        Some("Freeze Book --AuthoredBy--> staged Person before Person commit".to_string()),
    )?;

    test_case.add_commit_step(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        Some("Commit Person while Book remains transient".to_string()),
    )?;

    test_case.add_begin_transaction_step(
        None,
        Some("Begin transaction to stage and commit Book after Person is saved".to_string()),
    )?;
    let _book_token =
        test_case.add_stage_holon_step(&mut fixture_holons, book_token, None, None)?;
    test_case.add_commit_step(
        &mut fixture_holons,
        ExpectedCommitStatus::Complete,
        None,
        Some("Commit Book with frozen Person relationship member".to_string()),
    )?;

    let expected_db_count = MapInteger(
        fixture_holons.count_saved().0
            + CORE_SCHEMA_METRICS.committed
            + BOOK_PERSON_INVERSE_METRICS.committed,
    );
    test_case.add_ensure_database_count_step(expected_db_count, None)?;
    test_case.add_match_saved_content_step()?;

    test_case.finalize(&fixture_context, &fixture_holons)?;

    Ok(test_case)
}
