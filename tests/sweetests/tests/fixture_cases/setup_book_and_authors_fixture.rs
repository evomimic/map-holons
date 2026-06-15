use holons_prelude::prelude::*;
use holons_test::{DancesTestCase, FixtureBindings, FixtureHolons};
use std::collections::BTreeMap;
use std::sync::Arc;

use holons_test::harness::helpers::{
    BOOK_KEY, BOOK_TO_PERSON_RELATIONSHIP, PERSON_1_KEY, PERSON_2_KEY, PUBLISHER_KEY,
};

/// Adds undescribed, freeform Book/Person/Publisher holons to `test_case`.
///
/// This helper stages four ordinary holons and intentionally does not attach
/// `DescribedBy` descriptors. It also does not stage any relationships. With
/// commit-time relationship validation, this remains useful for fixtures that
/// exercise property or lifecycle behavior without graph state.
///
/// Specifically, this function adds test steps that will stage:
/// * Book holon with `BOOK_KEY` Title
/// * Person holon with `PERSON_1_KEY`
/// * Person holon with `PERSON_2_KEY`
/// * Publisher holon with `PUBLISHER_KEY`
///
/// The Nursery within the supplied context is used as the test data setup area.
///
/// FixtureHolons contains the minted token TestReferences that are used to track a lineage of state to mirror in parallel the ExecutionHolons.
/// This parallel reflects 'expected' (Fixture) vs 'actual' (Mock DHT).
pub fn setup_undescribed_book_people_publisher_steps_with_context<'a>(
    fixture_context: &Arc<TransactionContext>,
    test_case: &mut DancesTestCase,
    fixture_holons: &mut FixtureHolons,
    bindings: &'a mut FixtureBindings,
) -> Result<&'a mut FixtureBindings, HolonError> {
    //  STAGE:  Book Holon  //
    //
    // Create fresh holon
    let book_label = MapString("Book".to_string());
    let book_key = MapString(BOOK_KEY.to_string());
    let book_transient_reference = fixture_context.mutation().new_holon(Some(book_key.clone()))?;

    // Mint
    let mut book_properties = BTreeMap::new();
    book_properties.insert("Title".to_property_name(), BOOK_KEY.to_base_value());
    book_properties.insert("Description".to_property_name(), "Why is there so much chaos and suffering in the world today? Are we sliding towards dystopia and perhaps extinction, or is there hope for a better future?".to_base_value());

    let book_transient_token = test_case.add_new_holon_step(
        fixture_holons,
        book_transient_reference,
        book_properties,
        Some(book_key.clone()),
        None,
        Some(format!("Creating {:?} holon...", book_label)),
    )?;
    // Stage & bind with label
    let book_staged_token = test_case.add_stage_holon_step(
        fixture_holons,
        book_transient_token,
        None,
        Some(format!("Staging {:?} holon...", book_label)),
    )?;
    bindings.insert_token(book_label.clone(), book_staged_token.clone());

    // //  STAGE:  Person 1 //
    //
    // Create
    let person_1_label = MapString("Person1".to_string());
    let person_1_key = MapString(PERSON_1_KEY.to_string());
    let person_1_transient_reference =
        fixture_context.mutation().new_holon(Some(person_1_key.clone()))?;

    let mut person_1_properties = BTreeMap::new();
    person_1_properties.insert("first name".to_property_name(), "Roger".to_base_value());
    person_1_properties.insert("last name".to_property_name(), "Briggs".to_base_value());

    let person_1_transient_token = test_case.add_new_holon_step(
        fixture_holons,
        person_1_transient_reference,
        person_1_properties,
        Some(person_1_key.clone()),
        None,
        Some(format!("Creating {:?} holon...", person_1_label)),
    )?;

    let person_1_staged_token = test_case.add_stage_holon_step(
        fixture_holons,
        person_1_transient_token,
        None,
        Some(format!("Staging {:?} holon...", person_1_label)),
    )?;
    bindings.insert_token(person_1_label.clone(), person_1_staged_token.clone());

    //  STAGE:  Person 2 //
    //
    // Create
    let person_2_label = MapString("Person2".to_string());
    let person_2_key = MapString(PERSON_2_KEY.to_string());
    let person_2_transient_reference =
        fixture_context.mutation().new_holon(Some(person_2_key.clone()))?;

    let mut person_2_properties = BTreeMap::new();
    person_2_properties.insert("first name".to_property_name(), "George".to_base_value());
    person_2_properties.insert("last name".to_property_name(), "Smith".to_base_value());

    let person_2_transient_token = test_case.add_new_holon_step(
        fixture_holons,
        person_2_transient_reference,
        person_2_properties,
        Some(person_2_key.clone()),
        None,
        Some(format!("Creating {:?} holon...", person_2_label.0)),
    )?;

    let person_2_staged_token = test_case.add_stage_holon_step(
        fixture_holons,
        person_2_transient_token,
        None,
        Some(format!("Staging {:?} holon...", person_2_label)),
    )?;
    bindings.insert_token(person_2_label.clone(), person_2_staged_token.clone());

    //  STAGE:  Publisher //
    //
    // Create
    let publisher_label = MapString("Publisher".to_string());
    let publisher_key = MapString(PUBLISHER_KEY.to_string());
    let publisher_transient_reference =
        fixture_context.mutation().new_holon(Some(publisher_key.clone()))?;

    let mut publisher_properties = BTreeMap::new();
    publisher_properties.insert("name".to_property_name(), PUBLISHER_KEY.to_base_value());
    publisher_properties.insert(
        "Description".to_property_name(),
        "We publish Holons for testing purposes".to_base_value(),
    );

    let publisher_transient_token = test_case.add_new_holon_step(
        fixture_holons,
        publisher_transient_reference,
        publisher_properties,
        Some(publisher_key.clone()),
        None,
        Some(format!("Creating {:?} holon...", publisher_label)),
    )?;

    let publisher_staged_token = test_case.add_stage_holon_step(
        fixture_holons,
        publisher_transient_token,
        None,
        Some(format!("Staging {:?} holon...", publisher_label)),
    )?;
    bindings.insert_token(publisher_label, publisher_staged_token.clone());

    Ok(bindings)
}

