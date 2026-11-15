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
    /// Folding applies to both complete strings before matching, and matching is
    /// over the resulting scalar sequences. A folded expansion can therefore match
    /// part of one original character, at a boundary too: `s` is a prefix of the
    /// `ss…` folded from `ß…`. The literal is never interpreted as a pattern, and
    /// an empty literal matches every subject.
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
        // A fold expansion may match partially at either boundary.
        assert!(UnicodeDefaultCaseFold.matches(StartsWith, "ßeta", "s"));
        assert!(UnicodeDefaultCaseFold.matches(EndsWith, "Fuß", "s"));
        assert!(!CaseSensitive.matches(StartsWith, "ßeta", "s"));
        assert!(!CaseSensitive.matches(EndsWith, "Fuß", "s"));
        // Final and ordinary sigma fold alike at the end of a word.
        assert!(UnicodeDefaultCaseFold.matches(EndsWith, "ΟΔΟΣ", "ος"));
        assert!(UnicodeDefaultCaseFold.matches(EndsWith, "οδος", "ΟΣ"));
        assert!(!CaseSensitive.matches(EndsWith, "οδος", "ΟΣ"));
    }

    #[test]
    fn matching_leaves_its_inputs_unchanged() {
        let (subject, literal) = (String::from("Straße"), String::from("STRA"));
        assert!(UnicodeDefaultCaseFold.matches(StartsWith, &subject, &literal));
        assert_eq!((subject.as_str(), literal.as_str()), ("Straße", "STRA"));
    }
}
