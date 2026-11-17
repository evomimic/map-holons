use base_types::{BaseValue, MapBoolean, MapBytes, MapEnumValue, MapInteger, MapString};
use holons_core::descriptors::value_constraints::{
    Bound, ConstraintEvaluationOutcome as Outcome, ConstraintMeasure, ConstraintParameters,
    IntegerInterval, MeasureBounds, ScalarConstraintFamily, ScalarConstraintParameters,
    BYTES_LENGTH_OUT_OF_RANGE, NUMERIC_VALUE_OUT_OF_RANGE, STRING_LENGTH_OUT_OF_RANGE,
};
use unicode_segmentation::{UnicodeSegmentation, UNICODE_VERSION};

fn bound<T>(value: T, inclusive: bool) -> Option<Bound<T>> {
    Some(Bound { value, inclusive })
}

fn integer(value: i64) -> BaseValue {
    BaseValue::IntegerValue(MapInteger(value))
}

fn string(value: &str) -> BaseValue {
    BaseValue::StringValue(MapString(value.to_owned()))
}

fn assert_result(outcome: Outcome, satisfied: bool, code: &str, measure: ConstraintMeasure) {
    match outcome {
        Outcome::Satisfied => assert!(satisfied),
        Outcome::Violated(detail) => {
            assert!(!satisfied, "{detail}");
            assert_eq!(detail.code, code);
            assert_eq!(detail.measure, measure);
        }
        Outcome::IncompatibleValueKind => panic!("compatible value was declined"),
    }
}

#[test]
fn numeric_bounds_matrix_and_extrema() {
    // Exercise the complete endpoint matrix against an independent interval predicate.
    for minimum in [None, bound(-5, false), bound(-5, true)] {
        for maximum in [None, bound(5, false), bound(5, true)] {
            let parameters =
                ScalarConstraintParameters::NumericRange(IntegerInterval { minimum, maximum });
            for value in [i64::MIN, -6, -5, -4, 0, 4, 5, 6, i64::MAX] {
                let expected = match minimum {
                    None => true,
                    Some(Bound { inclusive: true, .. }) => value >= -5,
                    Some(_) => value > -5,
                } && match maximum {
                    None => true,
                    Some(Bound { inclusive: true, .. }) => value <= 5,
                    Some(_) => value < 5,
                };
                assert_result(
                    parameters.evaluate(&integer(value)),
                    expected,
                    NUMERIC_VALUE_OUT_OF_RANGE,
                    ConstraintMeasure::Integer(value),
                );
            }
        }
    }
    for (minimum, maximum, accepted) in [
        (bound(-5, true), bound(-5, true), Some(-5)),
        (bound(0, false), bound(1, false), None),
        (bound(i64::MIN, true), bound(i64::MIN, true), Some(i64::MIN)),
        (bound(i64::MAX, true), bound(i64::MAX, true), Some(i64::MAX)),
        (None, bound(i64::MIN, false), None),
        (bound(i64::MAX, false), None, None),
    ] {
        for value in [i64::MIN, i64::MIN + 1, -6, -5, -4, -1, 0, 1, 2, i64::MAX - 1, i64::MAX] {
            assert_result(
                ScalarConstraintParameters::NumericRange(IntegerInterval { minimum, maximum })
                    .evaluate(&integer(value)),
                accepted == Some(value),
                NUMERIC_VALUE_OUT_OF_RANGE,
                ConstraintMeasure::Integer(value),
            );
        }
    }
    let empty = IntegerInterval { minimum: bound(0, false), maximum: bound(1, false) };
    for value in -100..=100 {
        assert!(matches!(
            ScalarConstraintParameters::NumericRange(empty).evaluate(&integer(value)),
            Outcome::Violated(_)
        ));
    }
    let all = IntegerInterval { minimum: bound(i64::MIN, true), maximum: bound(i64::MAX, true) };
    for value in [i64::MIN, 0, i64::MAX] {
        assert_eq!(
            ScalarConstraintParameters::NumericRange(all).evaluate(&integer(value)),
            Outcome::Satisfied
        );
    }
}

