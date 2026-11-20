use super::syntax::{ParameterIndex, ParameterRef, SegmentBody, TemplatePiece};
use super::{
    PositionalStringTemplate, TemplateConfigurationDefect, TemplateInput, TemplateRenderError,
    TemplateSyntaxError, TemplateSyntaxErrorReason::*,
};
use crate::{BaseValue, BaseValueKind, MapBytes, MapEnumValue, MapString, ToBaseValue};
use std::ops::Range;

fn parameter(index: usize, span: Range<usize>) -> ParameterRef {
    ParameterRef { index: ParameterIndex::Position(index), span }
}

#[test]
fn retains_exact_source_and_coalesces_adjacent_literals() {
    let source = MapString("  e\u{301}\né {{text}} ".to_owned());
    let template = PositionalStringTemplate::try_from(&source).unwrap();
    assert_eq!(template.source(), source.0);
    assert_eq!(template.pieces, [TemplatePiece::Literal("  e\u{301}\né {text} ".to_owned())]);
    assert!(PositionalStringTemplate::parse("").unwrap().pieces.is_empty());
}

#[test]
fn decodes_the_specification_escape_examples() {
    use TemplatePiece::*;

    let cases = [
        ("{{0}}", vec![Literal("{0}".to_owned())]),
        (r"\[1\]", vec![Literal("[1]".to_owned())]),
        (
            "{{{0}}}",
            vec![Literal("{".to_owned()), Required(parameter(0, 2..5)), Literal("}".to_owned())],
        ),
        (
            r"{0}\\[1]",
            vec![
                Required(parameter(0, 0..3)),
                Literal("\\".to_owned()),
                OptionalField(parameter(1, 5..8)),
            ],
        ),
        (
            r"{0}[.{1}]\]",
            vec![
                Required(parameter(0, 0..3)),
                Segment(SegmentBody {
                    pieces: vec![Literal(".".to_owned()), Required(parameter(1, 5..8))],
                    referenced: vec![1],
                }),
                Literal("]".to_owned()),
            ],
        ),
        (r"\$0", vec![Literal("$0".to_owned())]),
    ];

    for (source, expected) in cases {
        assert_eq!(PositionalStringTemplate::parse(source).unwrap().pieces, expected, "{source}");
    }
}

#[test]
fn decodes_brace_and_bracket_escapes_inside_segments() {
    use TemplatePiece::*;

    let cases = [
        (
            r"[.{0}\]]",
            SegmentBody {
                pieces: vec![
                    Literal(".".to_owned()),
                    Required(parameter(0, 2..5)),
                    Literal("]".to_owned()),
                ],
                referenced: vec![0],
            },
        ),
        (
            "[{{0}}{1}]",
            SegmentBody {
                pieces: vec![Literal("{0}".to_owned()), Required(parameter(1, 6..9))],
                referenced: vec![1],
            },
        ),
    ];

    for (source, expected) in cases {
        assert_eq!(
            PositionalStringTemplate::parse(source).unwrap().pieces,
            [Segment(expected)],
            "{source}"
        );
    }
}

#[test]
fn distinguishes_digit_only_fields_from_segments_and_keeps_repeated_substitutions() {
    use TemplatePiece::*;

    let template = PositionalStringTemplate::parse("[10][01{2}-{2}.{0}]").unwrap();
    assert_eq!(
        template.pieces,
        [
            OptionalField(parameter(10, 0..4)),
            Segment(SegmentBody {
                pieces: vec![
                    Literal("01".to_owned()),
                    Required(parameter(2, 7..10)),
                    Literal("-".to_owned()),
                    Required(parameter(2, 11..14)),
                    Literal(".".to_owned()),
                    Required(parameter(0, 15..18)),
                ],
                referenced: vec![0, 2],
            }),
        ]
    );

    let template = PositionalStringTemplate::parse("{1}-{0}-{1}").unwrap();
    assert_eq!(
        template.pieces,
        [
            Required(parameter(1, 0..3)),
            Literal("-".to_owned()),
            Required(parameter(0, 4..7)),
            Literal("-".to_owned()),
            Required(parameter(1, 8..11)),
        ]
    );
}

