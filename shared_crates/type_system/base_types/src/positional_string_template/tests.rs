use super::syntax::{ParameterIndex, ParameterRef, SegmentBody, TemplatePiece};
use super::{PositionalStringTemplate, TemplateSyntaxError, TemplateSyntaxErrorReason::*};
use crate::MapString;
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

    for (source, reason, span) in cases {
        let expected = TemplateSyntaxError { reason, span };
        assert_eq!(PositionalStringTemplate::parse(source).unwrap_err(), expected, "{source:?}");
        assert_eq!(PositionalStringTemplate::validate_syntax(source), Err(expected), "{source:?}");
        assert!(source.get(span.0..span.1).is_some(), "{source:?}");
    }
}

#[test]
fn displays_the_syntax_reason_and_utf8_byte_span() {
    let error = PositionalStringTemplate::parse("«é»{0}[.{1}").unwrap_err();
    assert_eq!(error.to_string(), "unclosed optional segment at UTF-8 bytes [9, 14)");
}
