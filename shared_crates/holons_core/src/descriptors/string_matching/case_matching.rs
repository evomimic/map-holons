use super::case_folding::default_case_fold;
use base_types::MapString;
use core_types::HolonError;

/// Case-matching policy declared by string matching operator descriptors.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CaseMatching {
    /// Literal matching on the original scalar sequences.
    CaseSensitive,
    /// Literal matching after Unicode full default case folding of both strings.
    UnicodeDefaultCaseFold,
}

/// Where a string matching operator requires its literal to occur in the subject.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum LiteralMatch {
    /// Anywhere: the subject contains the literal as a substring.
    Contains,
    /// At the start: the subject begins with the literal.
    StartsWith,
    /// At the end: the subject ends with the literal.
    EndsWith,
}

impl LiteralMatch {
    /// Whether `literal` occurs in `subject` at this position, comparing scalars exactly.
    fn holds(self, subject: &str, literal: &str) -> bool {
        match self {
            Self::Contains => subject.contains(literal),
            Self::StartsWith => subject.starts_with(literal),
            Self::EndsWith => subject.ends_with(literal),
        }
    }
}

impl CaseMatching {
    /// Parses the enum variant TypeName stored on operator descriptors.
    pub fn parse(value: &MapString) -> Result<Self, HolonError> {
        match value.0.as_str() {
            "CaseSensitive" => Ok(Self::CaseSensitive),
            "UnicodeDefaultCaseFold" => Ok(Self::UnicodeDefaultCaseFold),
            _ => Err(HolonError::UnknownCaseMatching { value: value.to_string() }),
        }
    }