#[test]
fn length_bounds_matrix_for_both_representations() {
    for minimum in [None, bound(2, false), bound(2, true)] {
        for maximum in [None, bound(4, false), bound(4, true)] {
            let bounds = MeasureBounds { minimum, maximum };
            for length in 0..=6 {
                let expected = minimum
                    .is_none_or(|b| length > b.value || (b.inclusive && length == b.value))
                    && maximum
                        .is_none_or(|b| length < b.value || (b.inclusive && length == b.value));
                for (parameters, value, code) in [
                    (
                        ScalarConstraintParameters::StringLength(bounds),
                        string(&"a".repeat(length as usize)),
                        STRING_LENGTH_OUT_OF_RANGE,
                    ),
                    (
                        ScalarConstraintParameters::BytesLength(bounds),
                        BaseValue::BytesValue(MapBytes(vec![0; length as usize])),
                        BYTES_LENGTH_OUT_OF_RANGE,
                    ),
                ] {
                    assert_result(
                        parameters.evaluate(&value),
                        expected,
                        code,
                        ConstraintMeasure::Length(length),
                    );
                }
            }
        }
    }
    for (minimum, maximum, accepted, all) in [
        (bound(0, true), bound(0, true), Some(0), false),
        (bound(2, true), bound(2, true), Some(2), false),
        (bound(0, false), bound(1, false), None, false),
        (bound(u64::MAX, false), None, None, false),
        (None, bound(u64::MAX, true), None, true),
    ] {
        let bounds = MeasureBounds { minimum, maximum };
        for length in 0..=6 {
            let expected = all || accepted == Some(length);
            assert_result(
                ScalarConstraintParameters::StringLength(bounds)
                    .evaluate(&string(&"a".repeat(length))),
                expected,
                STRING_LENGTH_OUT_OF_RANGE,
                ConstraintMeasure::Length(length as u64),
            );
            assert_result(
                ScalarConstraintParameters::BytesLength(bounds)
                    .evaluate(&BaseValue::BytesValue(MapBytes(vec![0; length]))),
                expected,
                BYTES_LENGTH_OUT_OF_RANGE,
                ConstraintMeasure::Length(length as u64),
            );
        }
    }
}

#[test]
fn every_incompatible_native_variant_declines() {
    let values = [
        string("1"),
        integer(1),
        BaseValue::BytesValue(MapBytes(vec![1])),
        BaseValue::BooleanValue(MapBoolean(true)),
        BaseValue::EnumValue(MapEnumValue(MapString("1".into()))),
    ];
    let bounds = MeasureBounds { minimum: None, maximum: bound(10, true) };
    let numeric = IntegerInterval { minimum: None, maximum: bound(10, true) };
    for (index, value) in values.iter().enumerate() {
        for (own_index, outcome) in [
            (0, ScalarConstraintParameters::StringLength(bounds).evaluate(value)),
            (1, ScalarConstraintParameters::NumericRange(numeric).evaluate(value)),
            (2, ScalarConstraintParameters::BytesLength(bounds).evaluate(value)),
        ] {
            assert_eq!(
                outcome,
                if index == own_index {
                    Outcome::Satisfied
                } else {
                    Outcome::IncompatibleValueKind
                }
            );
        }
    }
}

#[test]
fn focused_graphemes_and_bytes_diverge() {
    for (text, clusters) in
        [("", 0), ("a\u{301}", 1), ("👩‍👩‍👧‍👦", 1), ("🇺🇸🇨🇦🇬", 3), ("é", 1), ("e\u{301}", 1), ("\r\n", 1)]
    {
        let exact =
            MeasureBounds { minimum: bound(clusters, true), maximum: bound(clusters, true) };
        assert_eq!(
            ScalarConstraintParameters::StringLength(exact).evaluate(&string(text)),
            Outcome::Satisfied
        );
        let byte_exact = MeasureBounds {
            minimum: bound(text.len() as u64, true),
            maximum: bound(text.len() as u64, true),
        };
        assert_eq!(
            ScalarConstraintParameters::BytesLength(byte_exact)
                .evaluate(&BaseValue::BytesValue(MapBytes(text.as_bytes().to_vec()))),
            Outcome::Satisfied
        );
        if text.len() as u64 != clusters {
            assert!(matches!(
                ScalarConstraintParameters::BytesLength(exact)
                    .evaluate(&BaseValue::BytesValue(MapBytes(text.as_bytes().to_vec()))),
                Outcome::Violated(_)
            ));
        }
    }
    assert_ne!("é".len(), "e\u{301}".len());
}

