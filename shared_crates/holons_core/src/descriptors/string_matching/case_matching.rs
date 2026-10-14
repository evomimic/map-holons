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

impl CaseMatching {
    /// Parses the enum variant TypeName stored on operator descriptors.
    pub fn parse(value: &MapString) -> Result<Self, HolonError> {
        match value.0.as_str() {
            "CaseSensitive" => Ok(Self::CaseSensitive),
            "UnicodeDefaultCaseFold" => Ok(Self::UnicodeDefaultCaseFold),
            _ => Err(HolonError::UnknownCaseMatching { value: value.to_string() }),
        }
    }

    /// Returns whether `subject` contains `literal` under this policy.
    ///
    /// Matching is over scalar sequences, so a folded expansion can match part of
    /// one original character (`s` within the `ss` folded from `ß`). The literal is
    /// never interpreted as a pattern, and an empty literal matches every subject.
    pub fn contains(self, subject: &str, literal: &str) -> bool {
        match self {
            Self::CaseSensitive => subject.contains(literal),
            Self::UnicodeDefaultCaseFold => {
                default_case_fold(subject).contains(&default_case_fold(literal))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CaseMatching::{self, CaseSensitive, UnicodeDefaultCaseFold};
    use base_types::MapString;
    use core_types::HolonError;

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
        assert!(UnicodeDefaultCaseFold.contains("Straße", "STRASSE"));
        assert!(UnicodeDefaultCaseFold.contains("Straße", "s"));
        assert!(UnicodeDefaultCaseFold.contains("Fuß", "SS"));
        assert!(UnicodeDefaultCaseFold.contains("ΟΔΟΣ", "οδος"));
    }

    #[test]
    fn folded_matching_does_not_normalize() {
        assert!(!UnicodeDefaultCaseFold.contains("caf\u{00E9}", "cafe\u{0301}"));
        assert!(!UnicodeDefaultCaseFold.contains("cafe\u{0301}", "caf\u{00E9}"));
    }

    #[test]
    fn literals_are_not_patterns() {
        assert!(UnicodeDefaultCaseFold.contains("a.*[b", ".*["));
        assert!(!UnicodeDefaultCaseFold.contains("abc", "a.*"));
    }

    #[test]
    fn empty_literal_matches_every_subject() {
        assert!(UnicodeDefaultCaseFold.contains("anything", ""));
        assert!(UnicodeDefaultCaseFold.contains("", ""));
        assert!(CaseSensitive.contains("", ""));
    }

    #[test]
    fn case_sensitive_matching_does_not_fold() {
        assert!(CaseSensitive.contains("Straße", "aß"));
        assert!(!CaseSensitive.contains("Straße", "STRASSE"));
        assert!(!CaseSensitive.contains("Graph", "graph"));
    }
}