/// Adds an undescribed, freeform Book/Person/Publisher setup with `AuthoredBy`.
///
/// This helper layers a freeform `BOOK_TO_PERSON_RELATIONSHIP` edge on top of
/// [`setup_undescribed_book_people_publisher_steps_with_context`]. It is valid
/// for non-committing ergonomics tests and expected-failure tests that
/// deliberately exercise freeform staged relationship mutation.
///
/// Fixtures that expect relationship commit success should not use this helper;
/// they need a core-schema-backed described setup instead.
pub fn setup_undescribed_book_author_steps_with_context<'a>(
    fixture_context: &Arc<TransactionContext>,
    test_case: &mut DancesTestCase,
    fixture_holons: &mut FixtureHolons,
    bindings: &'a mut FixtureBindings,
) -> Result<&'a mut FixtureBindings, HolonError> {
    setup_undescribed_book_people_publisher_steps_with_context(
        fixture_context,
        test_case,
        fixture_holons,
        bindings,
    )?;

    // Set relationship
    let relationship_label = MapString("BOOK_TO_PERSON".to_string());
    let relationship_name = BOOK_TO_PERSON_RELATIONSHIP.to_relationship_name();
    bindings.set_relationship_name(relationship_label, relationship_name.clone());

    let book_label = MapString("Book".to_string());
    let person_1_label = MapString("Person1".to_string());
    let person_2_label = MapString("Person2".to_string());
    let book_staged_token =
        bindings.get_token(&book_label).expect("undescribed book setup must bind Book").clone();
    let person_1_staged_token = bindings
        .get_token(&person_1_label)
        .expect("undescribed book setup must bind Person1")
        .clone();
    let person_2_staged_token = bindings
        .get_token(&person_2_label)
        .expect("undescribed book setup must bind Person2")
        .clone();

    //  RELATIONSHIP:  (Book)-AUTHORED_BY->[(Person1),(Person2)]  //
    let book_mod_token = test_case.add_add_related_holons_step(
        fixture_holons,
        book_staged_token,
        relationship_name,
        vec![person_1_staged_token, person_2_staged_token],
        None,
        Some(format!(
            "Adding Relationships: {:?} -> {:?} -> [{:?}, {:?}] ",
            book_label, BOOK_TO_PERSON_RELATIONSHIP, person_1_label, person_2_label
        )),
    )?;
    // Update bindings to reflect the added relationships
    bindings.insert_token(book_label, book_mod_token);

    Ok(bindings)
}