#[test]
fn violation_details_display_signed_measures_and_open_bounds() {
    for (bounds, value, expected) in [
        (
            IntegerInterval { minimum: bound(-5, true), maximum: bound(5, false) },
            5,
            "measured 5; required [-5, 5)",
        ),
        (
            IntegerInterval { minimum: None, maximum: bound(-5, true) },
            0,
            "measured 0; required (-inf, -5]",
        ),
        (
            IntegerInterval { minimum: bound(0, false), maximum: None },
            -1,
            "measured -1; required (0, +inf)",
        ),
    ] {
        let Outcome::Violated(detail) =
            ScalarConstraintParameters::NumericRange(bounds).evaluate(&integer(value))
        else {
            panic!("expected violation")
        };
        assert_eq!(detail.to_string(), expected);
        assert_eq!(detail.bounds, ScalarConstraintParameters::NumericRange(bounds));
    }
    let bounds = MeasureBounds { minimum: bound(1, true), maximum: bound(2, false) };
    let Outcome::Violated(detail) =
        ScalarConstraintParameters::StringLength(bounds).evaluate(&string("abc"))
    else {
        panic!("expected violation")
    };
    assert_eq!(detail.to_string(), "measured 3 graphemes; required [1, 2)");
}

#[test]
fn only_scalar_configuration_produces_executable_parameters() {
    let bounds = MeasureBounds { minimum: None, maximum: bound(2, true) };
    for (parameters, family) in [
        (ConstraintParameters::StringLength(bounds), ScalarConstraintFamily::StringLength),
        (ConstraintParameters::BytesLength(bounds), ScalarConstraintFamily::BytesLength),
        (
            ConstraintParameters::NumericRange(IntegerInterval {
                minimum: None,
                maximum: bound(2, true),
            }),
            ScalarConstraintFamily::NumericRange,
        ),
    ] {
        assert_eq!(parameters.scalar_parameters().unwrap().family(), family);
    }
    for parameters in [
        ConstraintParameters::ItemCount(bounds),
        ConstraintParameters::Cardinality { minimum: 0, maximum: None },
        ConstraintParameters::UniqueItems,
    ] {
        assert_eq!(parameters.scalar_parameters(), None);
    }
}

#[test]
fn length_diagnostics_include_units_and_the_natural_floor() {
    let bounds = MeasureBounds { minimum: None, maximum: bound(2, true) };
    for (parameters, value, expected) in [
        (
            ScalarConstraintParameters::StringLength(bounds),
            string("abc"),
            "measured 3 graphemes; required [0, 2]",
        ),
        (
            ScalarConstraintParameters::BytesLength(bounds),
            BaseValue::BytesValue(MapBytes(vec![0; 3])),
            "measured 3 bytes; required [0, 2]",
        ),
    ] {
        let Outcome::Violated(detail) = parameters.evaluate(&value) else {
            panic!("expected violation")
        };
        assert_eq!(detail.to_string(), expected);
        assert_eq!(detail.bounds, parameters);
    }
}

#[test]
fn semantic_codes_keep_their_public_contract() {
    assert_eq!(STRING_LENGTH_OUT_OF_RANGE, "StringLengthOutOfRange");
    assert_eq!(BYTES_LENGTH_OUT_OF_RANGE, "BytesLengthOutOfRange");
    assert_eq!(NUMERIC_VALUE_OUT_OF_RANGE, "NumericValueOutOfRange");
}

#[test]
fn unicode_17_default_extended_grapheme_boundaries() {
    assert_eq!(UNICODE_VERSION, (17, 0, 0));
    let data = include_str!("data/unicode-17.0.0/GraphemeBreakTest.txt");
    assert!(data.starts_with("# GraphemeBreakTest-17.0.0.txt"));
    let mut cases = 0;
    for (line_number, line) in data.lines().enumerate() {
        let specification = line.split('#').next().unwrap().trim();
        if specification.is_empty() {
            continue;
        }
        let mut text = String::new();
        let mut expected = Vec::new();
        for token in specification.split_whitespace() {
            match token {
                "÷" => expected.push(text.len()),
                "×" => {}
                codepoint => {
                    text.push(char::from_u32(u32::from_str_radix(codepoint, 16).unwrap()).unwrap())
                }
            }
        }
        let mut actual: Vec<_> = text.grapheme_indices(true).map(|(offset, _)| offset).collect();
        actual.push(text.len());
        assert_eq!(actual, expected, "Unicode test line {}: {}", line_number + 1, line);
        cases += 1;
    }
    assert_eq!(cases, 766, "all vendored Unicode 17 cases must be exercised");
}
