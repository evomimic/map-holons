use std::iter::Peekable;
use std::ops::Range;
use std::str::CharIndices;

use crate::MapString;

use super::error::{TemplateSyntaxError, TemplateSyntaxErrorReason};
use TemplateSyntaxErrorReason::*;

/// Exact decoded source and its in-memory, syntax-checked positional template.
#[derive(Debug)]
pub struct PositionalStringTemplate {
    source: String,
    pub(super) pieces: Vec<TemplatePiece>,
}

impl PositionalStringTemplate {
    /// Parses decoded template source without resolving any parameter positions.
    pub fn parse(source: &str) -> Result<Self, TemplateSyntaxError> {
        let pieces = Parser::parse(source)?;
        Ok(Self { source: source.to_owned(), pieces })
    }

    /// Returns the exact decoded source supplied at construction.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Checks syntax using the same parser as construction.
    pub fn validate_syntax(source: &str) -> Result<(), TemplateSyntaxError> {
        Parser::parse(source).map(|_| ())
    }
}

impl TryFrom<&MapString> for PositionalStringTemplate {
    type Error = TemplateSyntaxError;

    fn try_from(source: &MapString) -> Result<Self, Self::Error> {
        Self::parse(&source.0)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum TemplatePiece {
    Literal(String),
    Required(ParameterRef),
    OptionalField(ParameterRef),
    Segment(SegmentBody),
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct SegmentBody {
    pub(super) pieces: Vec<TemplatePiece>,
    /// Distinct representable positions for presence checks. Walk the pieces to validate every index.
    pub(super) referenced: Vec<usize>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct ParameterRef {
    pub(super) index: ParameterIndex,
    pub(super) span: Range<usize>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ParameterIndex {
    Position(usize),
    Unrepresentable,
}

struct Parser<'a> {
    source: &'a str,
    chars: Peekable<CharIndices<'a>>,
}

impl<'a> Parser<'a> {
    fn parse(source: &'a str) -> Result<Vec<TemplatePiece>, TemplateSyntaxError> {
        let mut parser = Self { source, chars: source.char_indices().peekable() };
        parser.parse_pieces(None, String::new())
    }

    /// Parses top-level pieces or stops at the closing bracket of an optional segment.
    fn parse_pieces(
        &mut self,
        segment_start: Option<usize>,
        mut literal: String,
    ) -> Result<Vec<TemplatePiece>, TemplateSyntaxError> {
        let mut pieces = Vec::new();
        while let Some((offset, ch)) = self.chars.next() {
            let piece = match ch {
                // Consume doubled braces as escapes before interpreting a single brace.
                '{' | '}' if self.consume(ch) => {
                    literal.push(ch);
                    continue;
                }
                '\\' => {
                    literal.push(self.parse_escape(offset)?);
                    continue;
                }
                '{' => TemplatePiece::Required(self.parse_required(offset)?),
                '[' if segment_start.is_none() => self.parse_bracketed(offset)?,
                '[' => return Err(syntax_error(NestedSegment, offset..offset + 1)),
                ']' if segment_start.is_some() => {
                    push_literal(&mut pieces, &mut literal);
                    return Ok(pieces);
                }
                '}' | ']' => return Err(syntax_error(UnmatchedDelimiter, offset..offset + 1)),
                '\0' => return Err(syntax_error(NulCharacter, offset..offset + 1)),
                '$' if self.next_is_digit() => {
                    return Err(syntax_error(LegacyPlaceholder, offset..offset + 2));
                }
                _ => {
                    literal.push(ch);
                    continue;
                }
            };
            push_literal(&mut pieces, &mut literal);
            pieces.push(piece);
        }

        if let Some(start) = segment_start {
            return Err(syntax_error(UnclosedSegment, start..self.source.len()));
        }
        push_literal(&mut pieces, &mut literal);
        Ok(pieces)
    }

    fn parse_escape(&mut self, start: usize) -> Result<char, TemplateSyntaxError> {
        match self.chars.next() {
            Some((_, ch @ ('[' | ']' | '\\' | '$'))) => Ok(ch),
            Some((offset, '\0')) => Err(syntax_error(NulCharacter, offset..offset + 1)),
            Some((offset, ch)) => Err(syntax_error(UnknownEscape, start..offset + ch.len_utf8())),
            None => Err(syntax_error(TrailingBackslash, start..start + 1)),
        }
    }

    fn parse_required(&mut self, start: usize) -> Result<ParameterRef, TemplateSyntaxError> {
        let (digits, index) = self.take_digits();
        match self.chars.next() {
            Some((end, '}')) => self.parameter_ref(digits, index, start..end + 1),
            Some((offset, '\0')) => Err(syntax_error(NulCharacter, offset..offset + 1)),
            Some((offset, ch)) => {
                Err(syntax_error(InvalidIndexSpelling, offset..offset + ch.len_utf8()))
            }
            None => Err(syntax_error(UnclosedSubstitution, start..self.source.len())),
        }
    }

    fn parse_bracketed(&mut self, start: usize) -> Result<TemplatePiece, TemplateSyntaxError> {
        let (digits, index) = self.take_digits();
        if !digits.is_empty() && self.consume(']') {
            let end = digits.end + 1;
            return self.parameter_ref(digits, index, start..end).map(TemplatePiece::OptionalField);
        }

        // Without an immediate closing bracket, the initial digits are segment text.
        let pieces = self.parse_pieces(Some(start), self.source[digits].to_owned())?;
        let mut has_substitution = false;
        let mut referenced = Vec::new();
        for piece in &pieces {
            if let TemplatePiece::Required(parameter) = piece {
                has_substitution = true;
                if let ParameterIndex::Position(position) = parameter.index {
                    referenced.push(position);
                }
            }
        }
        if !has_substitution {
            return Err(syntax_error(EmptySegment, start..self.offset()));
        }

        referenced.sort_unstable();
        referenced.dedup();
        Ok(TemplatePiece::Segment(SegmentBody { pieces, referenced }))
    }

    /// Consumes the ASCII digit run once, retaining its spelling and any overflow.
    fn take_digits(&mut self) -> (Range<usize>, ParameterIndex) {
        let start = self.offset();
        let mut position: Option<usize> = Some(0);
        while let Some((_, ch @ '0'..='9')) = self.chars.peek().copied() {
            self.chars.next();
            let digit = (ch as u8 - b'0') as usize;
            position = position.and_then(|value| value.checked_mul(10)?.checked_add(digit));
        }
        let index = position.map_or(ParameterIndex::Unrepresentable, ParameterIndex::Position);
        (start..self.offset(), index)
    }

    fn parameter_ref(
        &self,
        digits: Range<usize>,
        index: ParameterIndex,
        span: Range<usize>,
    ) -> Result<ParameterRef, TemplateSyntaxError> {
        if digits.is_empty() || (digits.len() > 1 && self.source[digits].starts_with('0')) {
            return Err(syntax_error(InvalidIndexSpelling, span));
        }
        Ok(ParameterRef { index, span })
    }

    fn next_is_digit(&mut self) -> bool {
        self.chars.peek().is_some_and(|&(_, ch)| ch.is_ascii_digit())
    }

    fn consume(&mut self, expected: char) -> bool {
        self.chars.next_if(|&(_, ch)| ch == expected).is_some()
    }

    fn offset(&mut self) -> usize {
        self.chars.peek().map_or(self.source.len(), |&(offset, _)| offset)
    }
}

fn push_literal(pieces: &mut Vec<TemplatePiece>, literal: &mut String) {
    if !literal.is_empty() {
        pieces.push(TemplatePiece::Literal(std::mem::take(literal)));
    }
}

fn syntax_error(reason: TemplateSyntaxErrorReason, span: Range<usize>) -> TemplateSyntaxError {
    TemplateSyntaxError { span, reason }
}
