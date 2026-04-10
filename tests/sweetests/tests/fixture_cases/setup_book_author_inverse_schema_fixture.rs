use super::setup_book_author_steps_with_context;
use holons_prelude::prelude::*;
use holons_test::harness::helpers::{
    BOOK_DESCRIPTOR_KEY, BOOK_TO_PERSON_RELATIONSHIP, BOOK_TO_PERSON_RELATIONSHIP_KEY,
    PERSON_1_KEY, PERSON_2_KEY, PERSON_DESCRIPTOR_KEY, PERSON_TO_BOOK_REL_INVERSE,
    PERSON_TO_BOOK_RELATIONSHIP_INVERSE_KEY,
};
use holons_test::{DancesTestCase, FixtureBindings, FixtureHolons, TestReference};
use std::collections::BTreeMap;
use std::sync::Arc;

fn stage_schema_descriptor(
    fixture_context: &Arc<TransactionContext>,
    test_case: &mut DancesTestCase,
    fixture_holons: &mut FixtureHolons,
    key: &str,
    type_name: &str,
    description: String,
) -> Result<TestReference, HolonError> {
    let transient_reference =
        fixture_context.mutation().new_holon(Some(MapString(key.to_string())))?;

    let mut properties = BTreeMap::new();
    properties.insert(
        CorePropertyTypeName::TypeName.as_property_name(),
        BaseValue::StringValue(MapString(type_name.to_string())),
    );

    let transient_token = test_case.add_new_holon_step(
        fixture_holons,
        transient_reference,
        properties,
        Some(MapString(key.to_string())),
        None,
        Some(format!("Creating {description} descriptor...")),
    )?;

    test_case.add_stage_holon_step(
        fixture_holons,
        transient_token,
        None,
        Some(format!("Staging {description} descriptor...")),
    )
}

pub fn setup_book_author_inverse_schema_steps_with_context<'a>(
    fixture_context: &Arc<TransactionContext>,
    test_case: &mut DancesTestCase,
    fixture_holons: &mut FixtureHolons,
    bindings: &'a mut FixtureBindings,
) -> Result<&'a mut FixtureBindings, HolonError> {
    setup_book_author_steps_with_context(fixture_context, test_case, fixture_holons, bindings)?;

    let authored_by_relationship = BOOK_TO_PERSON_RELATIONSHIP.to_relationship_name();
    let authors_inverse_relationship = PERSON_TO_BOOK_REL_INVERSE.to_relationship_name();
    let described_by_relationship = CoreRelationshipTypeName::DescribedBy.as_relationship_name();
    let instance_relationships =
        CoreRelationshipTypeName::InstanceRelationships.as_relationship_name();
    let has_inverse_relationship = CoreRelationshipTypeName::HasInverse.as_relationship_name();

    bindings.set_relationship_name(
        MapString("PERSON_TO_BOOK".to_string()),
        authors_inverse_relationship.clone(),
    );

    let book_token = bindings
        .get_token(&MapString("Book".to_string()))
        .expect("Expected Book token in setup bindings")
        .clone();
    let person_1_token = bindings
        .get_token(&MapString("Person1".to_string()))
        .expect("Expected Person1 token in setup bindings")
        .clone();
    let person_2_token = bindings
        .get_token(&MapString("Person2".to_string()))
        .expect("Expected Person2 token in setup bindings")
        .clone();

    let book_type_token = stage_schema_descriptor(
        fixture_context,
        test_case,
        fixture_holons,
        BOOK_DESCRIPTOR_KEY,
        "Book",
        "BookType".to_string(),
    )?;
    bindings.insert_token(MapString("BookType".to_string()), book_type_token.clone());

    let person_type_token = stage_schema_descriptor(
        fixture_context,
        test_case,
        fixture_holons,
        PERSON_DESCRIPTOR_KEY,
        "Person",
        "PersonType".to_string(),
    )?;
    bindings.insert_token(MapString("PersonType".to_string()), person_type_token.clone());

    let authored_by_descriptor_token = stage_schema_descriptor(
        fixture_context,
        test_case,
        fixture_holons,
        BOOK_TO_PERSON_RELATIONSHIP_KEY,
        authored_by_relationship.0.0.as_str(),
        "AuthoredByRelationship".to_string(),
    )?;
    bindings.insert_token(
        MapString("AuthoredByDescriptor".to_string()),
        authored_by_descriptor_token.clone(),
    );

    let authors_inverse_descriptor_token = stage_schema_descriptor(
        fixture_context,
        test_case,
        fixture_holons,
        PERSON_TO_BOOK_RELATIONSHIP_INVERSE_KEY,
        authors_inverse_relationship.0.0.as_str(),
        "AuthorsInverseRelationship".to_string(),
    )?;
    bindings.insert_token(
        MapString("AuthorsInverseDescriptor".to_string()),
        authors_inverse_descriptor_token.clone(),
    );

    let authored_by_with_inverse = test_case.add_add_related_holons_step(
        fixture_holons,
        authored_by_descriptor_token,
        has_inverse_relationship,
        vec![authors_inverse_descriptor_token.clone()],
        None,
        Some("Declaring AuthoredBy -> HasInverse -> Authors".to_string()),
    )?;
    bindings.insert_token(
        MapString("AuthoredByDescriptor".to_string()),
        authored_by_with_inverse.clone(),
    );

    let book_type_with_relationships = test_case.add_add_related_holons_step(
        fixture_holons,
        book_type_token,
        instance_relationships,
        vec![authored_by_with_inverse],
        None,
        Some("Declaring BookType -> InstanceRelationships -> AuthoredBy".to_string()),
    )?;
    bindings.insert_token(
        MapString("BookType".to_string()),
        book_type_with_relationships.clone(),
    );

    let book_with_descriptor = test_case.add_add_related_holons_step(
        fixture_holons,
        book_token,
        described_by_relationship.clone(),
        vec![book_type_with_relationships],
        None,
        Some("Typing Book with BookType descriptor".to_string()),
    )?;
    bindings.insert_token(MapString("Book".to_string()), book_with_descriptor);

    let person_1_with_descriptor = test_case.add_add_related_holons_step(
        fixture_holons,
        person_1_token,
        described_by_relationship.clone(),
        vec![person_type_token.clone()],
        None,
        Some(format!("Typing {} with PersonType descriptor", PERSON_1_KEY)),
    )?;
    bindings.insert_token(MapString("Person1".to_string()), person_1_with_descriptor);

    let person_2_with_descriptor = test_case.add_add_related_holons_step(
        fixture_holons,
        person_2_token,
        described_by_relationship,
        vec![person_type_token],
        None,
        Some(format!("Typing {} with PersonType descriptor", PERSON_2_KEY)),
    )?;
    bindings.insert_token(MapString("Person2".to_string()), person_2_with_descriptor);

    Ok(bindings)
}