#[test]
fn records_index_overflow_without_rejecting_syntax() {
    use TemplatePiece::*;

    let largest = format!("{{{}}}", usize::MAX);
    assert_eq!(
        PositionalStringTemplate::parse(&largest).unwrap().pieces,
        [Required(parameter(usize::MAX, 0..largest.len()))]
    );

    let source = "{99999999999999999999999}[99999999999999999999999][.{99999999999999999999999}]";
    assert_eq!(
        PositionalStringTemplate::parse(source).unwrap().pieces,
        [
            Required(ParameterRef { index: ParameterIndex::Unrepresentable, span: 0..25 }),
            OptionalField(ParameterRef { index: ParameterIndex::Unrepresentable, span: 25..50 }),
            Segment(SegmentBody {
                pieces: vec![
                    Literal(".".to_owned()),
                    Required(ParameterRef { index: ParameterIndex::Unrepresentable, span: 52..77 }),
                ],
                referenced: vec![],
            }),
        ]
    );
}

#[test]
fn accepts_decoded_templates_independently_of_configuration() {
    for source in [
        "",
        "Singleton",
        "\n\t ",
        "{0}",
        "${0}",
        "cost$",
        "$name",
        "$١",
        "[0]",
        "[1{2}]",
        "{0}.[1].[2]",
        "{0}[.{1}][.{2}]",
        "{0}[.{1}-{2}]",
        "{1}-{0}-{1}",
        "{0}[0]",
        "{0}[.{2}]",
        r"[\[{{{0}}}\]\\\$0]",
    ] {
        assert_eq!(PositionalStringTemplate::parse(source).unwrap().source(), source);
        assert_eq!(PositionalStringTemplate::validate_syntax(source), Ok(()), "{source:?}");
    }
}

#[test]
fn reports_malformed_constructs_at_utf8_byte_spans() {
    let cases = [
        ("{}", InvalidIndexSpelling, (0, 2)),
        ("{01}", InvalidIndexSpelling, (0, 4)),
        ("[01]", InvalidIndexSpelling, (0, 4)),
        ("{-1}", InvalidIndexSpelling, (1, 2)),
        ("{ 0 }", InvalidIndexSpelling, (1, 2)),
        ("{0 }", InvalidIndexSpelling, (2, 3)),
        ("{0:2}", InvalidIndexSpelling, (2, 3)),
        ("é{١}", InvalidIndexSpelling, (3, 5)),
        (r"{0\}", InvalidIndexSpelling, (2, 3)),
        ("«é»{0", UnclosedSubstitution, (6, 8)),
        ("{", UnclosedSubstitution, (0, 1)),
        ("é}", UnmatchedDelimiter, (2, 3)),
        ("é]", UnmatchedDelimiter, (2, 3)),
        ("«é»{0}[.{1}", UnclosedSegment, (9, 14)),
        ("[1", UnclosedSegment, (0, 2)),
        ("[]", EmptySegment, (0, 2)),
        ("[ab]", EmptySegment, (0, 4)),
        ("[{{0}}]", EmptySegment, (0, 7)),
        ("[.{1}[2]]", NestedSegment, (5, 6)),
        ("[[", NestedSegment, (1, 2)),
        ("{0}[1]]", UnmatchedDelimiter, (6, 7)),
        (r"é\🦀", UnknownEscape, (2, 7)),
        (r"\n", UnknownEscape, (0, 2)),
        (r"\{", UnknownEscape, (0, 2)),
        (r"\}", UnknownEscape, (0, 2)),
        ("é\\", TrailingBackslash, (2, 3)),
        ("é\0", NulCharacter, (2, 3)),
        ("[.\0{0}]", NulCharacter, (2, 3)),
        ("{0\0}", NulCharacter, (2, 3)),
        ("\\\0", NulCharacter, (1, 2)),
        ("é$0", LegacyPlaceholder, (2, 4)),
        (r"\\$0", LegacyPlaceholder, (2, 4)),
        ("[.$0{1}]", LegacyPlaceholder, (2, 4)),
    ];

    for (source, reason, (start, end)) in cases {
        let expected = TemplateSyntaxError { reason, span: start..end };
        assert_eq!(PositionalStringTemplate::parse(source).unwrap_err(), expected, "{source:?}");
        assert_eq!(PositionalStringTemplate::validate_syntax(source), Err(expected), "{source:?}");
        assert!(source.get(start..end).is_some(), "{source:?}");
    }
}

#[test]
fn displays_the_syntax_reason_and_utf8_byte_span() {
    let error = PositionalStringTemplate::parse("«é»{0}[.{1}").unwrap_err();
    assert_eq!(error.to_string(), "unclosed optional segment at UTF-8 bytes [9, 14)");
}

