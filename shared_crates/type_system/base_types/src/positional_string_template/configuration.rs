use crate::BaseValueKind;

use super::error::TemplateConfigurationDefect;
use super::scalar::supports_kind;
use super::syntax::{ParameterIndex, PositionalStringTemplate, TemplatePiece};

impl PositionalStringTemplate {
    /// Checks the range and coverage of the ordered parameter positions and their declared kinds.
    pub fn check_configuration(
        &self,
        parameter_kinds: &[BaseValueKind],
    ) -> Result<(), Vec<TemplateConfigurationDefect>> {
        let mut defects = self.check_positions(parameter_kinds.len());
        for (position, &kind) in parameter_kinds.iter().enumerate() {
            if !supports_kind(kind) {
                defects
                    .push(TemplateConfigurationDefect::UnsupportedParameterKind { position, kind });
            }
        }
        if defects.is_empty() {
            Ok(())
        } else {
            Err(defects)
        }
    }

    /// Reports out-of-range references and parameter positions that never appear.
    pub(super) fn check_positions(&self, count: usize) -> Vec<TemplateConfigurationDefect> {
        let mut used = vec![false; count];
        let mut defects = Vec::new();
        mark_references(&self.pieces, &mut used, &mut defects);
        for (position, used) in used.into_iter().enumerate() {
            if !used {
                defects.push(TemplateConfigurationDefect::UnusedParameter { position });
            }
        }
        defects
    }
}

fn mark_references(
    pieces: &[TemplatePiece],
    used: &mut [bool],
    defects: &mut Vec<TemplateConfigurationDefect>,
) {
    for piece in pieces {
        match piece {
            TemplatePiece::Required(reference) | TemplatePiece::OptionalField(reference) => {
                match reference.index {
                    ParameterIndex::Position(position) if position < used.len() => {
                        used[position] = true;
                    }
                    _ => defects.push(TemplateConfigurationDefect::IndexOutOfRange {
                        span: reference.span.clone(),
                    }),
                }
            }
            TemplatePiece::Segment(segment) => mark_references(&segment.pieces, used, defects),
            TemplatePiece::Literal(_) => {}
        }
    }
}
