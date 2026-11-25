use std::collections::{BTreeSet, HashMap};

use base_types::{BaseValue, MapBoolean, MapBytes, MapEnumValue, MapInteger, MapString};
use core_types::{HolonError, HolonId, LocalId, PropertyMap, PropertyName, PropertyValue};
use type_names::{CorePropertyTypeName, ToPropertyName};

use crate::core_shared_objects::holon::SavedHolon;
use crate::descriptors::test_support::{
    build_context, build_context_with_read_errors, build_context_with_saved_holons, new_test_holon,
};
use crate::{HolonReference, ReadableHolon, WritableHolon};

fn id(value: u8) -> HolonId {
    HolonId::Local(LocalId(vec![value; 39]))
}

fn string(value: &str) -> PropertyValue {
    BaseValue::StringValue(MapString(value.to_owned()))
}

fn properties(values: &[(&str, PropertyValue)]) -> PropertyMap {
    values.iter().map(|(name, value)| (name.to_property_name(), value.clone())).collect()
}

/// One populated property per value representation, plus the fixture key.
fn populated() -> PropertyMap {
    properties(&[
        ("Key", string("subject")),
        ("Title", string("Example")),
        ("IsActive", BaseValue::BooleanValue(MapBoolean(true))),
        ("Count", BaseValue::IntegerValue(MapInteger(7))),
        ("Status", BaseValue::EnumValue(MapEnumValue(MapString("Open".into())))),
        ("Payload", BaseValue::BytesValue(MapBytes(vec![1, 2, 3]))),
    ])
}

fn saved(value: u8, properties: PropertyMap) -> SavedHolon {
    SavedHolon::new(LocalId(vec![value; 39]), properties, None, MapInteger(1))
}

/// Builds the expected selection from ordinary reads, so parity is with `property_value`.
fn read_individually(
    holon: &impl ReadableHolon,
    names: &[&str],
) -> Result<PropertyMap, HolonError> {
    let mut values = PropertyMap::new();
    for name in names {
        if let Some(value) = holon.property_value(*name)? {
            values.insert(name.to_property_name(), value);
        }
    }
    Ok(values)
}

fn assert_selection_contract(holon: &impl ReadableHolon) -> Result<(), HolonError> {
    let selected = ["Title", "IsActive", "Count", "Status", "Payload"];
    let mut expected = populated();
    // Key is populated but never selected.
    expected.remove(&"Key".to_property_name());
    assert_eq!(holon.project_property_values(selected)?, expected);

    for names in [
        &selected[..],
        &["Title"],
        &["Absent", "Title", "AlsoAbsent", "Count"],
        &["Absent", "AlsoAbsent"],
        &["Title", "Title"],
        &[],
    ] {
        assert_eq!(
            holon.project_property_values(names.iter().copied())?,
            read_individually(holon, names)?,
            "selection {names:?}"
        );
    }

    assert_eq!(
        holon.project_property_values(["Absent", "Title"])?,
        properties(&[("Title", string("Example"))])
    );
    assert!(holon.project_property_values(["Absent", "AlsoAbsent"])?.is_empty());
    assert!(holon.project_property_values([] as [&str; 0])?.is_empty());
    Ok(())
}

#[test]
fn selection_matches_property_value_in_every_phase() -> Result<(), HolonError> {
    let context = build_context_with_saved_holons(vec![saved(1, populated())], HashMap::new());

    let mut transient = new_test_holon(&context, "subject")?;
    for (name, value) in populated() {
        transient.with_property_value(name, value)?;
    }
    let staged = context.mutation().stage_new_holon(transient.clone())?;
    let smart = crate::SmartReference::new_from_id(context.space_read_handle(), id(1));

    assert_selection_contract(&transient)?;
    assert_selection_contract(&staged)?;
    assert_selection_contract(&smart)?;
    assert_selection_contract(&HolonReference::from(transient))?;
    assert_selection_contract(&HolonReference::from(staged))?;
    assert_selection_contract(&HolonReference::from(smart))
}

#[test]
fn duplicate_converted_names_yield_one_entry() -> Result<(), HolonError> {
    let context = build_context();
    let mut holon = new_test_holon(&context, "subject")?;
    holon.with_property_value("Title", "Example")?;
    let expected = properties(&[("Title", string("Example"))]);

    assert_eq!(holon.project_property_values(["Title", "Title"])?, expected);
    // Distinct spellings canonicalize to the same property name.
    assert_eq!(holon.project_property_values(["title", "Title"])?, expected);
    Ok(())
}

