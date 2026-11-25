use std::fmt;
use std::ops::Range;

use crate::BaseValueKind;

/// A defect in the binding between a parsed template and its parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateConfigurationDefect {
    /// A reference exceeds the parameter range. The half-open UTF-8 byte span covers the entire reference.
    IndexOutOfRange { span: Range<usize> },
    /// No substitution references this parameter. Escaped placeholder text is literal.
    UnusedParameter { position: usize },
    /// The declared kind has no canonical template rendering.
    UnsupportedParameterKind { position: usize, kind: BaseValueKind },
}

impl fmt::Display for TemplateConfigurationDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IndexOutOfRange { span } => write!(
                f,
                "parameter index out of range at UTF-8 bytes [{}, {})",
                span.start, span.end
            ),
            Self::UnusedParameter { position } => write!(f, "parameter {position} is unused"),
            Self::UnsupportedParameterKind { position, kind } => {
                write!(f, "parameter {position} declares unsupported kind {kind}")
            }
        }
    }
}

impl std::error::Error for TemplateConfigurationDefect {}

/// A failure while rendering a syntax-checked template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateRenderError {
    /// Range or coverage defects found against the number of supplied inputs.
    Configuration(Vec<TemplateConfigurationDefect>),
    /// A top-level required substitution refers to an absent input.
    MissingRequiredInput { position: usize },
    /// A present input has an unsupported kind.
    InvalidInput { position: usize, kind: BaseValueKind },
    /// The output exceeds the caller's UTF-8 byte bound or its buffer could not be allocated.
    Resource { bound: usize },
}

impl fmt::Display for TemplateRenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(defects) => {
                f.write_str("invalid template configuration")?;
                for defect in defects {
                    write!(f, ", {defect}")?;
                }
                Ok(())
            }
            Self::MissingRequiredInput { position } => {
                write!(f, "required input {position} is absent")
            }
            Self::InvalidInput { position, kind } => {
                write!(f, "input {position} has unsupported kind {kind}")
            }
            Self::Resource { bound } => {
                write!(f, "could not produce template output within the {bound}-byte limit")
            }
        }
    }
}

impl std::error::Error for TemplateRenderError {}

/// A syntax failure at a half-open UTF-8 byte span in the decoded template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateSyntaxError {
    /// Half-open UTF-8 byte range in the decoded template.
    pub span: Range<usize>,
    /// The grammar rule violated at this span.
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
        write!(f, "{} at UTF-8 bytes [{}, {})", self.reason, self.span.start, self.span.end)
    }
}

impl std::error::Error for TemplateSyntaxError {}
