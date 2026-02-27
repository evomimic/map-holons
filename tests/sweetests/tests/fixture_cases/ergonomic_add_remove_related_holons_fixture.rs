use holons_core::{core_shared_objects::holon::EssentialRelationshipMap, CollectionState};
use holons_prelude::prelude::*;
use holons_test::{
    DancesTestCase, TestCaseInit, BOOK_KEY, BOOK_TO_PERSON_RELATIONSHIP, EDITOR_FOR, PERSON_1_KEY,
    PERSON_2_KEY, PUBLISHED_BY, PUBLISHER_KEY,
};
use pretty_assertions::assert_eq;
use rstest::*;
use std::collections::BTreeMap;
use type_names::{CoreRelationshipTypeName::DescribedBy, ToRelationshipName};

#[fixture]
pub fn ergonomic_add_remove_related_holons_fixture() -> Result<DancesTestCase, HolonError> {
    // == Init == //
    let TestCaseInit { mut test_case, fixture_context, fixture_holons: _fixture_holons, fixture_bindings: _fixture_bindings } = TestCaseInit::new(
        "Ergonomic Add / Remove Related Holons Testcase".to_string(),
        "Tests the adding and removing of related Holons, using all combinations of ergonomic relationship names, for both Transient & Staged Holons".to_string(),
    );
    let _published_by = RelationshipName(MapString(PUBLISHED_BY.to_string()));
    // == //

    // Add : Enum, String, str, MapString, RelationshipName  ... complete ✅
    // Remove : String, str, MapString, RelationshipName  ... complete ✅

    // Creating 'fresh' references for this fixture instead of setup_book_and_authors_fixture

    // === TRANSIENT === //
    //
    let book_key = MapString(BOOK_KEY.to_string());
    let person_1_key = MapString(PERSON_1_KEY.to_string());
    let person_2_key = MapString(PERSON_2_KEY.to_string());
    let publisher_key = MapString(PUBLISHER_KEY.to_string());
    let descriptor_key = MapString("DESCRIPTOR_KEY".to_string());
    let mut book_transient_reference = new_holon(&fixture_context, Some(book_key.clone()))?;
    let person_1_transient_reference = new_holon(&fixture_context, Some(person_1_key.clone()))?;
    let person_2_transient_reference = new_holon(&fixture_context, Some(person_2_key.clone()))?;
    let publisher_transient_reference = new_holon(&fixture_context, Some(publisher_key.clone()))?;
    let descriptor_transient_reference = new_holon(&fixture_context, Some(descriptor_key.clone()))?;
    // -- ADD -- //
    //
    // Book
    book_transient_reference
        .add_related_holons(
            PUBLISHED_BY, // str
            vec![HolonReference::Transient(publisher_transient_reference.clone())],
        )?
        .add_related_holons(
            BOOK_TO_PERSON_RELATIONSHIP.to_string(), // String
            vec![
                HolonReference::Transient(person_1_transient_reference.clone()),
                HolonReference::Transient(person_2_transient_reference.clone()),
            ],
        )?
        .add_related_holons(
            DescribedBy, // Enum
            vec![HolonReference::Transient(descriptor_transient_reference.clone())],
        )?;
    // Set expected
    let mut book_properties = BTreeMap::new();
    book_properties.insert("Key".to_property_name(), BOOK_KEY.to_base_value());
    let mut book_expected_relationships = EssentialRelationshipMap::default();
    book_expected_relationships.add_related_holons(
        CollectionState::Transient,
        PUBLISHED_BY.to_relationship_name(), // str
        vec![HolonReference::Transient(publisher_transient_reference.clone())],
    )?;
    book_expected_relationships.add_related_holons(
        CollectionState::Transient,
        BOOK_TO_PERSON_RELATIONSHIP.to_string().to_relationship_name(), // String
        vec![
            HolonReference::Transient(person_1_transient_reference.clone()),
            HolonReference::Transient(person_2_transient_reference.clone()),
        ],
    )?;
    book_expected_relationships.add_related_holons(
        CollectionState::Transient,
        DescribedBy.to_relationship_name(), // Enum
        vec![HolonReference::Transient(descriptor_transient_reference.clone())],
    )?;

    // Assert essential content equal
    assert_eq!(book_expected_relationships, book_transient_reference.all_related_holons()?.into());
    // -- //

    // -- REMOVE -- //
    //
    // Mod
    book_transient_reference
        .remove_related_holons(
            PUBLISHED_BY, // str
            vec![HolonReference::Transient(publisher_transient_reference.clone())],
        )?
        .remove_related_holons(
            BOOK_TO_PERSON_RELATIONSHIP.to_string(), // String
            vec![HolonReference::Transient(person_1_transient_reference.clone())],
        )?
        .remove_related_holons(
            DescribedBy, // Enum
            vec![HolonReference::Transient(descriptor_transient_reference.clone())],
        )?;
    // Expected
    book_expected_relationships.remove_related_holons(
        &PUBLISHED_BY.to_relationship_name(), // str
        vec![HolonReference::Transient(publisher_transient_reference.clone())],
    )?;
    book_expected_relationships.remove_related_holons(
        &BOOK_TO_PERSON_RELATIONSHIP.to_string().to_relationship_name(), // String
        vec![HolonReference::Transient(person_1_transient_reference.clone())],
    )?;
    book_expected_relationships.remove_related_holons(
        &DescribedBy.to_relationship_name(), // Enum
        vec![HolonReference::Transient(descriptor_transient_reference.clone())],
    )?;
    // Assert
    assert_eq!(book_expected_relationships, book_transient_reference.all_related_holons()?.into());

    // == //

    // === STAGED === //
    //
    let mut person_1_staged_reference =
        stage_new_holon(&fixture_context, person_1_transient_reference.clone())?;
    let publisher_staged_reference =
        stage_new_holon(&fixture_context, publisher_transient_reference.clone())?;
    let descriptor_staged_reference =
        stage_new_holon(&fixture_context, descriptor_transient_reference.clone())?;

    // -- ADD -- //
    //
    // Source
    person_1_staged_reference
        .add_related_holons(
            MapString(EDITOR_FOR.to_string()), // MapString
            vec![HolonReference::Staged(publisher_staged_reference.clone())],
        )?
        .add_related_holons(
            RelationshipName(MapString("DescribedBy".to_string())), // RelationshipName
            vec![HolonReference::Staged(descriptor_staged_reference.clone())],
        )?;
    // Expected
    let mut person_1_properties = BTreeMap::new();
    person_1_properties.insert("Key".to_property_name(), person_1_key.clone().to_base_value());
    let mut staged_person_1_expected_relationships = EssentialRelationshipMap::default();
    staged_person_1_expected_relationships.add_related_holons(
        CollectionState::Staged,
        EDITOR_FOR.to_relationship_name(),
        vec![HolonReference::Staged(publisher_staged_reference.clone())],
    )?;
    staged_person_1_expected_relationships.add_related_holons(
        CollectionState::Staged,
        DescribedBy.to_relationship_name(),
        vec![HolonReference::Staged(descriptor_staged_reference.clone())],
    )?;
    // Assert
    assert_eq!(
        staged_person_1_expected_relationships,
        person_1_staged_reference.all_related_holons()?.into()
    );

    // -- //

    // -- REMOVE -- //
    person_1_staged_reference
        .remove_related_holons(
            MapString(EDITOR_FOR.to_string()), // MapString
            vec![HolonReference::Staged(publisher_staged_reference.clone())],
        )?
        .remove_related_holons(
            RelationshipName(MapString("DescribedBy".to_string())), // RelationshipName
            vec![HolonReference::Staged(descriptor_staged_reference.clone())],
        )?;
    // Expected
    staged_person_1_expected_relationships.remove_related_holons(
        &MapString(EDITOR_FOR.to_string()).to_relationship_name(), // MapString
        vec![HolonReference::Staged(publisher_staged_reference.clone())],
    )?;
    staged_person_1_expected_relationships.remove_related_holons(
        &RelationshipName(MapString("DescribedBy".to_string())).to_relationship_name(), // RelationshipName
        vec![HolonReference::Staged(descriptor_staged_reference.clone())],
    )?;
    // Assert
    assert_eq!(
        staged_person_1_expected_relationships,
        person_1_staged_reference.all_related_holons()?.into()
    );

    // == //

    // Finalize
    test_case.finalize(&fixture_context)?;

    Ok(test_case)
}