    /// Returns whether `literal` occurs in `subject` as `literal_match` requires,
    /// under this policy.
    ///
    /// Under `UnicodeDefaultCaseFold`:
    /// 1. Both the complete subject and the complete literal undergo Unicode full
    ///    default case folding ([`default_case_fold`]) before any matching.
    /// 2. Matching is performed on the folded scalar sequences, never on the
    ///    original strings.
    /// 3. A match need not align with character or grapheme boundaries of the
    ///    original strings. One original character can fold to several scalars,
    ///    and a match may cover part of that expansion, at either boundary:
    ///    `StartsWith("ßeta", "s")`, `StartsWith("ßeta", "ss")`, `EndsWith("Fuß", "s")`
    ///    and `EndsWith("Fuß", "ss")` all hold, because `ß` folds to `ss`.
    /// 4. Contains, StartsWith and EndsWith share these folding semantics and this
    ///    implementation; they differ only in where the folded literal must occur.
    /// 5. The folding data is pinned to one Unicode version,
    ///    [`CASE_FOLDING_UNICODE_VERSION`](super::CASE_FOLDING_UNICODE_VERSION)
    ///    (17.0.0), independent of the compiler's standard library, so host and guest
    ///    builds fold identically.
    ///
    /// `CaseSensitive` matches the original scalar sequences exactly. Under either
    /// policy, no normalization is applied, the literal is never interpreted as a
    /// pattern, and an empty literal matches every subject.
    pub fn matches(self, literal_match: LiteralMatch, subject: &str, literal: &str) -> bool {
        match self {
            Self::CaseSensitive => literal_match.holds(subject, literal),
            Self::UnicodeDefaultCaseFold => {
                literal_match.holds(&default_case_fold(subject), &default_case_fold(literal))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CaseMatching::{self, CaseSensitive, UnicodeDefaultCaseFold};
    use super::LiteralMatch::{self, Contains, EndsWith, StartsWith};
    use crate::descriptors::string_matching::default_case_fold;
    use base_types::MapString;
    use core_types::HolonError;

    const ALL: [LiteralMatch; 3] = [Contains, StartsWith, EndsWith];

    #[test]
    fn parses_stored_variant_names_and_rejects_unknown_modes() {
        let parse = |value: &str| CaseMatching::parse(&MapString(value.to_string()));
        assert_eq!(parse("CaseSensitive").unwrap(), CaseSensitive);
        assert_eq!(parse("UnicodeDefaultCaseFold").unwrap(), UnicodeDefaultCaseFold);
        assert!(matches!(
            parse("CaseMatching.CaseSensitive"),
            Err(HolonError::UnknownCaseMatching { value }) if value == "CaseMatching.CaseSensitive"
        ));
    }

    #[test]
    fn folded_matching_includes_expansions() {
        assert!(UnicodeDefaultCaseFold.matches(Contains, "Straße", "STRASSE"));
        assert!(UnicodeDefaultCaseFold.matches(Contains, "Straße", "s"));
        assert!(UnicodeDefaultCaseFold.matches(Contains, "Fuß", "SS"));
        assert!(UnicodeDefaultCaseFold.matches(Contains, "ΟΔΟΣ", "οδος"));
    }

    #[test]
    fn folded_matching_does_not_normalize() {
        for literal_match in ALL {
            assert!(!UnicodeDefaultCaseFold.matches(literal_match, "caf\u{00E9}", "cafe\u{0301}"));
            assert!(!UnicodeDefaultCaseFold.matches(literal_match, "cafe\u{0301}", "caf\u{00E9}"));
        }
    }

    #[test]
    fn literals_are_not_patterns() {
        assert!(UnicodeDefaultCaseFold.matches(Contains, "a.*[b", ".*["));
        assert!(!UnicodeDefaultCaseFold.matches(Contains, "abc", "a.*"));
        assert!(UnicodeDefaultCaseFold.matches(StartsWith, "^a.*", "^A."));
        assert!(!UnicodeDefaultCaseFold.matches(StartsWith, "abc", "^a"));
        assert!(UnicodeDefaultCaseFold.matches(EndsWith, "x$[0-9]", "$[0-9]"));
        assert!(!UnicodeDefaultCaseFold.matches(EndsWith, "abc9", "[0-9]"));
    }

    #[test]
    fn empty_literal_matches_every_subject() {
        for literal_match in ALL {
            for case_matching in [CaseSensitive, UnicodeDefaultCaseFold] {
                assert!(case_matching.matches(literal_match, "anything", ""));
                assert!(case_matching.matches(literal_match, "", ""));
            }
        }
    }

    #[test]
    fn case_sensitive_matching_does_not_fold() {
        assert!(CaseSensitive.matches(Contains, "Straße", "aß"));
        assert!(!CaseSensitive.matches(Contains, "Straße", "STRASSE"));
        assert!(!CaseSensitive.matches(Contains, "Graph", "graph"));
        assert!(CaseSensitive.matches(StartsWith, "Graph", "Gr"));
        assert!(!CaseSensitive.matches(StartsWith, "Graph", "gr"));
        assert!(CaseSensitive.matches(EndsWith, "Straße", "ße"));
        assert!(!CaseSensitive.matches(EndsWith, "Straße", "SSE"));
    }

    #[test]
    fn boundary_operators_match_only_at_their_boundary() {
        for case_matching in [CaseSensitive, UnicodeDefaultCaseFold] {
            assert!(case_matching.matches(StartsWith, "Graph Theory", "Graph"));
            assert!(!case_matching.matches(StartsWith, "Graph Theory", "Theory"));
            assert!(case_matching.matches(EndsWith, "Graph Theory", "Theory"));
            assert!(!case_matching.matches(EndsWith, "Graph Theory", "Graph"));
            // Interior-only text matches Contains but neither boundary.
            assert!(case_matching.matches(Contains, "Graph Theory", "ph Th"));
            assert!(!case_matching.matches(StartsWith, "Graph Theory", "ph Th"));
            assert!(!case_matching.matches(EndsWith, "Graph Theory", "ph Th"));
            // A literal longer than the subject never matches a boundary.
            assert!(!case_matching.matches(StartsWith, "Gr", "Graph"));
            assert!(!case_matching.matches(EndsWith, "", "x"));
            // The whole subject is both its own prefix and suffix.
            assert!(case_matching.matches(StartsWith, "Graph", "Graph"));
            assert!(case_matching.matches(EndsWith, "Graph", "Graph"));
        }
    }

    #[test]
    fn folded_boundaries_match_case_insensitively_and_through_expansions() {
        assert!(UnicodeDefaultCaseFold.matches(StartsWith, "Straße", "STRA"));
        assert!(UnicodeDefaultCaseFold.matches(EndsWith, "Straße", "SSE"));
        assert!(UnicodeDefaultCaseFold.matches(StartsWith, "STRASSE", "straß"));
        // A fold expansion may match partially or wholly at either boundary,
        // without aligning to the original character.
        for literal in ["s", "ss", "S", "SS"] {
            assert!(UnicodeDefaultCaseFold.matches(StartsWith, "ßeta", literal), "{literal}");
            assert!(UnicodeDefaultCaseFold.matches(EndsWith, "Fuß", literal), "{literal}");
            assert!(!CaseSensitive.matches(StartsWith, "ßeta", literal), "{literal}");
            assert!(!CaseSensitive.matches(EndsWith, "Fuß", literal), "{literal}");
        }
        // The expansion is exactly two scalars long.
        assert!(!UnicodeDefaultCaseFold.matches(StartsWith, "ßeta", "sss"));
        assert!(!UnicodeDefaultCaseFold.matches(EndsWith, "Fuß", "sss"));
        // Final and ordinary sigma fold alike at the end of a word.
        assert!(UnicodeDefaultCaseFold.matches(EndsWith, "ΟΔΟΣ", "ος"));
        assert!(UnicodeDefaultCaseFold.matches(EndsWith, "οδος", "ΟΣ"));
        assert!(!CaseSensitive.matches(EndsWith, "οδος", "ΟΣ"));
    }

    #[test]
    fn all_three_operators_share_one_folded_comparison() {
        // Each operator agrees with its plain-string counterpart over the folded
        // strings, so the folding step is common and only the position differs.
        let cases =
            [("Straße", "SSE"), ("ßeta", "ss"), ("Fuß", "s"), ("ΟΔΟΣ", "ος"), ("Graph", "ph")];
        for (subject, literal) in cases {
            let (subject_folded, literal_folded) =
                (default_case_fold(subject), default_case_fold(literal));
            for (literal_match, expected) in [
                (Contains, subject_folded.contains(&literal_folded)),
                (StartsWith, subject_folded.starts_with(&literal_folded)),
                (EndsWith, subject_folded.ends_with(&literal_folded)),
            ] {
                assert_eq!(
                    UnicodeDefaultCaseFold.matches(literal_match, subject, literal),
                    expected,
                    "{literal_match:?}({subject:?}, {literal:?})"
                );
            }
        }
    }

    #[test]
    fn matching_leaves_its_inputs_unchanged() {
        let (subject, literal) = (String::from("Straße"), String::from("STRA"));
        assert!(UnicodeDefaultCaseFold.matches(StartsWith, &subject, &literal));
        assert_eq!((subject.as_str(), literal.as_str()), ("Straße", "STRA"));
    }
}