#[test]
fn every_property_name_form_is_accepted() -> Result<(), HolonError> {
    let context = build_context();
    let mut holon = new_test_holon(&context, "subject")?;
    holon.with_property_value(CorePropertyTypeName::TypeName, "Example")?;
    let expected = properties(&[("TypeName", string("Example"))]);

    let text = "TypeName";
    let map_string = MapString(text.into());
    let property_name = PropertyName(map_string.clone());

    // Owned items, in arrays, vectors and other iterables.
    assert_eq!(holon.project_property_values([text])?, expected);
    assert_eq!(holon.project_property_values(vec![text])?, expected);
    assert_eq!(holon.project_property_values(vec![text.to_string()])?, expected);
    assert_eq!(holon.project_property_values([map_string.clone()])?, expected);
    assert_eq!(holon.project_property_values([property_name.clone()])?, expected);
    assert_eq!(holon.project_property_values([CorePropertyTypeName::TypeName])?, expected);

    // Borrowed collections whose item references are convertible.
    let map_strings = vec![map_string];
    let property_names = [property_name];
    let ordered: BTreeSet<PropertyName> = property_names.iter().cloned().collect();
    assert_eq!(holon.project_property_values(&map_strings)?, expected);
    assert_eq!(holon.project_property_values(&property_names)?, expected);
    assert_eq!(holon.project_property_values(&ordered)?, expected);
    assert_eq!(holon.project_property_values(expected.keys())?, expected);
    assert_eq!(holon.project_property_values(&[CorePropertyTypeName::TypeName])?, expected);

    // Documented forms for slices whose item references are not convertible.
    let borrowed_text: &[&str] = &[text];
    let borrowed_strings: &[String] = &[text.to_string()];
    assert_eq!(holon.project_property_values(borrowed_text.iter().copied())?, expected);
    assert_eq!(holon.project_property_values(borrowed_strings.iter().cloned())?, expected);
    Ok(())
}

#[test]
fn name_conversions_are_not_normalized_further() -> Result<(), HolonError> {
    let context = build_context();
    let mut holon = new_test_holon(&context, "subject")?;
    holon.with_property_value("SomeName", "Example")?;

    // `&str` is canonicalized; owned strings pass through unchanged.
    assert_eq!(
        holon.project_property_values(["some_name"])?,
        properties(&[("SomeName", string("Example"))])
    );
    assert!(holon.property_value("some_name")?.is_some());

    assert!(holon.project_property_values([String::from("some_name")])?.is_empty());
    assert!(holon.property_value(String::from("some_name"))?.is_none());

    assert!(holon.project_property_values([MapString("some_name".into())])?.is_empty());
    assert!(holon.property_value(MapString("some_name".into()))?.is_none());
    Ok(())
}

#[test]
fn cached_smart_values_need_no_resolvable_backing_holon() -> Result<(), HolonError> {
    let context = build_context();
    let cached = properties(&[("Title", string("Cached")), ("Count", string("3"))]);
    let reference =
        HolonReference::smart_with_properties(context.space_read_handle(), id(9), cached.clone());

    assert_eq!(reference.project_property_values(["Title", "Count"])?, cached);
    Ok(())
}

#[test]
fn cached_smart_values_take_precedence_over_the_backing_holon() -> Result<(), HolonError> {
    let backing = properties(&[("Title", string("Recorded")), ("Subtitle", string("Backing"))]);
    let context = build_context_with_saved_holons(vec![saved(1, backing)], HashMap::new());
    let reference = HolonReference::smart_with_properties(
        context.space_read_handle(),
        id(1),
        properties(&[("Title", string("Cached"))]),
    );

    // Uncached names still fall back to the backing holon.
    let names = ["Title", "Subtitle"];
    let expected = properties(&[("Title", string("Cached")), ("Subtitle", string("Backing"))]);
    assert_eq!(reference.project_property_values(names)?, expected);
    assert_eq!(read_individually(&reference, &names)?, expected);
    Ok(())
}

#[test]
fn a_failed_read_fails_the_whole_selection() {
    let context = build_context();
    let unresolvable = HolonReference::smart_with_properties(
        context.space_read_handle(),
        id(9),
        properties(&[("Title", string("Cached"))]),
    );
    let expected = unresolvable.property_value("Uncached").unwrap_err();
    assert!(matches!(expected, HolonError::HolonNotFound(_)));
    assert_eq!(unresolvable.project_property_values(["Uncached"]), Err(expected.clone()));
    // The successful cached read must not survive as a partial result.
    assert_eq!(unresolvable.project_property_values(["Title", "Uncached"]), Err(expected));

    let injected = HolonError::InvalidState("content read forbidden".into());
    let context = build_context_with_read_errors(
        vec![saved(1, populated())],
        HashMap::new(),
        HashMap::from([(id(1), injected.clone())]),
    );
    let failing = HolonReference::smart_with_properties(
        context.space_read_handle(),
        id(1),
        properties(&[("Title", string("Cached"))]),
    );
    assert_eq!(failing.project_property_values(["Title", "Count"]), Err(injected));
}

#[test]
fn an_empty_selection_reads_nothing() -> Result<(), HolonError> {
    let context = build_context();
    let unresolvable = HolonReference::smart_from_id(context.space_read_handle(), id(9));
    assert!(unresolvable.property_value("Title").is_err());

    assert!(unresolvable.project_property_values([] as [&str; 0])?.is_empty());
    assert!(unresolvable.project_property_values(Vec::<PropertyName>::new())?.is_empty());
    Ok(())
}

#[test]
fn the_returned_map_is_independent_of_its_source() -> Result<(), HolonError> {
    let context = build_context();
    let mut holon = new_test_holon(&context, "subject")?;
    holon.with_property_value("Title", "Original")?;

    let mut values = holon.project_property_values(["Title"])?;
    values.insert("Title".to_property_name(), string("Edited copy"));
    values.insert("Extra".to_property_name(), string("Added"));
    assert_eq!(holon.property_value("Title")?, Some(string("Original")));
    assert_eq!(holon.property_value("Extra")?, None);

    let before = holon.project_property_values(["Title"])?;
    holon.with_property_value("Title", "Changed source")?;
    assert_eq!(before, properties(&[("Title", string("Original"))]));
    Ok(())
}
