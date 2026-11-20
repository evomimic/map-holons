use std::borrow::Cow;

use crate::{BaseValue, MapString};

use super::error::TemplateRenderError;
use super::scalar::canonical_text;
use super::syntax::{ParameterIndex, PositionalStringTemplate, TemplatePiece};

/// A parameter state prepared by the caller.
#[derive(Debug, Clone, Copy)]
pub enum TemplateInput<'a> {
    /// One explicit scalar value prepared for this parameter.
    Present(&'a BaseValue),
    /// Complete evidence establishes that the subject has no explicit value for this parameter.
    /// Missing prerequisites, ambiguous bindings, and malformed values are preparation failures.
    Absent,
}

impl PositionalStringTemplate {
    /// Substitutes prepared inputs and returns the completed String within the UTF-8 byte bound.
    pub fn render(
        &self,
        inputs: &[TemplateInput<'_>],
        max_output_bytes: usize,
    ) -> Result<MapString, TemplateRenderError> {
        let defects = self.check_positions(inputs.len());
        if !defects.is_empty() {
            return Err(TemplateRenderError::Configuration(defects));
        }

        let mut text = Vec::with_capacity(inputs.len());
        for (position, input) in inputs.iter().enumerate() {
            text.push(match input {
                TemplateInput::Absent => None,
                TemplateInput::Present(value) => Some(canonical_text(value).ok_or_else(|| {
                    TemplateRenderError::InvalidInput { position, kind: value.kind() }
                })?),
            });
        }

        for piece in &self.pieces {
            if let TemplatePiece::Required(reference) = piece {
                let position = checked_position(&reference.index);
                if text[position].is_none() {
                    return Err(TemplateRenderError::MissingRequiredInput { position });
                }
            }
        }

        let resource = TemplateRenderError::Resource { bound: max_output_bytes };
        let mut total: usize = 0;
        visit_output(&self.pieces, &text, &mut |part| {
            total = total
                .checked_add(part.len())
                .filter(|&length| length <= max_output_bytes)
                .ok_or_else(|| resource.clone())?;
            Ok(())
        })?;

        let mut output = String::new();
        output.try_reserve_exact(total).map_err(|_| resource)?;
        visit_output(&self.pieces, &text, &mut |part| {
            output.push_str(part);
            Ok(())
        })?;
        Ok(MapString(output))
    }
}

/// Visits only the text that will be emitted, in source order.
fn visit_output(
    pieces: &[TemplatePiece],
    inputs: &[Option<Cow<'_, str>>],
    visit: &mut impl FnMut(&str) -> Result<(), TemplateRenderError>,
) -> Result<(), TemplateRenderError> {
    for piece in pieces {
        match piece {
            TemplatePiece::Literal(text) => visit(text)?,
            TemplatePiece::Required(reference) | TemplatePiece::OptionalField(reference) => {
                if let Some(text) = &inputs[checked_position(&reference.index)] {
                    visit(text)?;
                }
            }
            TemplatePiece::Segment(segment) => {
                if segment.referenced.iter().all(|&position| inputs[position].is_some()) {
                    visit_output(&segment.pieces, inputs, visit)?;
                }
            }
        }
    }
    Ok(())
}

/// Extracts a position after configuration validation has succeeded.
fn checked_position(index: &ParameterIndex) -> usize {
    match *index {
        ParameterIndex::Position(position) => position,
        ParameterIndex::Unrepresentable => unreachable!("configuration rejected this index"),
    }
}
