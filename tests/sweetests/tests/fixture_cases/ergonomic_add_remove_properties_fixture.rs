use holons_core::core_shared_objects::holon::{EssentialHolonContent, EssentialRelationshipMap};
use holons_test::DancesTestCase;
use pretty_assertions::assert_eq;
use std::{collections::BTreeMap, sync::Arc};
use tracing::{error, info};

use holons_prelude::prelude::*;
use rstest::*;

use crate::helpers::init_fixture_context;
use type_names::{CorePropertyTypeName::Description, ToPropertyName};

#[fixture]
pub fn ergonomic_add_remove_properties_fixture() -> Result<DancesTestCase, HolonError> {
    // == Init == //
    let mut test_case = DancesTestCase::new(
        "Ergonomic Add / Remove Holon Properties Testcase".to_string(),
        "Tests the adding and removing of Holon properties using all combinations of ergonomic values".to_string(),
    );
    let fixture_context = init_fixture_context();
    // == //

    // Modifies an existing property and adds a new property, passing Enum, MapString, String, and string literal
    // each through both parameters of PropertyName and BaseValue.
    // Flexes ToPropertyName and ToBaseValue trait combinations.

    // Ergonomics::
    // ToPropertyName : Enum, String, str, MapString, PropertyName  ... complete ✅
    // ToBaseValue : String, str, MapString, int, bool, BaseValue  ... complete ✅
    // Add : Enum, String, str, MapString, PropertyName  ... complete ✅
    // Remove : String, str, MapString, PropertyName  ... complete ✅

    // Creating 'fresh' references for this fixture instead of setup_book_and_authors_fixture

    // === TRANSIENT === //

    // -- ADD -- //
    let book_key = MapString("BOOK_KEY".to_string());
    let mut book_transient_reference = new_holon(&*fixture_context, Some(book_key.clone()))?;
    book_transient_reference.with_property_value(
        &*fixture_context,
        "Description",
        "This is a book description",
    )?;
    // Set expected
    let mut expected_properties = PropertyMap::new();
    expected_properties.insert("Key".to_property_name(), book_key.clone().to_base_value());
    expected_properties
        .insert(Description.to_property_name(), "Changed description".to_string().to_base_value()); // Enum, String
    expected_properties.insert(
        MapString("NewProperty".to_string()).to_property_name(),
        "This is another property".to_base_value(),
    ); // MapString, str
    expected_properties.insert("Int".to_property_name(), 42.to_base_value()); // str, int
    expected_properties.insert("Bool".to_property_name(), true.to_base_value()); // str, bool
    let essential = EssentialHolonContent::new(
        expected_properties.clone(),
        EssentialRelationshipMap::default(),
        Some(book_key.clone()),
        Vec::new(),
    );
    // Modify source
    book_transient_reference
        .with_property_value(&*fixture_context, Description, "Changed description")? // Enum, str
        .with_property_value(
            &*fixture_context,
            "NewProperty".to_string(),
            "This is another property".to_string(),
        )?
        .with_property_value(&*fixture_context, "Int", 42)? // str, int
        .with_property_value(&*fixture_context, "Bool", true)?; // str, bool

    // Assert essential content equal
    assert_eq!(essential, book_transient_reference.essential_content(&*fixture_context)?);

    // -- REMOVE -- //
    let mut expected_properties_after_remove = expected_properties.clone();
    expected_properties_after_remove
        .remove(&MapString("NewProperty".to_string()).to_property_name());
    expected_properties_after_remove.remove(&"Int".to_string().to_property_name());
    expected_properties_after_remove.remove(&"Bool".to_property_name());
    let essential_after_remove = EssentialHolonContent::new(
        expected_properties_after_remove,
        EssentialRelationshipMap::default(),
        Some(book_key.clone()),
        Vec::new(),
    );

    book_transient_reference
        .remove_property_value(&*fixture_context, "NewProperty".to_string())? // String
        .remove_property_value(&*fixture_context, "Int")? // str
        .remove_property_value(&*fixture_context, MapString("Bool".to_string()))?; // MapString

    assert_eq!(
        essential_after_remove,
        book_transient_reference.essential_content(&*fixture_context)?
    );

    // === STAGED === //
    let mut book_staged_reference =
        stage_new_holon(&*fixture_context, book_transient_reference.clone())?;

    // -- ADD -- //
    let mut staged_expected_properties = PropertyMap::new();
    staged_expected_properties.insert("Key".to_property_name(), book_key.clone().to_base_value());
    staged_expected_properties.insert(
        PropertyName(MapString("Description".to_string())).to_property_name(), // PropertyName
        MapString("Another description again".to_string()).to_base_value(),    // MapString
    );
    staged_expected_properties.insert(
        "AnotherProperty".to_string().to_property_name(), // String
        BaseValue::StringValue(MapString("Adding a property".to_string())).to_base_value(), // BaseValue
    );
    let staged_essential = EssentialHolonContent::new(
        staged_expected_properties.clone(),
        EssentialRelationshipMap::default(),
        Some(book_key.clone()),
        Vec::new(),
    );

    book_staged_reference
        .with_property_value(
            &*fixture_context,
            PropertyName(MapString("Description".to_string())), // PropertyName,
            "Another description again".to_string(), // String
        )?
        .with_property_value(
            &*fixture_context,
            MapString("AnotherProperty".to_string()), // MapString
            MapString("Adding a property".to_string()), // MapString
        )?;

    assert_eq!(staged_essential, book_staged_reference.essential_content(&*fixture_context)?);

    // -- REMOVE -- //
    let mut staged_expected_properties_after_remove = staged_expected_properties.clone();
    staged_expected_properties_after_remove.remove(&"Description".to_property_name());
    staged_expected_properties_after_remove.remove(&"AnotherProperty".to_property_name());
    let staged_essential_after_remove = EssentialHolonContent::new(
        staged_expected_properties_after_remove,
        EssentialRelationshipMap::default(),
        Some(book_key.clone()),
        Vec::new(),
    );

    book_staged_reference.remove_property_value(
        &*fixture_context,
        Description, // Enum
    )?;
    book_staged_reference.remove_property_value(
        &*fixture_context,
        PropertyName(MapString("AnotherProperty".to_string())), // PropertyName
    )?;

    assert_eq!(
        staged_essential_after_remove,
        book_staged_reference.essential_content(&*fixture_context)?
    );

    Ok(test_case)
}