#[test]
fn displays_each_configuration_defect_in_a_render_error() {
    use TemplateConfigurationDefect::*;

    let error = TemplateRenderError::Configuration(vec![
        IndexOutOfRange { span: 5..8 },
        UnusedParameter { position: 1 },
    ]);
    assert_eq!(
        error.to_string(),
        "invalid template configuration, parameter index out of range at UTF-8 bytes [5, 8), parameter 1 is unused"
    );
}

fn render(source: &str, inputs: &[TemplateInput<'_>]) -> Result<MapString, TemplateRenderError> {
    PositionalStringTemplate::parse(source).unwrap().render(inputs, usize::MAX)
}

#[test]
fn renders_the_specification_examples() {
    use TemplateInput::{Absent, Present};

    let a = "A".to_base_value();
    let b = "B".to_base_value();
    let c = "C".to_base_value();
    let empty = "".to_base_value();
    let false_value = false.to_base_value();
    let zero = 0_i64.to_base_value();
    let delimiters = "{1}".to_base_value();
    let cases: &[(&str, &[TemplateInput<'_>], &str)] = &[
        ("{0}.[1].[2]", &[Present(&a), Present(&b), Present(&c)], "A.B.C"),
        ("{0}.[1].[2]", &[Present(&a), Absent, Present(&c)], "A..C"),
        ("{0}.[1].[2]", &[Present(&a), Absent, Absent], "A.."),
        ("{0}[.{1}][.{2}]", &[Present(&a), Absent, Present(&c)], "A.C"),
        ("{0}[.{1}][.{2}]", &[Present(&a), Absent, Absent], "A"),
        ("{0}[.{1}][.{2}]", &[Present(&a), Present(&empty), Absent], "A."),
        ("[.{0}]", &[Present(&empty)], "."),
        ("{0}[.{1}]", &[Present(&a), Present(&false_value)], "A.false"),
        ("{0}[.{1}]", &[Present(&a), Present(&zero)], "A.0"),
        ("{0}[.{1}-{2}]", &[Present(&a), Present(&b), Absent], "A"),
        ("{0}[.{1}-{2}]", &[Present(&a), Present(&b), Present(&c)], "A.B-C"),
        ("{1}-{0}-{1}", &[Present(&a), Present(&b)], "B-A-B"),
        ("{0}[0]", &[Present(&a)], "AA"),
        ("[0]", &[Absent], ""),
        ("Singleton", &[], "Singleton"),
        ("", &[], ""),
        ("{0}", &[Present(&delimiters)], "{1}"),
        ("{{0}}", &[], "{0}"),
        (r"\[1\]", &[], "[1]"),
        ("{{{0}}}", &[Present(&a)], "{A}"),
        (r"{0}\\[1]", &[Present(&a), Present(&b)], r"A\B"),
        (r"{0}[.{1}]\]", &[Present(&a), Absent], "A]"),
        (r"{0}[.{1}]\]", &[Present(&a), Present(&b)], "A.B]"),
        (r"\$0", &[], "$0"),
    ];

    for &(source, inputs, expected) in cases {
        assert_eq!(render(source, inputs).unwrap().0, expected, "{source:?}");
    }
}

#[test]
fn reports_every_configuration_defect_including_references_inside_segments() {
    use BaseValueKind::{Bytes, String};
    use TemplateConfigurationDefect::*;

    let template = PositionalStringTemplate::parse("{0}[.{2}]").unwrap();
    assert_eq!(
        template.check_configuration(&[String, Bytes]),
        Err(vec![
            IndexOutOfRange { span: 5..8 },
            UnusedParameter { position: 1 },
            UnsupportedParameterKind { position: 1, kind: Bytes },
        ])
    );
    assert_eq!(
        render("{0}[.{2}]", &[TemplateInput::Present(&"A".to_base_value()), TemplateInput::Absent]),
        Err(TemplateRenderError::Configuration(vec![
            IndexOutOfRange { span: 5..8 },
            UnusedParameter { position: 1 },
        ]))
    );

    let cases = [
        ("{0}[.{1}]", vec![String], vec![IndexOutOfRange { span: 5..8 }]),
        ("[.{99999999999999999999999}]", vec![], vec![IndexOutOfRange { span: 2..27 }]),
        ("[99999999999999999999999]", vec![], vec![IndexOutOfRange { span: 0..25 }]),
        ("{0}", vec![String, String], vec![UnusedParameter { position: 1 }]),
        (
            "{0}{2}",
            vec![String, String],
            vec![IndexOutOfRange { span: 3..6 }, UnusedParameter { position: 1 }],
        ),
        ("{{0}}", vec![String], vec![UnusedParameter { position: 0 }]),
        ("", vec![String], vec![UnusedParameter { position: 0 }]),
    ];
    for (source, kinds, defects) in cases {
        let template = PositionalStringTemplate::parse(source).unwrap();
        assert_eq!(template.check_configuration(&kinds), Err(defects), "{source:?}");
    }
}

#[test]
fn checks_present_values_before_required_presence_and_output_size() {
    use TemplateInput::{Absent, Present};
    use TemplateRenderError::*;

    let template = PositionalStringTemplate::parse("{0}[.{1}-{2}]").unwrap();
    let invalid = BaseValue::BytesValue(MapBytes(vec![1]));
    let a = "A".to_base_value();
    let invalid_input = Err(InvalidInput { position: 2, kind: BaseValueKind::Bytes });
    assert_eq!(template.render(&[Present(&a), Absent, Present(&invalid)], 0), invalid_input);
    assert_eq!(template.render(&[Absent, Absent, Present(&invalid)], 0), invalid_input);
    assert_eq!(
        template.render(&[Absent, Absent, Absent], 0),
        Err(MissingRequiredInput { position: 0 })
    );
    assert_eq!(render("{0}[0]", &[Absent]), Err(MissingRequiredInput { position: 0 }));
}

#[test]
fn uses_the_utf8_byte_bound_and_skips_suppressed_segments() {
    use TemplateInput::{Absent, Present};

    let template = PositionalStringTemplate::parse("é{0}[.{1}]").unwrap();
    let value = "🦀".to_base_value();
    let nonempty = [Present(&value), Present(&value)];
    let suppressed = [Present(&value), Absent];

    assert_eq!(template.render(&nonempty, 11).unwrap().0, "é🦀.🦀");
    assert_eq!(template.render(&nonempty, 10), Err(TemplateRenderError::Resource { bound: 10 }));
    assert_eq!(template.render(&suppressed, 6).unwrap().0, "é🦀");
    assert_eq!(template.render(&suppressed, 5), Err(TemplateRenderError::Resource { bound: 5 }));
    assert_eq!(PositionalStringTemplate::parse("").unwrap().render(&[], 0).unwrap().0, "");
}

#[test]
fn renders_canonical_scalars_and_reuses_the_parsed_template() {
    use TemplateInput::Present;

    let template = PositionalStringTemplate::parse("{0}").unwrap();
    for (number, expected) in [
        (i64::MIN, "-9223372036854775808"),
        (i64::MAX, "9223372036854775807"),
        (-1, "-1"),
        (0, "0"),
    ] {
        assert_eq!(template.render(&[Present(&number.to_base_value())], 20).unwrap().0, expected);
    }

    let token = MapEnumValue(MapString("Member_TOKEN".to_owned())).to_base_value();
    assert_eq!(template.render(&[Present(&token)], 12).unwrap().0, "Member_TOKEN");
    assert_eq!(template.render(&[Present(&true.to_base_value())], 4).unwrap().0, "true");
    let exact = " e\u{301} [0] \\$0 \n";
    assert_eq!(template.render(&[Present(&exact.to_base_value())], exact.len()).unwrap().0, exact);
}

#[test]
fn renders_prepared_key_rule_inputs() {
    use TemplateInput::{Absent, Present};

    let space = "MAP.CoreSchemaSpace".to_base_value();
    assert_eq!(render("{0}", &[Present(&space)]).unwrap().0, "MAP.CoreSchemaSpace");

    let name = "Smith".to_base_value();
    let inputs = &[Present(&name), Absent, Absent];
    assert_eq!(render("{0}.[1].[2]", inputs).unwrap().0, "Smith..");
    assert_eq!(render("{0}[.{1}][.{2}]", inputs).unwrap().0, "Smith");

    let member = "Member".to_base_value();
    let theme = "Theme".to_base_value();
    let pair = [Present(&member), Present(&theme)];
    let kinds = [BaseValueKind::String; 2];
    for (source, expected) in [
        ("{1}-{0}-{1}", "Theme-Member-Theme"),
        ("{0}.{1}.ThemeTokenAssignment", "Member.Theme.ThemeTokenAssignment"),
    ] {
        let template = PositionalStringTemplate::parse(source).unwrap();
        assert_eq!(template.check_configuration(&kinds), Ok(()));
        assert_eq!(template.render(&pair, 80).unwrap().0, expected);
    }

    let constraint_name = " a.b name ".to_base_value();
    assert_eq!(
        render("{0}.{1}", &[Present(&member), Present(&constraint_name)]).unwrap().0,
        "Member. a.b name "
    );
}
