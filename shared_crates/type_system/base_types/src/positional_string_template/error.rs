use std::fmt;

/// A syntax failure at a half-open UTF-8 byte span in the decoded template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemplateSyntaxError {
    pub span: (usize, usize),
    pub reason: TemplateSyntaxErrorReason,
}

/// The grammar rule violated by a template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateSyntaxErrorReason {
    UnmatchedDelimiter,
    UnclosedSubstitution,
    UnclosedSegment,
    /// Empty or zero-prefixed multi-digit indices highlight the whole reference.
    /// Unexpected characters in the index highlight only the offending character.
    InvalidIndexSpelling,
    UnknownEscape,
    TrailingBackslash,
    EmptySegment,
    NestedSegment,
    NulCharacter,
    LegacyPlaceholder,
}

impl fmt::Display for TemplateSyntaxErrorReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnmatchedDelimiter => "unmatched delimiter",
            Self::UnclosedSubstitution => "unclosed substitution",
            Self::UnclosedSegment => "unclosed optional segment",
            Self::InvalidIndexSpelling => "invalid index spelling",
            Self::UnknownEscape => "unknown escape",
            Self::TrailingBackslash => "trailing backslash",
            Self::EmptySegment => "optional segment has no required substitution",
            Self::NestedSegment => "optional segments cannot contain unescaped opening brackets",
            Self::NulCharacter => "NUL character",
            Self::LegacyPlaceholder => "legacy dollar-sign placeholder",
        })
    }
}

impl fmt::Display for TemplateSyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (start, end) = self.span;
        write!(f, "{} at UTF-8 bytes [{start}, {end})", self.reason)
    }
}

impl std::error::Error for TemplateSyntaxError {}
